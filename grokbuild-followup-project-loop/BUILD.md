# BUILD.md

Slice: 03 Gate dangerous IPC
Archive: slices/03-gate-dangerous-ipc.md

## Goal
`side_browser_eval` / `snapshot` / `install_download_hook` are fail-closed to side-browser webviews only: they cannot `eval()` into the first-party windows `main`, `session-*`, `pet`, or `theme-editor` (N1). The dangerous IPC that N10 names — flipping global YOLO (`settings_set.permission_policy`), pointing `manual_cli_path` at an attacker binary, starting or publishing the mirror (`mirror_start`, `mirror_set_publish_tunnel`, `mirror_set_allow_remote_yolo`), `plugin_install --trust`, and `serve_start` — is rejected unless the invoking window label is exactly `main`. The same field-level main-only gate covers `settings_set.acp_server_addr` (audit §8 close-as; same command and flip helper, not a new surface; R6 validation is untouched). This is a host-side caller-label check, not a React GlassModal and not Tauri command ACL. Ask stays the default policy. Existing YOLO installs keep their stored policy. A compromised `session-*` / `pet` / `theme-editor` renderer cannot perform those flips.

## Done when
**Pick (only this, not a menu):** host-side **main-only window-label check** via Tauri-injected `window: tauri::Window` + `require_main_window_label(window.label())`. Rejected alternatives: one-shot confirm nonce (no existing host confirm primitive) and Tauri command permissions in `main-only.json` (would require enabling app-command ACL across 423 commands — that is Out). Capability JSON is not edited. The six plugin permissions already in `main-only.json` stay as they are (D1 Held remainder).

**First-party reject list** (explicit; compare the **trimmed** label after `validate_label` charset/length succeeds):
- `main`
- `pet`
- `theme-editor`
- any label whose trimmed value `starts_with("session-")`

**N1 — side-browser target**

- `validate_side_label` (`src-tauri/src/side_browser_host.rs:117–123` today) becomes: `validate_label` → `is_first_party_webview_label` (named `pub(crate)` helper; true for the list above) → prefix `resource-browser` (`LABEL_PREFIX` at `:40`). First-party hit returns a named `pub(crate)` constant `FIRST_PARTY_SIDE_TARGET_ERR` whose text contains `first-party` (case-insensitive) and does **not** mention `resource-browser` (so the `eval("main")` test is not satisfied by the generic prefix error alone). The first-party error fires **before** the prefix error.
- `get_side_webview` (`:142–149`) calls `validate_side_label`, not `validate_label`. Consequence: `navigate` / `reload` / `current_url` (`:654–675`) also cannot retarget first-party windows. That is in-scope fail-closed of the same function, not a new command family.
- `eval` (`:685–701`) calls `validate_side_label` (or a one-line `pub(crate) fn check_side_eval_target(label: &str) -> Result<(), String>` that is only `validate_side_label`) **before** script-empty/size checks and **before** `get_side_webview`. `snapshot` (`:704–718`) stays a wrapper around `eval` and inherits the gate.
- `install_hook` (`src-tauri/src/side_browser_blob.rs:1168–1181`) calls `side_browser_host::validate_side_label` (made `pub(crate)`) before `get_webview`. Empty-label check may stay as a fast path; first-party and non-prefix labels must fail `validate_side_label`.
- Grep (cwd `/workspace`, counts recorded in Proof):
  - `rg -n 'validate_label\(' src-tauri/src/side_browser_host.rs` lists exactly **four** matches, and those four are only: (1) the `fn validate_label` definition, (2) the single production call `validate_label(label)?;` inside `validate_side_label`, (3) `assert!(validate_label("resource-browser-tab1").is_ok());` in `label_rules`, (4) `assert!(validate_label("../x").is_err());` in `label_rules`. Proof records the `-n` listing plus enough enclosing context to identify each hit (`fn validate_label` / `validate_side_label` / `label_rules`). `eval` and `get_side_webview` no longer call `validate_label` directly — a leftover in either is a fifth or sixth hit whose context is `eval` / `get_side_webview` and fails this list. Do not use a per-line `rg -v 'mod tests'` (or similar) as the criterion: the two `label_rules` lines sit inside `mod tests` but the lines themselves do not contain `mod tests`.
  - `rg -n 'validate_side_label' src-tauri/src/side_browser_host.rs` matches `validate_side_label` itself plus `create` (`:288`), `close` (`:632`), `get_side_webview`, and `eval` (or `check_side_eval_target`).
  - `rg -n 'validate_side_label' src-tauri/src/side_browser_blob.rs` ≥ 1, inside `install_hook`.
- Named tests in `side_browser_host.rs` `mod tests` (names normative):
  - `eval_rejects_first_party_labels` — `check_side_eval_target("main")` (or `validate_side_label("main")` if eval calls it directly) is `Err` and the message contains `first-party`; same for `"session-abc"`, `"pet"`, `"theme-editor"`; `"resource-browser-tab1"` is `Ok`. This is the audit accept-when `eval(label="main")` errors. Honest limitation: `eval` itself needs `AppHandle`; the test proves the function `eval` calls first, and the grep above proves `eval` calls it.
  - Existing `label_rules` (`:726–730`) still passes; add `validate_side_label("main")` / `"session-x"` / `"pet"` / `"theme-editor"` → `Err`.
- Named test in `side_browser_blob.rs` `mod tests`: `install_hook_rejects_first_party_labels` — a `pub(crate)` precheck used as the first non-empty-label statement of `install_hook` (same `validate_side_label`) rejects `"main"` / `"session-x"` / `"pet"` / `"theme-editor"` and accepts `"resource-browser-x"`.

**N10 — dangerous IPC, main-only**

- `pub(crate) const MAIN_ONLY_IPC_ERR: &str` in `src-tauri/src/commands/mod.rs` (facade stays ≪ 800 lines). Text: `this command may only be invoked from the main window`. `pub(crate) fn require_main_window_label(label: &str) -> Result<(), String>` returns `Ok` iff `label.trim() == "main"`, else `Err(MAIN_ONLY_IPC_ERR.into())`.
- Every gated command takes an injected `window: tauri::Window` (Tauri fills it; JS `invoke` args are unchanged) and calls `require_main_window_label(window.label())?` **before** any state mutation / spawn. `mirror` / `serve` call `crate::commands::require_main_window_label`.
- Commands that are **unconditionally** main-only (whole command):
  - `mirror_start` (`src-tauri/src/mirror/mod.rs:847–861`) — including when `publish_tunnel` / `allow_remote_yolo` are `None`. `maybe_autostart` (`:870`) keeps calling `host.start()` directly and is **not** gated (headless env path, not IPC).
  - `mirror_set_publish_tunnel` (`:1180–1184`)
  - `mirror_set_allow_remote_yolo` (`:1188–1193`)
  - `plugin_install` (`src-tauri/src/commands/extensions_p2.rs:462–474`) — `--trust` argv unchanged
  - `serve_start` (`src-tauri/src/serve.rs:674–677`) — check **before** `spawn_blocking`
- `settings_set` (`src-tauri/src/commands/settings.rs:13–17`) is **field-gated**, not whole-command-gated (session windows already call it for `defaultOpenTarget`; `src/components/side-workbench/FilesWorkspace.tsx:140–142`; theme-editor calls it for `theme`). Before `validate_manual_cli_path` / save:
  - `pub(crate) fn dangerous_settings_flipped(prev_policy, next_policy, prev_cli, next_cli, prev_acp, next_acp) -> bool` is true if any of: `PermissionPolicy::parse` of the two policies differ (`parse` never fails — unknown tokens become `Ask`, existing behaviour); trimmed optional `manual_cli_path` differs; trimmed optional `acp_server_addr` differs (`None` / `Some("")` / whitespace-only are the same empty).
  - If true, `require_main_window_label(window.label())?`. If false, any first-party caller may proceed (then existing validators run).
  - R6 `validate_acp_server_addr_setting` (`:756–778`) stays byte-identical. Non-loopback still needs `confirm_remote_acp_server` (client bool). Main-only is an additional caller check on any addr flip, including loopback.
- Named tests (normative):
  - `commands/mod.rs` `mod ipc_gate_tests`: `require_main_window_label_allows_only_main` — `"main"` and `"  main  "` Ok; `"session-abc"`, `"pet"`, `"theme-editor"`, `""`, `"Main"` Err; Err equals `MAIN_ONLY_IPC_ERR`.
  - `settings_tests`: `settings_set_always_approve_from_non_main_errors` — `dangerous_settings_flipped("ask", "always_approve", None, None, None, None)` is true; composing `require_main_window_label("session-abc")` / `"pet"` / `"theme-editor"` is `Err(MAIN_ONLY_IPC_ERR)`. `"main"` Ok. This is the audit accept-when `settings_set{permission_policy:"always_approve"}` from a non-main label errors. Honest limitation: `settings_set` needs `AppHandle`/`State`; the test proves the two functions `settings_set` calls for this gate, and Proof greps that `settings_set` calls them inside the flip guard.
  - `settings_tests`: `settings_set_unrelated_field_from_non_main_ok` — `dangerous_settings_flipped` with identical policy/cli/acp is false (covers session-window `defaultOpenTarget` and theme-editor `theme` saves).
  - `settings_tests`: `settings_set_manual_cli_path_from_non_main_errors` — cli path flip → requires main.
  - `settings_tests`: `settings_set_acp_server_addr_from_non_main_errors` — addr flip (e.g. `None` → `"127.0.0.1:8799"`) → requires main.
  - Existing `validate_acp_server_addr_gate_tests` and `settings_set_validates_manual_cli_path` still pass (R6 / path validators unchanged).
- Grep: `rg -n 'require_main_window_label' src-tauri/src/` lists exactly the helper, `settings_set` (inside the flip guard), `mirror_start`, `mirror_set_publish_tunnel`, `mirror_set_allow_remote_yolo`, `plugin_install`, `serve_start`, and tests. Zero matches on `session_set_policy` / `composer_prefs_set` / `project_set_permission_policy` / `mirror_set_allow_lan`.
- `rg -c '#\[tauri::command\]' src-tauri/src` remains 423 (add `Window` params only; do not add/remove commands).
- `src-tauri/capabilities/default.json` and `src-tauri/capabilities/main-only.json` are byte-identical to HEAD (6 plugin perms only on main-only).

No file outside the Files constraint changes.

## Out
- Redesigning the 423-command surface in one slice.
- Held IDs: P1, P3, P4, P5, R1, R2, R3, R5, R6 (do not change ACP validation / `confirm_remote_acp_server` semantics), S1, S3, C3, D2–D6; D1 remainder already accepted (the 6 plugin permissions in `main-only.json`).
- Enabling Tauri app-command ACL; adding `allow-*` entries to capability JSON; one-shot confirm nonce; native OS dialog.
- Main-only gating of `side_browser_create` / `close` / `list` / `navigate` / `reload` / `url` as **caller** checks (session windows host the overlay via `window_label`; N1 is the **target** label, not the caller). Target-label fail-closed via `get_side_webview` is in-scope.
- `session_set_policy` (`settings.rs:425–443`), `composer_prefs_set` (`:355–396`), `project_set_permission_policy` (`session_p1.rs:576–593`) — per-session/project composer YOLO used from `session-*` windows. Residual: XSS in a session window can still flip **that** session’s policy. Global `settings_set.permission_policy` cannot.
- `mirror_set_allow_lan`, `mirror_set_read_only`, `mirror_set_max_clients`, `mirror_rotate_token`, `mirror_stop`, `serve_stop`.
- Tightening `validate_manual_cli_path` to a known install root / first-seen hash (audit N10 extra sentence). Residual for later / release review.
- Frontend GlassModal / `setAppDialog` / i18n. Settings YOLO `<Select>` (`GeneralSection.tsx:479–481`) and `serveStart` (`LeaderServePanel.tsx:192–196`) stay React-unconfirmed even on main. Mirror publish/YOLO and plugin `--trust` keep their existing GlassModals. Slice 09 leftover: docs must state React-vs-host confirm after this lands (`docs/features/remote-security.md:23` overclaim).
- Slices 04–09 work (serve secret names, CLI installer, headless children, 0600 writes, path_scope / replay, docs).
- rustfmt/clippy baseline fixes (see Constraints).
- Widening `allow_from`; disabling parent-session Grok subagents; publishing / deploying.

## Constraints
- **Files:** `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/settings.rs`, `src-tauri/src/commands/extensions_p2.rs`, `src-tauri/src/side_browser_host.rs`, `src-tauri/src/side_browser_blob.rs`, `src-tauri/src/mirror/mod.rs`, `src-tauri/src/serve.rs`. No other file. No `Cargo.toml` / `Cargo.lock`. No `capabilities/*.json`. No `src/`, i18n, or docs.
- Do not add a new crate module (helper lives in `commands/mod.rs`). Do not edit `lib.rs`.
- Ask remains the default permission policy. Do not change `store.rs` defaults. Do not rewrite stored settings.
- Held R2 plumbing (`allow_remote_yolo` default false, env `GROK_MIRROR_ALLOW_REMOTE_YOLO`) is unchanged; only the IPC setters gain a caller check.
- `plugin_install` keeps `--trust` on argv (required for non-interactive install). The gate is who may invoke, not the flag.
- Rust style: new/changed hunks rustfmt-clean. **Do not rustfmt-rewrite pre-existing dirt** in `mirror/mod.rs` or `serve.rs` (they are already on the slice-02 dirty list). rustfmt/clippy non-regression vs slice 02 baseline (rustc 1.98.1): `cargo fmt --all -- --check` still exits 1 with diffs **only** in the same 16 files (`agent_home_config.rs`, `batch_agents.rs`, `cli_install.rs`, `cli_update.rs`, `mirror/mod.rs`, `mirror/rpc.rs`, `models_aux.rs`, `official_aux.rs`, `path_scope.rs`, `permission.rs`, `relay_stream_proxy.rs`, `secrets.rs`, `serve.rs`, `session_manager/control.rs`, `store.rs`, `wallpaper_source.rs`); `cargo clippy --all-targets -- -D warnings` still exactly `batch_agents.rs:79` (`unnecessary_map_or`), `path_scope.rs:129` (`manual_contains`), `wecom.rs:210` (`too_many_arguments`). Do not fix those here. Slice files that are not on that dirty list (`commands/mod.rs`, `settings.rs`, `extensions_p2.rs`, `side_browser_host.rs`, `side_browser_blob.rs`) must be `rustfmt --edition 2021 --check` clean and introduce zero clippy findings.
- Do not claim cargo passed unless this session ran it.

## Data / state impact
- Stored `permission_policy`, `manual_cli_path`, `acp_server_addr`, mirror flags, and serve state are not migrated. A user already on YOLO stays on YOLO until they change it from `main`.
- Session-window `settings_set({ ...s, defaultOpenTarget })` and theme-editor `settings_set({ ...s, theme })` keep working when the copied `s` matches stored policy/cli/acp. A session/theme-editor window holding a **stale** full settings object whose policy/cli/acp differ from the stored prev is fail-closed (main-only error). That is intentional: the host cannot tell a stale copy from an XSS flip.
- Setup wizard / Settings on `main` keep working (`Window` is injected; JS invoke args unchanged).
- `GROK_MIRROR_HEADLESS=1` autostart unchanged (`maybe_autostart` → `host.start()`, no window).
- Capability JSON unchanged; no new permission prompts; no new stored nonce.
- Confirm UX unchanged: host does not show a dialog. GlassModal remains React-only on the surfaces that already have it. Slice 09 must not claim host confirm for these commands — it should say main-only IPC + React GlassModal where present.

## Tests
- `cargo test --manifest-path src-tauri/Cargo.toml --lib side_browser_host::tests` — existing tests plus `eval_rejects_first_party_labels`; `label_rules` still ok; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib side_browser_blob::tests` — existing plus `install_hook_rejects_first_party_labels`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::ipc_gate_tests` — `require_main_window_label_allows_only_main`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::settings_tests` — existing plus the four `settings_set_*` tests named above; `validate_acp_server_addr_gate_tests` and `settings_set_validates_manual_cli_path` still ok; `0 failed`.
- Negative proof: Proof includes `rg` showing `eval` / `get_side_webview` no longer call `validate_label` directly, and a sentence that pre-change `eval_rejects_first_party_labels` would have no symbol / `validate_side_label("main")` is `Err` only after this change (`label_rules` today only checks `"other"`; today's `validate_side_label("main")` is already `Err` but the message is the prefix error, not `FIRST_PARTY_SIDE_TARGET_ERR`).
- Grep criteria in Done when; each command + count in Proof.
- Lint non-regression: `rustfmt --edition 2021 --check` on the five previously-clean Files exits 0; `cargo fmt --all -- --check` dirty set identical to the 16-file baseline (including still-dirty `mirror/mod.rs` and `serve.rs`); clippy exactly the three baseline lints, none in Files.
- Scope: `git diff --stat` vs the 03 implementation baseline lists exactly the seven Files.
- No `pnpm vitest` (no TS change). Implementation Proof runs the targeted libs above. Full `cd src-tauri && cargo test` is required in implementation Proof (environment links: webkit2gtk present, same as slice 02); expected `0 failed`.

## Proof
none (Proposed; draft `D03-DRAFT-1` produced this page, no code)

## Review
Plan approval: none (`D03-PLAN-1` REJECT_PLAN; see below)
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

### D03-PLAN-1 — REJECT_PLAN (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-f13a47d0-9c79-51eb-abd4-a397a8f66912`, worktree `/tmp/loop-review/D03-PLAN-1` @ `d98db113`.
Contract `116fc217…070e` (match). Candidate before/after `2a3620d1…390b` (unchanged, clean-tree). Porcelain empty. Code vs `fd142233` empty.
Judgments: (a) main-only window-label check satisfies Now 03 / locked D1; (b) `acp_server_addr` in field gate is in authority (audit §8, same command); (c) field-gate reason real (`FilesWorkspace` / theme-editor); (d) helper tests + greps honest except one unsatisfiable grep; (e) one coherent slice.
Blocker 1: Done when / N1 grep ``rg -n 'validate_label\(' src-tauri/src/side_browser_host.rs` → exactly **one** match`` is unsatisfiable together with “`label_rules` still passes”. Today the pattern matches the definition, the `validate_side_label` call, `get_side_webview`, `eval`, and two `label_rules` lines (6). After a correct impl it is still ≥4 (def + one production call + two test asserts). Repair: count production call sites excluding `fn` / `mod tests`, or name the exact remaining lines.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/grokbuild-followup-loop-c341` off `origin/main` `ea4ec712` (= `c66b3ec7` + pack files only).
Worker / role / phase: Reviewer / plan review (revised contract) / slice 03
Dispatch ID / launch state / input identity: `D03-PLAN-2` / launching / candidate `2a3620d1…390b` (code HEAD `fd142233`), contract pending recompute, draft `D03-DRAFT-2` (blocker 1 grep rewrite only)
Pending result / last consumed dispatch: none / `D03-DRAFT-2`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir (source, tests, `.github/workflows/`, `scripts/`, lockfiles, docs, capabilities, assets).
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/` (protocol + artifacts).
Baseline snapshot: slice 02 shipped candidate — HEAD `fd142233dd2235cb3832a87b00bdb95d83cb74f2` (code), clean-tree, CANDIDATE `2a3620d159da5a3960a7b59e66e8908de27727a3c265ad0a0760235a5bb7390b`
Contract identity: `116fc21710bd21f874c157b8ffb1073d694673a079710c586fb8bcdbeb06070e`
Candidate snapshot: HEAD `fd142233dd2235cb3832a87b00bdb95d83cb74f2` (code commit), clean-tree, CANDIDATE `2a3620d159da5a3960a7b59e66e8908de27727a3c265ad0a0760235a5bb7390b`
Rejection count: 1
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: D03-PLAN-1 blocker 1 addressed in contract by `D03-DRAFT-2` (named four-line `validate_label(` list); pending `D03-PLAN-2`
Repair awaiting review: false
Review events:
- E1 / `D03-PLAN-1` / plan / REJECT_PLAN / contract `116fc217…070e`, candidate `2a3620d1…390b` / gap: blocker 1 / rejection count 0→1
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: none
Advance phase: next selected (03); BUILD replaced with Proposed page
Next slice ID / draft: 04 (after 03 ships)
Environment note: `cargo test` is linkable here — webkit2gtk-4.1 2.52.6, gtk+-3.0 3.24.41, libsoup-3.0, javascriptcoregtk-4.1, ayatana-appindicator3, librsvg installed via apt on 2026-09-06; rustc 1.98.1 stable default.

## Status
Proposed (revised after `D03-PLAN-1`; pending `D03-PLAN-2`)

## Next
Independent plan review `D03-PLAN-2`. On APPROVE_PLAN → Not started, Builder `D03-BUILD-1`. On REJECT_PLAN → Proposed, Builder revises.
