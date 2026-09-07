# BUILD.md

Slice: 07 0600 every agent-home secret write
Archive: slices/07-0600-every-agent-home-secret-write.md

## Goal
The seven leftover production writes of App `agent-home/config.toml` and MCP OAuth token files use `write_private_agent_home_file` so those secrets are created at Unix mode `0600` (not umask `0644`). Live leftover count in the N6/S2 audit files is **seven call sites in six files** (both `mcp_oauth` sites). Held helper (`agent_home_config.rs`) and `providers.rs` stay Held. Ask stays default.

## Done when
Close **S2, N6** only. Do not reopen Held IDs. Do not implement slices 08–09. Do not rewrite the agent-home module or change helper semantics except to **call** it.

Locked family (copy; do not invent a new writer set). S2: all **listed** leftover agent-home secret writes go through `write_private_agent_home_file`. N6 leftover files/lines at code HEAD `0b536c3d` (verify live; do not trust stale numbers blindly):

- `src-tauri/src/agent_config_edit.rs:604` — `save_agent_config_edit` → `agent_config_toml()`
- `src-tauri/src/agent_privacy.rs:316` — `save_privacy_config` → `agent_config_toml()`
- `src-tauri/src/agent_codebase_indexing.rs:299` — `save_codebase_indexing` → `agent_config_toml()`
- `src-tauri/src/permission_rules.rs:440` — `save_permission_rules` → `permission_config_path` (independent: agent-home `config.toml`; shared: `~/.grok/config.toml`)
- `src-tauri/src/agent_memory_embed.rs:665` — `save_memory_embed_config` → `agent_config_toml()`
- `src-tauri/src/mcp_oauth.rs:771` — `persist_oauth_tokens` → `mcp_agent_config_path` **and** `~/.grok/config.toml` (Bearer in `config.toml`)
- `src-tauri/src/mcp_oauth.rs:857` — `write_mcp_credentials_full` → `mcp_credentials.json` under `credential_homes()` (agent GROK_HOME **and** `~/.grok`)

Replace **each** of those seven `fs::write` / `std::fs::write` calls with `crate::agent_home_config::write_private_agent_home_file`. Keep each site’s existing `map_err` string (`"write config: {e}"` vs `e.to_string()`). Do **not** switch these sites to `update_config_toml` / `update_config_toml_if_independent` (shared-mode refuse would change `permission_rules` and `mcp_oauth` destinations). Do not change shared-vs-independent write policy.

At `mcp_oauth.rs:857`, drop the following Unix `set_permissions(0o600)` block (`:858–862`); the helper creates and chmods `0600`. The helper’s non-Unix `fs::write` fallback (`agent_home_config.rs:600`) is **not** a leftover site.

Call-site replacement only. `lib.rs` unchanged. `#[tauri::command]` count stays **423**.

### Named tests (names normative)
`#[cfg(unix)]` after each leftover writer: `metadata.permissions().mode() & 0o777 == 0o600` (same assert as `write_private_agent_home_file_enforces_0600`). Isolate with `APP_HOME_ENV_LOCK` + temp `GROK_APP_HOME`. For `mcp_oauth` (and any path that also writes `user_home()/.grok`), also set `HOME` to a temp dir under that lock — **do not** write the real `~/.grok`.

- `agent_config_edit`: extend `load_and_save_roundtrip_independent` with the Unix 0600 assert on `agent_config_toml()` after `save_agent_config_edit`.
- `agent_privacy`: extend `load_and_save_roundtrip_independent` the same way after `save_privacy_config`.
- `agent_memory_embed`: extend `load_and_save_roundtrip_independent` the same way after `save_memory_embed_config`.
- `agent_codebase_indexing`: add `save_codebase_indexing_enforces_0600` — independent save that writes `agent_config_toml()`, then Unix 0600.
- `permission_rules`: add `save_permission_rules_enforces_0600` — independent `save_permission_rules` that writes agent-home `config.toml`, then Unix 0600. Do not add a shared-mode refuse.
- `mcp_oauth`: add `write_mcp_credentials_full_enforces_0600` — call the private `write_mcp_credentials_full` (same module) and assert 0600 on the agent-home `mcp_credentials.json` (and isolated `HOME/.grok/mcp_credentials.json` if written). Add `persist_oauth_config_toml_enforces_0600`: seed an HTTP MCP server into isolated agent-home `config.toml`, `invalidate_mcp_cache`, call `persist_oauth_tokens`, assert 0600 on that `config.toml`. If `list_mcp_server_defs` prefers a live `grok mcp list` and cannot see the seed, extract a same-module write helper that `:771` calls and assert 0600 on that helper; `persist_oauth_tokens` must use it (not `fs::write`). No network.
- Static: `leftover_n6_writers_do_not_use_bare_fs_write` in one leftover test module (`include_str!` the six Files, strip `#[cfg(test)]` modules). Production text must contain **zero** `fs::write(` and **zero** `std::fs::write(`. Fails if a leftover production write returns.

Do not re-test helper internals. Do not edit `agent_home_config.rs` / `providers.rs`.

`#[tauri::command]` count stays **423**. No file outside Files changes.

Grep (cwd `/workspace`, Proof lists `-n`):
- `rg -n 'fs::write\(' src-tauri/src/agent_config_edit.rs src-tauri/src/agent_privacy.rs src-tauri/src/agent_codebase_indexing.rs src-tauri/src/permission_rules.rs src-tauri/src/agent_memory_embed.rs` → **0**
- `rg -n 'std::fs::write\(' src-tauri/src/mcp_oauth.rs` → **0**
- `rg -n 'write_private_agent_home_file' src-tauri/src/agent_config_edit.rs src-tauri/src/agent_privacy.rs src-tauri/src/agent_codebase_indexing.rs src-tauri/src/permission_rules.rs src-tauri/src/agent_memory_embed.rs src-tauri/src/mcp_oauth.rs` — each of the first five files ≥1; `mcp_oauth.rs` ≥2 (both leftover sites)
- `rg -n 'write_private_agent_home_file' src-tauri/src/providers.rs src-tauri/src/agent_home_config.rs` — still present (Held)
- `rg -n 'fs::write' src-tauri/src/agent_home_config.rs` — production Unix write path is still the helper; the only production `fs::write` is the non-Unix fallback at `:600`. Test fixtures at `:841` / `:849` stay.

## Out
- Rewriting the whole agent-home module. Changing `write_private_agent_home_file` semantics. Touching the non-Unix fallback.
- `update_config_toml` / shared-mode refuse on `permission_rules` or `mcp_oauth`. Changing write destinations.
- Held IDs. Do not reopen P1, P3, P4, P5, R1–R3, R5, R6, S1, S3, C3, D1–D6.
- Slices 08–09: `path_scope.rs` denials, load-replay, docs / i18n / `settingsCatalog` / `store.rs` defaults.
- Additional live `config.toml` writers **not** on the N6 list (residual unless a later authorized slice expands Files): `extensions.rs:1657,1686,1721,2043,2077`; `models_aux.rs:522,553,1463,1472,1492`; `relay_stream_proxy.rs:348`. Isolated `official_aux.rs:143` `agent-home-official/config.toml`. `extensions.rs:669` `extensions.json`.
- New Settings keys, new IPC, publishing / deploying.
- rustfmt-rewrite of the post-06 15-file dirty set. Widening `allow_from`. Changing Ask default.

## Constraints
- **Files:** `src-tauri/src/agent_config_edit.rs`, `src-tauri/src/agent_privacy.rs`, `src-tauri/src/agent_codebase_indexing.rs`, `src-tauri/src/permission_rules.rs`, `src-tauri/src/agent_memory_embed.rs`, `src-tauri/src/mcp_oauth.rs`. Tests stay in those files. A small test-only 0600 assert helper in one of those modules is OK. No `Cargo.toml` / `Cargo.lock`. No `agent_home_config.rs`, `providers.rs`, `lib.rs`, `store.rs`, `path_scope.rs`, docs, i18n, capabilities, App shell.
- Ask remains default. Untrusted projects stay Ask.
- Do not add crates. Disk tests use `APP_HOME_ENV_LOCK` + temp `GROK_APP_HOME` (and temp `HOME` when a writer also touches `user_home()/.grok`). No network. No live `~/.grok` writes.
- App shell freeze: no new `useState` / feature blocks in `App.tsx` / `AppWorkbench.tsx`. Combined line count of those two files must not grow.
- Rust style: new/changed hunks rustfmt-clean. **Do not rustfmt-rewrite pre-existing dirt.** Post-06 dirty set is **15 files**: `agent_home_config.rs`, `batch_agents.rs`, `cli_update.rs`, `mirror/mod.rs`, `mirror/rpc.rs`, `models_aux.rs`, `official_aux.rs`, `path_scope.rs`, `permission.rs`, `relay_stream_proxy.rs`, `secrets.rs`, `serve.rs`, `session_manager/control.rs`, `store.rs`, `wallpaper_source.rs`. `cli_install.rs` is clean. Leftover 06 modules stay clean. The six Files above are not on that list — they must stay fmt-clean.
- Clippy `-D warnings` after this slice: exactly `path_scope.rs:129`, `wecom.rs:210`.
- Do not claim cargo passed unless that session ran it.

## Data / state impact
- No settings / secret-store migration. `store.rs` `permission_policy` default stays **ask**. `session_data_mode` default unchanged (shared).
- No new IPC. Command count **423**.
- On the next leftover save, `config.toml` / `mcp_credentials.json` at those sites are `0600` on Unix. Existing `0644` files are not rewritten until that save.
- Shared-vs-independent policy unchanged: edit/privacy/codebase/memory still refuse shared; `permission_rules` still writes the active GROK_HOME (including `~/.grok` when shared); `mcp_oauth` still dual-writes agent-home and `~/.grok`. Those same call sites also get `0600` when they write `~/.grok`.
- Residual: other production writers of `agent-home/config.toml` (extensions / models_aux / relay_stream_proxy) and `agent-home-official/config.toml` still use umask `fs::write` until a later authorized slice. Coordinator accepted this residual (same class as slice 06 aux global-only YOLO) so the slice stays the N6 six-file leftover list. Locked S2 “all” is Held for the listed leftover sites; extras stay Partial.

## Tests
- `cargo test --manifest-path src-tauri/Cargo.toml --lib agent_config_edit::tests` — existing tests plus Unix 0600 on `load_and_save_roundtrip_independent` and `leftover_n6_writers_do_not_use_bare_fs_write` if hosted here; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib agent_privacy::tests` — existing plus Unix 0600 on the roundtrip; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib agent_codebase_indexing::tests` — existing plus `save_codebase_indexing_enforces_0600`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib permission_rules::tests` — existing plus `save_permission_rules_enforces_0600`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib agent_memory_embed::tests` — existing plus Unix 0600 on the roundtrip; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib mcp_oauth::tests` — existing plus `write_mcp_credentials_full_enforces_0600` and `persist_oauth_config_toml_enforces_0600` (or the extracted write-helper 0600 test); `0 failed`.
- Grep criteria in Done when; each listing + count in Proof.
- Lint non-regression: `cargo fmt --all -- --check` still exits 1 with diffs **only** in the 15-file post-06 set (no new dirty files; Files stay clean; `cli_install.rs` stays clean). `cargo clippy --all-targets -- -D warnings` exactly `path_scope.rs:129`, `wecom.rs:210`.
- Scope: `git diff --stat` vs this slice’s implementation baseline lists only Files.
- No `pnpm vitest` required (no i18n / settings catalog).
- Full `cd src-tauri && cargo test` required at implementation; expected `0 failed` (or the same pre-existing parallel flake outside Files as prior slices, with serial `--test-threads=1` green). Do not claim it passed in this contract.

## Proof
Builder `D07-BUILD-1` (agent `bc-242d40e8-f34a-59bd-8b05-5cea3a17a81b`), implemented in `/workspace` at HEAD `d56a6cbd`, committed by coordinator as `e5600725` (code-only). Candidate identity (clean-tree) `66debe655623292c3aac8fcada12de18a787b5a14a62abe80c4fa79f30024338`. Changed paths: exactly the six Files (+334/−12). Rust `rustc 1.98.1`. Command count 423.

Implementation: seven leftover writes use `write_private_agent_home_file`. Persist uses extracted `write_oauth_config_toml`. Credentials site dropped post-write `set_permissions`. Shared-vs-independent policy unchanged. Extra non-N6 writers residual.

### Done when → evidence
- `rg -n 'fs::write\('` on first five Files → **0**
- `rg -n 'std::fs::write\(' src-tauri/src/mcp_oauth.rs` → **0**
- `write_private_agent_home_file`: first five **1** each; `mcp_oauth.rs` **2** (`:709` helper, `:866` credentials)
- Held helper still in `providers.rs` / `agent_home_config.rs`
- `#[tauri::command]` count **423**

### Tests → evidence
- `agent_config_edit::tests`: existing + Unix 0600 on roundtrip + `leftover_n6_writers_do_not_use_bare_fs_write`
- `agent_privacy::tests` / `agent_memory_embed::tests`: Unix 0600 on roundtrip
- `save_codebase_indexing_enforces_0600`, `save_permission_rules_enforces_0600`
- `write_mcp_credentials_full_enforces_0600`, `persist_oauth_config_toml_enforces_0600` (`seed_visible=true`; persist uses extracted helper)
- Full `cd src-tauri && cargo test`: `1667 passed; 0 failed; 1 ignored`
- `cargo fmt --all -- --check` exit 1; dirty set is the 15-file post-06 list
- Clippy `-D warnings` exit 101 at exactly `path_scope.rs:129`, `wecom.rs:210`

Caveats: residual non-N6 `config.toml` writers unchanged. Coordinator did not re-run the full suite (Reviewer rematches).

## Review
Plan approval: `D07-PLAN-1` APPROVE_PLAN — reviewer `bc-0d024b76-6f0f-5edd-ab46-88a0b7a57f08`, contract `2dec2226…b45c`, candidate `f19f791b…f59c`.
Implementation approval: `D07-IMPL-1` APPROVE_IMPLEMENTATION — reviewer `bc-f9e40781-bd87-527b-8fed-17894beb5281`, contract `2dec2226…b45c`, candidate `66debe65…4338` (code HEAD `e5600725`). Coordinator recomputed identities at consume time: `/workspace` and `/tmp/loop-review/D07-IMPL-1` both HEAD `e2a742f0`, CANDIDATE `66debe655623292c3aac8fcada12de18a787b5a14a62abe80c4fa79f30024338` / CONTRACT `2dec222662a394184a94b5d24fdadee5f3edc1c7462807d0aea72cf34017b45c` / MODE=clean-tree. Counters frozen at 0/0.
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

### D07-PLAN-1 — APPROVE_PLAN (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-0d024b76-6f0f-5edd-ab46-88a0b7a57f08`, worktree `/tmp/loop-review/D07-PLAN-1` @ `7a77390d`.
Contract `2dec2226…b45c` (match). Candidate before/after `f19f791b…f59c` (unchanged, clean-tree). Porcelain empty. Code vs `0b536c3d` is pack-only.
Judgments: (a) BUILD Goal–Tests matches SLICES Now 07 (S2, N6; seven sites / six files); (b) locked constraints present and not weakened; (c) live leftover write-site “today” descriptions accurate; (d) fail-closed / Held IDs / 08–09 Out; (e) one coherent slice with observable tests/greps; (f) extra non-N6 writers residual is within authority (S2 Held = audit leftover list); (g) tests/greps satisfiable without network or live `~/.grok`. No blockers.
Note: if Builder claims `persist_oauth_tokens` 0600 without the helper-extract fallback, require evidence the seed was visible or that `:771` calls the extracted helper.

### D07-IMPL-1 — APPROVE_IMPLEMENTATION (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-f9e40781-bd87-527b-8fed-17894beb5281`, worktree `/tmp/loop-review/D07-IMPL-1` @ `e2a742f0`.
Contract `2dec2226…b45c` (match, before/after). Candidate before/after `66debe65…4338` (unchanged, clean-tree). Porcelain empty. Scope vs `0b536c3d...e5600725`: exactly the six Files.
Every Done when and Tests bullet remapped this session: seven helper writes; persist uses `write_oauth_config_toml` at live `:780`; credentials chmod drop; greps 0/0/1×5+2; named 0600 + static leftover test; extras residual; command count 423; fmt 15-file set; clippy two-site baseline; parallel `1666/1/1` known `terminal_pty` flake outside Files; serial `--test-threads=1` `1667 passed; 0 failed; 1 ignored`. No blockers.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/slice-06-restrict-headless-9f74` off `origin/main` `fbb03fc8`.
Worker / role / phase: Builder / draft-proposal / slice 08
Dispatch ID / launch state / input identity: `D08-DRAFT-1` / launching / candidate `66debe655623292c3aac8fcada12de18a787b5a14a62abe80c4fa79f30024338` (code HEAD `e5600725`), no 08 contract yet (draft)
Pending result / last consumed dispatch: none / `D07-IMPL-1`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir.
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/`.
Baseline snapshot: slice 06 shipped candidate — HEAD `0b536c3d` (code), clean-tree, CANDIDATE `f19f791bfe2e9f89e1de0415c7b89cf9ed3eec9f0c4cbcdae403748c1595f59c`
Contract identity: `2dec222662a394184a94b5d24fdadee5f3edc1c7462807d0aea72cf34017b45c`
Candidate snapshot: HEAD `e5600725` (code commit), clean-tree, CANDIDATE `66debe655623292c3aac8fcada12de18a787b5a14a62abe80c4fa79f30024338`
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events:
- E1 / `D07-PLAN-1` / plan / APPROVE_PLAN / contract `2dec2226…b45c`, candidate `f19f791b…f59c` / no gaps / rejection count 0
- E2 / `D07-IMPL-1` / implementation / APPROVE_IMPLEMENTATION / contract `2dec2226…b45c`, candidate `66debe65…4338` / no gaps / counters frozen: rejections 0, no-progress 0
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: if interrupted before `D08-DRAFT-1` returns, re-dispatch `D08-DRAFT-1` (proposal only, no code). Do not publish.
Advance phase: archive pending then next selected
Next slice ID / draft: 08 (`D08-DRAFT-1` launching)
Environment note: rustc 1.98.1 / webkit2gtk present, same as 02–06.

## Status
Shipped (implementation approved `D07-IMPL-1`; code commit `e5600725`, candidate `66debe65…4338`)

## Next
Coordinator: archive this page to `slices/07-0600-every-agent-home-secret-write.md`, add 07 to SLICES Shipped, select 08 as Now, dispatch Builder draft-proposal for 08 (no code edits), then replace BUILD.md with the 08 Proposed page.

**Resume action:** dispatch `D08-DRAFT-1` (Builder draft-proposal for slice 08 Path scope and silent replay). Do not re-ship 01–07. Do not implement Later-outside work. Do not publish.
