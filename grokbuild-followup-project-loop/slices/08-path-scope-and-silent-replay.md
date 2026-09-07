# BUILD.md

Slice: 08 Path scope and silent replay
Archive: slices/08-path-scope-and-silent-replay.md

## Goal
`path_scope` denies `agent-home/config.toml`, `~/.netrc`, `~/.kube`, `~/.docker/config.json`, and `~/.npmrc` in addition to the existing S4 set. Mirror `session.send` attachment paths pass `path_scope::is_allowed` before they can become `@path` refs. Load-replay permission auto-answer is `Cancelled` unless the tool call id already has a journaled `tool-{id}` row. Ask stays default. Host-computed scope stays Held.

## Done when
Close **S4 leftover, N11, N9** only. Do not reopen Held IDs. Do not implement slice 09. Do not verify live Grok Build `@path` semantics (Out). Do not change `is_session_load_replay_flags` (the idle heuristic stays; only the permission *answer* changes).

Locked family (copy; do not invent a new deny set or a second attachment filter). Verify live; do not trust stale numbers blindly. Code inspect HEAD `e5600725`:

**S4 leftover — extra deny paths** (`src-tauri/src/path_scope.rs:94–135` today)

Extend `is_denied_target` only (same host function `is_allowed` / `require_allowed` already consult). Match existing style: path components / filename, **not** “must live under real `$HOME`”. Do **not** call `user_home()` and do **not** write the operator’s real home secret files in tests.

| Target | Rule (exact) |
|---|---|
| `agent-home/config.toml` | filename `config.toml` **and** parent directory component is `agent-home` (last two components). Do **not** deny every `config.toml`. Do **not** deny `agent-home-official/config.toml` or `.grok/config.toml`. |
| `~/.netrc` | filename `.netrc` (anywhere, like `secrets.json`) |
| `~/.kube` | directory component `.kube` (anywhere, like `.ssh`) — covers `~/.kube/config` and the directory itself |
| `~/.docker/config.json` | filename `config.json` **and** parent directory component is `.docker`. Do **not** deny all of `.docker` (e.g. `.docker/daemon.json` stays not-denied). |
| `~/.npmrc` | filename `.npmrc` (anywhere) |

Existing denies stay. `grant_path` must still fail to unlock a denied target (`is_allowed` checks deny first — do not change that order).

Clippy site `path_scope.rs:129` (`manual_contains` on `comps.iter().any(|c| *c == "remote-im")`): rewrite that check to `comps.contains(&"remote-im")` (or equivalent). Use `contains` for any new single-value component check so this slice does **not** add a new `manual_contains`. Post-08 `cargo clippy --all-targets -- -D warnings`: exactly `wecom.rs:210`. Drop `path_scope.rs:129` from the baseline. Do not edit `wecom.rs`.

**N11 — mirror attachment paths** (`src-tauri/src/mirror/rpc.rs:470–516` today)

In `param_attachments`, after a non-empty `path` is parsed and **before** `out.push`, skip the item unless `crate::path_scope::is_allowed(std::path::Path::new(path))`. Keep the stored path string as the client sent it (no canonicalize rewrite). Empty/missing path stays `continue`. If every item is skipped, return `None` (same as today). Do **not** fail the whole `session.send`. Optional `tracing::warn!` on skip is OK. Do **not** filter `append_journal_attachment_refs` / desktop `session.send`.

**N9 — load-replay permission answer** (`src-tauri/src/session_manager/events.rs:322–345` today; gate `stream.rs:148–158`)

Keep `is_session_load_replay` / `is_session_load_replay_flags` byte-semantics identical. In the `PermissionRequest` replay arm only:

- If `tool_call_id` is empty **or** the App journal has no row with `id == "tool-{tool_call_id}"` and `role == "tool"` → `respond_permission(rpc_id, PermissionOutcome::Cancelled)`.
- If that journal row exists → keep today’s `allow_once` coerce (`coerce_wire_option_id_for_tool("allow_once", …)` + `PermissionOutcome::Selected`). That is the SLICES “unless journaled” exception.
- Empty `acp` still returns without surfacing UI.

Extract two `pub(super)` helpers on `SessionManager` in `events.rs` (names normative):

- `journal_has_tool_call_id(app_session_id: &str, tool_call_id: &str) -> bool` — false when either string is empty; otherwise `store::load_messages` has `id == format!("tool-{tool_call_id}")` and `role == "tool"`. Do **not** use `journal_terminal_tool_ids` (terminal-status only).
- `load_replay_permission_action(journaled: bool) -> LoadReplayPermissionAction` where the enum is `Cancel` \| `AllowOnce` (`Cancel` iff `!journaled`). The replay arm must call this helper (or equivalent `if journaled` that matches it) and map `Cancel` → `PermissionOutcome::Cancelled`.

Do not change `events_bg.rs`, `may_auto_allow`, or the live (non-replay) permission path.

`#[tauri::command]` count stays **423**. No file outside Files changes.

### Named tests (names normative)

**`path_scope.rs`**

- `denies_s4_leftover_targets_even_under_allowed_roots` — same `with_isolated_roots` / temp project+app pattern as `denies_sensitive_targets_even_under_allowed_roots`. Create dummy files **only** under that temp tree (never `user_home()` / real `~/.netrc` / `~/.kube` / `~/.docker` / `~/.npmrc`):
  - `app/agent-home/config.toml`
  - `project/.netrc`
  - `project/.kube/config`
  - `project/.docker/config.json`
  - `project/.npmrc`
  - controls: `project/config.toml` (allowed); `project/.docker/daemon.json` (not denied by the docker rule)
  - Assert `is_denied_target` + `!is_allowed` for each of the five leftover targets (including `.kube` directory if you also create it).
  - Assert `is_allowed(&project/config.toml)`.
  - `grant_path` on `app/agent-home/config.toml` then still `!is_allowed`.
- Existing `denies_sensitive_targets_even_under_allowed_roots` still passes (regression). Do not rustfmt-rewrite its dirty asserts; new asserts in the new test must be written already rustfmt-clean.

**`mirror/rpc.rs`**

- `param_attachments_drops_disallowed_paths` — same-module call to `param_attachments`. Hold `path_scope::TEST_LOCK` if the test grants or refreshes roots.
  - Denied existing S4 (e.g. a `.ssh/...` path) and one new leftover (e.g. `.netrc`) are absent from the result.
  - Out-of-roots path (e.g. `/etc/passwd`, no grant) is absent (`is_allowed`, not only `is_denied_target`).
  - A granted temp file that is not a denied target is kept.
  - Mixed list → only the allowed item.
  - All-disallowed list → `None`.
  - Do not write real home secret files.

**`session_manager/events.rs`** (`#[cfg(test)] mod tests` at end of file)

- `load_replay_auto_answer_is_cancelled_unless_journaled` (audit accept-when: replay test asserts `Cancelled`):
  - `load_replay_permission_action(false) == Cancel`
  - `load_replay_permission_action(true) == AllowOnce`
  - `journal_has_tool_call_id("", "x")` and `journal_has_tool_call_id("sid", "")` are false
  - Isolated `APP_HOME_ENV_LOCK` + temp `GROK_APP_HOME` + `ensure_app_dirs`: `append_message` a `role: "tool"`, `id: "tool-hist-1"` row; `journal_has_tool_call_id(sid, "hist-1")` is true; unknown id is false. Restore env; do not write real `~/.grok`.
- Existing `session_manager::routing_tests::session_load_replay_gate_matches_prompt_in_flight` still passes (do not edit `routing_tests.rs` / `stream.rs`).

Existing `path_scope` / `mirror::rpc` tests still pass.

Grep (cwd `/workspace`, Proof lists `-n`):

- `rg -n 'fn is_denied_target' -A 80 src-tauri/src/path_scope.rs` — production body (before `fn is_allowed`) contains `.netrc`, `.kube`, `.docker`, `.npmrc`, and `config.toml` / `agent-home`.
- `rg -n 'path_scope::is_allowed' src-tauri/src/mirror/rpc.rs` → **≥1**, inside `param_attachments`.
- `rg -n 'PermissionOutcome::Cancelled' src-tauri/src/session_manager/events.rs` → **≥1** in the `PermissionRequest` load-replay arm.
- `rg -n 'load_replay_permission_action' src-tauri/src/session_manager/events.rs` → **≥2** (definition + replay arm).
- `rg -n 'journal_has_tool_call_id' src-tauri/src/session_manager/events.rs` → **≥2** (definition + replay arm or the action helper’s caller).
- `rg -n 'is_session_load_replay_flags' src-tauri/src/session_manager/stream.rs` — still `!prompt_in_flight && !deferred_prompt_complete`.
- `rg -c '#\[tauri::command\]' src-tauri/src` → **423**.

## Out
- Verifying live Grok Build `@path` semantics.
- Slice 09 docs / i18n / `settingsCatalog` / `settings-remoteIm.ts` / `README_EN.md` / `SECURITY.md` / `docs/features/remote-security.md`.
- Held IDs. Do not reopen P1, P3, P4, P5, R1–R3, R5, R6, S1, S3, C3, D1–D6. Do not weaken host-computed scope (P4): deny/allow stay on host `path_scope`; no client “already allowed” flag; do not skip deny after `grant_path`.
- Widening `allow_from`. Changing Ask default / `store.rs` `permission_policy`. New Settings keys, new IPC, publishing / deploying.
- Filtering desktop `session.send` / `append_journal_attachment_refs` / `commands/session_p1.rs`. Residual: those paths stay unfiltered; N11 is mirror `param_attachments` only.
- Denying `.grok/config.toml`, `agent-home-official/config.toml`, all `config.toml`, or all of `.docker`. Residual unless a later authorized slice expands Files.
- Changing `is_session_load_replay` / `is_session_load_replay_flags` / plan / ask_user replay gates. Changing `events_bg.rs` or `may_auto_allow`.
- Editing `stream.rs` (including the stale `:184–185` comment). Residual N12-class.
- rustfmt-rewrite of the 15-file post-06 dirty set. Touching `wecom.rs`. Adding crates. Editing `lib.rs` / `Cargo.toml` / `Cargo.lock` / capabilities / App shell.

## Constraints
- **Files:** `src-tauri/src/path_scope.rs`, `src-tauri/src/mirror/rpc.rs`, `src-tauri/src/session_manager/events.rs`. Tests stay in those files. No `Cargo.toml` / `Cargo.lock`. No `store.rs`, `lib.rs`, `stream.rs`, `types.rs`, `events_bg.rs`, `control.rs`, `routing_tests.rs`, docs, i18n, capabilities, App shell.
- Ask remains default. Untrusted projects stay Ask.
- Do not add crates. Disk/path tests use temp dirs + `path_scope::TEST_LOCK` / `APP_HOME_ENV_LOCK` + temp `GROK_APP_HOME`. No network. No live `~/.netrc` / `~/.kube` / `~/.docker/config.json` / `~/.npmrc` / real `~/.grok` writes.
- App shell freeze: no new `useState` / feature blocks in `App.tsx` / `AppWorkbench.tsx`. Combined line count of those two files must not grow.
- Rust style: new/changed hunks rustfmt-clean. **Do not rustfmt-rewrite pre-existing dirt.** Post-06 dirty set is **15 files**: `agent_home_config.rs`, `batch_agents.rs`, `cli_update.rs`, `mirror/mod.rs`, `mirror/rpc.rs`, `models_aux.rs`, `official_aux.rs`, `path_scope.rs`, `permission.rs`, `relay_stream_proxy.rs`, `secrets.rs`, `serve.rs`, `session_manager/control.rs`, `store.rs`, `wallpaper_source.rs`. `path_scope.rs` and `mirror/rpc.rs` are already dirty: new hunks clean, leftover dirt untouched. `events.rs` is **not** on that list — it must stay `rustfmt --edition 2021 --check` clean (do not add it to the dirty set).
- Clippy `-D warnings` after this slice: exactly `wecom.rs:210` (`too_many_arguments`). `path_scope.rs:129` (`manual_contains`) is **dropped** by the required `contains` rewrite. Zero new clippy findings in Files.
- Do not claim cargo passed unless that session ran it.

## Data / state impact
- No settings / secret-store migration. `store.rs` `permission_policy` default stays **ask**. `session_data_mode` default unchanged (shared).
- No new IPC. Command count **423**.
- Host `path_scope` denials apply to every existing `is_allowed` / `require_allowed` caller (media HTTP, `fs_read_absolute`, etc.) for the five leftover names — that is the S4 leftover closing, not a new surface.
- Mirror `session.send` attachments that fail `is_allowed` never reach the journal `@path` dual-write. Text of the send still goes through.
- Load-replay `request_permission` with an unknown / empty tool id is `Cancelled` (agent unblocked, not silently allowed). Journaled historical `tool-{id}` rows still get `allow_once` coerce.
- Residual: desktop attachments; `.grok` / `agent-home-official` `config.toml`; stale `stream.rs` permission-replay comment; background `may_auto_allow` (policy, not N9).

## Tests
- `cargo test --manifest-path src-tauri/Cargo.toml --lib path_scope::tests` — existing plus `denies_s4_leftover_targets_even_under_allowed_roots`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib mirror::rpc::tests` — existing plus `param_attachments_drops_disallowed_paths`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib session_manager::events::tests` — `load_replay_auto_answer_is_cancelled_unless_journaled`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib session_manager::routing_tests::session_load_replay_gate_matches_prompt_in_flight` — still passes (gate unchanged).
- Grep criteria in Done when; each listing + count in Proof.
- Lint non-regression: `cargo fmt --all -- --check` still exits 1 with diffs **only** in the 15-file post-06 set (no new dirty files; `events.rs` stays clean). `rustfmt --edition 2021 --check src-tauri/src/session_manager/events.rs` exit 0. `cargo clippy --all-targets -- -D warnings` exactly `wecom.rs:210`.
- Scope: `git diff --stat` vs this slice’s implementation baseline lists only Files.
- No `pnpm vitest` required (no i18n / settings catalog).
- Full `cd src-tauri && cargo test` required at implementation; expected `0 failed` (or the same pre-existing parallel flake `terminal_pty_spawn_rejects_untrusted_project_path` outside Files, with serial `--test-threads=1` green). Do not claim it passed in this contract.

## Proof
Builder `D08-BUILD-1` (agent `bc-6ebcdb83-d1d9-51ed-93d8-52d4c5d79d83`), implemented in `/workspace` at HEAD `c1afe7a6`, committed by coordinator as `62f55936` (code-only). Candidate identity (clean-tree) `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db`. Changed paths: exactly the three Files (+279/−16). Rust `rustc 1.98.1`. Command count 423.

Implementation: leftover deny paths in `is_denied_target`; `param_attachments` requires `path_scope::is_allowed`; load-replay permission is `Cancelled` unless journaled `tool-{id}`. `remote-im` check uses `contains`. Ask stays default.

### Done when → evidence
- `is_denied_target` body contains `.netrc`, `.kube`, `.docker`, `.npmrc`, `config.toml` / `agent-home`
- `path_scope::is_allowed` in `rpc.rs` `:488` inside `param_attachments`
- `PermissionOutcome::Cancelled` in replay arm; `load_replay_permission_action` + `journal_has_tool_call_id` used
- `is_session_load_replay_flags` unchanged
- `#[tauri::command]` count **423**

### Tests → evidence
- `path_scope::tests` including `denies_s4_leftover_targets_even_under_allowed_roots`
- `mirror::rpc::tests` including `param_attachments_drops_disallowed_paths`
- `load_replay_auto_answer_is_cancelled_unless_journaled`
- routing gate test still passes
- Full `cd src-tauri && cargo test`: `1670 passed; 0 failed; 1 ignored`
- fmt dirty set still the 15-file post-06 list; `events.rs` rustfmt-check exit 0
- Clippy `-D warnings` exactly `wecom.rs:210`

Caveats: desktop attachments residual; coordinator did not re-run the full suite (Reviewer rematches).

## Review
Plan approval: `D08-PLAN-1` APPROVE_PLAN — reviewer `bc-c5faeb80-ad97-5c17-9c77-a332e9e34c0b`, contract `9083988b…0dfe`, candidate `66debe65…4338`.
Implementation approval: `D08-IMPL-1` APPROVE_IMPLEMENTATION — reviewer `bc-835bb99a-0b72-549d-8510-4c668a43a75a`, contract `9083988b…0dfe`, candidate `87743f68…82db` (code HEAD `62f55936`). Coordinator recomputed identities at consume time: `/workspace` and `/tmp/loop-review/D08-IMPL-1` both HEAD `39c49440`, CANDIDATE `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db` / CONTRACT `9083988b9b74d5d955ac1db775093d01ca8968afd1b917b4ac6f7e21805e0dfe` / MODE=clean-tree. Counters frozen at 0/0.
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

### D08-PLAN-1 — APPROVE_PLAN (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-c5faeb80-ad97-5c17-9c77-a332e9e34c0b`, worktree `/tmp/loop-review/D08-PLAN-1` @ `b3b7edaa`.
Contract `9083988b…0dfe` (match). Candidate before/after `66debe65…4338` (unchanged, clean-tree). Porcelain empty. Code vs `e5600725` is pack-only.
Judgments: (a) BUILD matches SLICES Now 08 (S4 leftover, N11, N9); (b) P4 / Ask / journaled exception not weakened; (c) live “today” descriptions accurate; (d) 09 / Held / desktop attachments / stream.rs Out; (e) one coherent slice; (f) clippy plan drops `path_scope.rs:129` via `contains`, keeps `wecom.rs:210`; (g) tests without real home secrets. No blockers.

### D08-IMPL-1 — APPROVE_IMPLEMENTATION (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-835bb99a-0b72-549d-8510-4c668a43a75a`, worktree `/tmp/loop-review/D08-IMPL-1` @ `39c49440`.
Contract `9083988b…0dfe` (match). Candidate before/after `87743f68…82db` (unchanged, clean-tree). Porcelain empty. Scope vs `e5600725...62f55936`: exactly the three Files.
Every Done when and Tests bullet remapped: leftover denies exact; attachments filtered; replay Cancelled unless journaled; named tests green; fmt 15-file set; clippy exactly `wecom.rs:210`; full `cargo test` `1670 passed; 0 failed; 1 ignored`. No blockers.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/slice-06-restrict-headless-9f74` off `origin/main` `fbb03fc8`.
Worker / role / phase: Builder / draft-proposal / slice 09
Dispatch ID / launch state / input identity: `D09-DRAFT-1` / launching / candidate `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db` (code HEAD `62f55936`), no 09 contract yet (draft)
Pending result / last consumed dispatch: none / `D08-IMPL-1`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir.
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/`.
Baseline snapshot: slice 07 shipped candidate — HEAD `e5600725` (code), clean-tree, CANDIDATE `66debe655623292c3aac8fcada12de18a787b5a14a62abe80c4fa79f30024338`
Contract identity: `9083988b9b74d5d955ac1db775093d01ca8968afd1b917b4ac6f7e21805e0dfe`
Candidate snapshot: HEAD `62f55936` (code commit), clean-tree, CANDIDATE `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db`
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events:
- E1 / `D08-PLAN-1` / plan / APPROVE_PLAN / contract `9083988b…0dfe`, candidate `66debe65…4338` / no gaps / rejection count 0
- E2 / `D08-IMPL-1` / implementation / APPROVE_IMPLEMENTATION / contract `9083988b…0dfe`, candidate `87743f68…82db` / no gaps / counters frozen: rejections 0, no-progress 0
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: if interrupted before `D09-DRAFT-1` returns, re-dispatch `D09-DRAFT-1` (proposal only, no code). Do not publish.
Advance phase: archive written; next selected
Next slice ID / draft: 09 (`D09-DRAFT-1` launching)
Environment note: rustc 1.98.1 / webkit2gtk present, same as 02–07.

## Status
Shipped (implementation approved `D08-IMPL-1`; code commit `62f55936`, candidate `87743f68…82db`)

## Next
Coordinator: archive this page to `slices/08-path-scope-and-silent-replay.md`, add 08 to SLICES Shipped, select 09 as Now, dispatch Builder draft-proposal for 09 (no code edits), then replace BUILD.md with the 09 Proposed page.

**Resume action:** dispatch `D09-DRAFT-1` (Builder draft-proposal for slice 09 Docs match the leftover posture). Do not re-ship 01–08. Do not implement Later-outside work. Do not publish.
