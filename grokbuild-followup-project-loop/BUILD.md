# BUILD.md

Slice: 04 Serve secret names
Archive: slices/04-serve-secret-names.md

## Goal
The app-spawned `grok agent serve` child receives the same secret under **both** `GROK_AGENT_SECRET` and `GROK_SERVE_SECRET`. `--secret` and the token stay off argv (`ps` cannot read them). After the child is listening, an **unauthenticated** HTTP GET to `/health` on the bind runs; if that request returns 2xx, the start path **does not** populate `connection_url` / `connection_cli`. A non-loopback bind is kept only when that unauthenticated GET is **not** 2xx; if it is 2xx the tracked child is killed and `serve_start` returns `Err`. Missing `/health` (connect fail / non-HTTP) is **Inconclusive**, not Open — it does not kill bind (official CLI does not document `/health`; requiring `Closed` would break LAN serve). Slice 03 `serve_start` main-only gate stays. Ask stays default.

## Done when
**Pick (only this, not a menu):** **Unauthenticated HTTP/1.1 GET `/health`** over a direct `TcpStream` to the bind (no reqwest, no proxy, no new crate). No secret is sent. Classify the first status line; drive advertise + non-loopback keep/kill from that class. Rejected: authenticated `/health`, WebSocket `/ws` upgrade probe, `serve_tcp_probe` reuse as the auth gate.

Official Grok Build documents `GROK_AGENT_SECRET` / `--secret` and `ws://{bind}/ws?server-key=…`. This repo and that user-guide do **not** document an HTTP `/health` on `grok agent serve`. The probe target is still `/health` so the slice is observable; live CLI without that route is Inconclusive (not 2xx). Residual recorded.

### R7 — both env names, `--secret` off argv
- `build_serve_command` (`src-tauri/src/serve.rs:479–509` today) sets **both**:
  - `cmd.env("GROK_SERVE_SECRET", secret);`
  - `cmd.env("GROK_AGENT_SECRET", secret);`
- Delete `cmd.env_remove("GROK_AGENT_SECRET");` (`:506` today). Do not replace it with another remove of that name.
- Do not add `.arg("--secret")` or the token to argv. Existing `--bind` / optional `--remote` argv stays.
- `build_connection_cli` / `build_connection_cli_masked` (`:189–206`) stay `GROK_SERVE_SECRET=… grok --remote ws://…` (no `--secret`). Child env is the R7 fix; the copy-paste hint is not rewritten.
- Grep (cwd `/workspace`, Proof records `-n` listings):
  - `rg -n 'env_remove\("GROK_AGENT_SECRET"\)' src-tauri/src/` → **0**
  - `rg -n 'cmd\.env\("GROK_AGENT_SECRET"' src-tauri/src/serve.rs` → **exactly 1** (inside `build_serve_command`)
  - `rg -n 'cmd\.env\("GROK_SERVE_SECRET"' src-tauri/src/serve.rs` → **exactly 1** (inside `build_serve_command`)
  - `rg -n '\.arg\("--secret"\)' src-tauri/src/serve.rs` → **0**
  - `rg -n 'require_main_window_label' src-tauri/src/serve.rs` → **exactly 1**, the existing call at today’s `:680`, **before** `spawn_blocking`. Do not move it inside the blocking closure; do not drop `window: tauri::Window`.

### N3 — post-start unauthenticated `/health` probe
**Named types / fns** (`pub` or `pub(crate)` in `serve.rs`; tests in the same `mod tests` via `include!("serve_tests_ext.rs")`):
- `pub const UNAUTH_HEALTH_PATH: &str = "/health";`
- `pub const UNAUTH_HEALTH_PROBE_MS: u64 = 800;`
- `pub enum UnauthHealthClass { Open, Closed, Inconclusive }`
- `pub fn unauth_health_url(bind: &str) -> String` — `http://{host}/{path}` with path `UNAUTH_HEALTH_PATH`. Host rewrite: bind host `0.0.0.0` → `127.0.0.1`; `::` / `[::]` → `[::1]`; other hosts unchanged (keep IPv6 brackets). No query string.
- `pub fn classify_unauth_health_status(status: Option<u16>) -> UnauthHealthClass` — `Some(200..=299)` → `Open`; `Some(other)` → `Closed`; `None` → `Inconclusive`.
- `pub fn serve_auth_policy(class: UnauthHealthClass, non_loopback: bool) -> ServeAuthPolicy` where `ServeAuthPolicy { advertise: bool, keep_bind: bool }`:
  - `advertise` is `false` iff `class == Open`
  - `keep_bind` is `false` iff `non_loopback && class == Open`
- `fn probe_unauth_health(bind: &str) -> UnauthHealthClass` (same module; `pub(crate)` ok):
  1. Build URL via `unauth_health_url`.
  2. `TcpStream::connect_timeout` to the rewritten host:port, timeout `UNAUTH_HEALTH_PROBE_MS`. Do **not** use `crate::proxy::apply_to_*`. Do **not** use reqwest.
  3. Write only:
     `GET /health HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n`
     Path is exactly `/health`. No `Authorization`, `Cookie`, `server-key`, `GROK_AGENT_SECRET`, `GROK_SERVE_SECRET`, or query.
  4. Read ≤ 8192 bytes. Parse the first line as `HTTP/1.x <code>`. Missing/unparseable/timeout/connect-fail → `classify_unauth_health_status(None)`.
- `fn finish_serve_start_with_probe(bind: &str) -> Result<ServeStatusDto, String>`:
  1. `class = probe_unauth_health(bind)`
  2. `policy = serve_auth_policy(class, is_non_loopback_bind(bind))`
  3. If `!policy.keep_bind`: take/kill `TRACKED_SERVE` (same kill path as `serve_stop`’s tracked take), return `Err` whose text contains `unauthenticated` and `non-loopback` and does **not** contain the secret or `--secret`.
  4. `st = collect_status_sync(policy.advertise)`
  5. If `class == Open`: force `st.connection_url = None`, `st.connection_cli = None`, set `st.message` to a host string containing `unauthenticated` (no secret).
  6. `Ok(st)`

**Wire into `serve_start` only** (today `:674–786`). Replace every `collect_status_sync(true)` in that command (today `:685` already-running re-issue, `:768` port-open success, `:772` deadline still-alive) with `finish_serve_start_with_probe(&bind_norm)` — for the re-issue arm use the tracked/current bind (same string `collect_status_sync` would use; default `DEFAULT_SERVE_BIND` if somehow missing). Do **not** call `probe_unauth_health` from `serve_status` / `collect_status_sync(false)` / `serve_tcp_probe` / `serve_stop`.

`serve_start` still: `window: tauri::Window` → `require_main_window_label(&caller)?` → `spawn_blocking`. Gate before any spawn/mutate. `#[tauri::command]` count across `src-tauri/src` stays **423**.

Grep:
- `rg -n 'collect_status_sync\(true\)' src-tauri/src/serve.rs` → **exactly 1**, inside `finish_serve_start_with_probe` (not inlined in the three former arms).
- `rg -n 'probe_unauth_health' src-tauri/src/serve.rs` → the `fn` plus the call inside `finish_serve_start_with_probe` (and tests may sit in `serve_tests_ext.rs`, which this pattern also matches if `rg` hits the include file — Proof lists each line’s function). Zero matches in `serve_tcp_probe` / `serve_status` / `serve_stop`.
- `rg -n 'UNAUTH_HEALTH_PATH' src-tauri/src/serve.rs src-tauri/src/serve_tests_ext.rs` ≥ 2 (const + URL builder and/or probe write).

**Named tests** in `serve_tests_ext.rs` `mod tests` (names normative):
- `spawn_serve_process_passes_secret_via_env` (existing, `:206`) — argv still has no `--secret` and no token; `envs.get("GROK_SERVE_SECRET")` **and** `envs.get("GROK_AGENT_SECRET")` are both `Some(&Some("sekrit-token-123".to_string()))`. This is the audit accept-when “both env names present, `--secret` absent”.
- `classify_unauth_health_status_matrix` — `Some(200)` / `Some(204)` → `Open`; `Some(401)` / `Some(403)` / `Some(404)` / `Some(500)` → `Closed`; `None` → `Inconclusive`. `serve_auth_policy(Open, false).advertise == false` and `.keep_bind == true`; `serve_auth_policy(Open, true).keep_bind == false`; `Closed`/`Inconclusive` × `{false,true}` → `advertise == true` and `keep_bind == true`.
- `unauth_health_url_rewrites_unspecified_and_keeps_loopback` — `unauth_health_url("127.0.0.1:2419") == "http://127.0.0.1:2419/health"`; `"0.0.0.0:2419"` → `http://127.0.0.1:2419/health`; `"[::1]:2419"` contains `/health` and `[::1]`; `"0.0.0.0:2419"` / `"::"`-form has **no** query (`?` absent).
- `unauth_health_probe_open_refuses_advertise` — `std::net::TcpListener::bind("127.0.0.1:0")` thread replies `HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n`; `probe_unauth_health` → `Open`; `serve_auth_policy(Open, false).advertise == false`.
- `unauth_health_probe_closed_allows_advertise` — same fake server with `401 Unauthorized`; class `Closed`; `advertise == true`; `keep_bind == true` even when `non_loopback` is true.
- `unauth_health_probe_sends_no_secret` — fake server records the first request; it contains `GET /health`; it does **not** contain `server-key`, `Authorization`, `Cookie`, `GROK_AGENT_SECRET`, `GROK_SERVE_SECRET`, or `--secret`.

Existing tests still pass, including `build_connection_cli_template_and_mask` (still `GROK_SERVE_SECRET=` and no `--secret`) and `normalize_bind_detects_non_loopback`.

No file outside Files changes.

## Out
- Changing official CLI source (Out of Now 04). Do not add a `/health` handler to Grok Build.
- Held IDs. Do not reopen P1, P3, P4, P5, R1, R2, R3, R5, R6, S1, S3, C3, D1–D6.
- Undoing slice 03: `serve_start` stays main-only via `require_main_window_label`. Do not edit capability JSON. Command count stays 423.
- Authenticated health (secret query/header), WebSocket `/ws` upgrade probe, probing `--remote` upstream.
- Rewriting `build_connection_cli` to advertise `GROK_AGENT_SECRET=` or both names; rewriting `src/lib/serveConnect.ts` `grokRemote` (`--secret` examples). Residual for docs / later.
- Frontend / i18n / `LeaderServePanel` / `SdkConnectWizard`. Omitting `connectionUrl` already skips clipboard (`LeaderServePanel.tsx:198–199`).
- Gating `serve_stop`, `serve_status`, `serve_tcp_probe`. Changing default bind. Widening `allow_from`. Disabling parent-session Grok subagents. Publishing / deploying.
- rustfmt/clippy baseline fixes (see Constraints). Do not rustfmt-rewrite pre-existing dirt in `serve.rs` beyond required hunks. Do not rustfmt-rewrite untouched pre-existing dirt in `serve_tests_ext.rs` unless that test is edited.
- Slices 05–09.

## Constraints
- **Files:** `src-tauri/src/serve.rs`, `src-tauri/src/serve_tests_ext.rs`. No other file. No `Cargo.toml` / `Cargo.lock`. No `src/`, i18n, docs, capabilities, `lib.rs`.
- Ask remains default. Do not change `store.rs` defaults. Do not rewrite stored settings.
- Do not add crates. Probe is std `TcpStream` + `TcpListener` in tests. `reqwest` stays unused here.
- Direct connect only — no `proxy::apply_to_reqwest` / `apply_to_std_command` on the probe socket (child spawn still uses existing `apply_to_std_command` for `--remote`).
- Rust style: new/changed hunks rustfmt-clean. **Do not rustfmt-rewrite pre-existing dirt** in `serve.rs` (already on the slice 02/03 dirty list). rustfmt/clippy non-regression vs that baseline (rustc 1.98.1): `cargo fmt --all -- --check` still exits 1 with diffs **only** in the same 16 files (`agent_home_config.rs`, `batch_agents.rs`, `cli_install.rs`, `cli_update.rs`, `mirror/mod.rs`, `mirror/rpc.rs`, `models_aux.rs`, `official_aux.rs`, `path_scope.rs`, `permission.rs`, `relay_stream_proxy.rs`, `secrets.rs`, `serve.rs`, `session_manager/control.rs`, `store.rs`, `wallpaper_source.rs`). `serve_tests_ext.rs` is `include!`d; if it is already rustfmt-dirty, do not expand those hunks; new tests fmt-clean. `cargo clippy --all-targets -- -D warnings` still exactly `batch_agents.rs:79` (`unnecessary_map_or`), `path_scope.rs:129` (`manual_contains`), `wecom.rs:210` (`too_many_arguments`). Do not fix those here.
- Do not claim cargo passed unless that session ran it.

## Data / state impact
- No settings / secret-store migration. `TRACKED_SERVE` still holds the full secret in memory for mask/stop.
- Loopback + `Open`: process stays; full connection strings omitted; UI does not auto-copy (`if (st.connectionUrl)`).
- Non-loopback + `Open`: process killed; start errors; bind is not left up.
- `Closed` / `Inconclusive` (typical CLI with no `/health`): advertise and keep-bind unchanged from today’s TCP-ready start, including non-loopback (still `warn!` + `exposure_warning`). Residual: secretless WebSocket on LAN is undetected if `/health` is absent.
- `serve_status` polls still omit full `connection_url` / `connection_cli`; `connection_cli_masked` last-4 unchanged.
- Main-window `invoke("serve_start")` args unchanged (`Window` injected).

## Tests
- `cargo test --manifest-path src-tauri/Cargo.toml --lib serve::tests` — existing tests plus the five new names above; `spawn_serve_process_passes_secret_via_env` asserts both env names; `0 failed`.
- Negative proof: Proof includes today’s `rg -n 'env_remove\("GROK_AGENT_SECRET"\)' src-tauri/src/serve.rs` (line `:506`) and a sentence that pre-change `envs.get("GROK_AGENT_SECRET")` is absent/`None` (the command **removes** it). After change that pattern is 0 and both env keys are `Some(token)`.
- Grep criteria in Done when; each listing + count in Proof.
- Lint non-regression: dirty set identical to the 16-file baseline (including still-dirty `serve.rs`); clippy exactly the three baseline lints, none introduced in Files.
- Scope: `git diff --stat` vs the 04 implementation baseline lists exactly the two Files.
- No `pnpm vitest` (no TS change). Implementation Proof runs `serve::tests` above. Full `cd src-tauri && cargo test` is required in implementation Proof (webkit2gtk present); expected `0 failed`.

## Proof
none (Proposed; draft `D04-DRAFT-1` produced this page, no code)

## Review
Plan approval: none
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/grokbuild-followup-loop-c341` off `origin/main` `ea4ec712` (= `c66b3ec7` + pack files only).
Worker / role / phase: Reviewer / plan review / slice 04
Dispatch ID / launch state / input identity: `D04-PLAN-1` / launching / candidate `d72f7320…52a6` (code HEAD `0aed78ab`), contract pending recompute, draft `D04-DRAFT-1`
Pending result / last consumed dispatch: none / `D04-DRAFT-1`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir (source, tests, `.github/workflows/`, `scripts/`, lockfiles, docs, capabilities, assets).
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/` (protocol + artifacts).
Baseline snapshot: slice 03 shipped candidate — HEAD `0aed78abe79def986d98b7594a7625a334df8cc0` (code), clean-tree, CANDIDATE `d72f73209511cb4cae63933103c68287a8fabe2b8b64cbdc522a465f368252a6`
Contract identity: (recompute after commit)
Candidate snapshot: HEAD `0aed78abe79def986d98b7594a7625a334df8cc0` (code commit), clean-tree, CANDIDATE `d72f73209511cb4cae63933103c68287a8fabe2b8b64cbdc522a465f368252a6`
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events: none
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: none
Advance phase: next selected (04); BUILD replaced with Proposed page
Next slice ID / draft: 05 (after 04 ships)
Environment note: `cargo test` is linkable here — webkit2gtk-4.1 / gtk+-3.0 / rustc 1.98.1 stable, same as slices 02–03.

## Status
Proposed (draft `D04-DRAFT-1`; pending `D04-PLAN-1`)

## Next
Independent plan review `D04-PLAN-1` in isolated worktree. On APPROVE_PLAN → Not started, Builder `D04-BUILD-1`. On REJECT_PLAN → Proposed, Builder revises.
