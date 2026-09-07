# BUILD.md

Slice: 06 Restrict leftover headless children
Archive: slices/06-restrict-leftover-headless-children.md

## Goal
Leftover headless children `session_title`, `agent_workflows`, and `streaming_messages_json` use the same restricted-child contract as `official_aux`: `--no-subagents`, `--disallowed-tools` containing the official_aux comma list, pinned cwd (never the app process cwd), and `--always-approve` only when the **invoking session’s effective** policy is YOLO (`PermissionPolicy::AlwaysApprove`). Batch headless uses that invoking session, not `sessions.first()`. Fail closed: no / unknown session / untrusted project → Ask → no `--always-approve`. Interactive parent-session subagents stay on. Ask stays default.

## Done when
Close **P2 leftover, N4, N5** only. Do not reopen Held IDs. Do not implement slices 07–09. Do not change `official_aux` / `models_aux` / `wallpaper_source` policy source (audit residual: those still resolve YOLO from **global** only — Out).

Locked family (copy; do not invent a new tool list). Official_aux value at `official_aux.rs:224`:

`run_terminal_cmd,run_terminal_command,search_replace,write,Agent,spawn_subagent,bash,bash_tool`

Every leftover `--disallowed-tools` value **must contain each of those tokens**. Do not shrink that set. Extra denials already on `session_title` (`web_search`, `web_fetch` + `--disable-web-search`) **stay** (title is text-only). Do not add `workflow` to the list (`agent_workflows` must still be able to call the `workflow` tool).

`--always-approve` only when `effective_permission_policy(global, project_trusted, project_policy, session_policy)` is `AlwaysApprove` for a **found** invoking session. Missing/blank/unknown `session_id` → treat as Ask → **no** `--always-approve` (do **not** fall through to global YOLO; that is the official_aux residual, out of scope here). `project_trusted == Some(false)` → Ask (existing helper). Ask / AcceptEdits / DontAsk / Deny / Auto / missing → no `--always-approve`.

Do not use `sessions.first()`. Do not use `session.permission_policy` alone when a project override or untrusted project would change the effective tier.

### N4 — leftover children: restricted argv, pinned cwd, conditional approve

**`session_title.rs`** (today `:178–196`: `--always-approve` unconditional; `--no-subagents` + **superset** disallowed list; **no** `current_dir`; `llm_title_via_cli` has no session id; `auto_title_session_fast(id, …)` / `refine_title_in_background(…, id, …)` have one)

- Extract a pure args builder (family of `build_official_aux_args`), e.g. `title_headless_args(prompt, is_yolo) -> Vec<String>` (name may vary; tests call it). Include existing `-p` / `--effort low` / `--max-turns 2` / `--no-subagents` / `--disable-web-search` / `--disallowed-tools` (keep extras). Push `--always-approve` only if `is_yolo`.
- `llm_title_via_cli` takes the invoking session id. Resolve YOLO via `effective_permission_policy` for that id + its project + global; unknown/missing id → Ask.
- Pin `current_dir` to `std::env::temp_dir()` (audit N4). Never inherit the app cwd. Never use a project path for the title child.
- `auto_title_session_fast` / IPC `session_auto_title` signatures stay; refine already has `id` — pass it through.

**`agent_workflows.rs`** (today `:490–507` `workflow_run_args`: `--always-approve` unconditional, **no** `--no-subagents` / `--disallowed-tools`; spawn `:714–716` cwd = project if dir else temp; test `run_args_include_plain_and_approve` at `:1040` **asserts presence**; `run_workflow*` have no session id)

- Change `workflow_run_args` to take an `is_yolo: bool` (or equivalent policy flag). Always emit `--no-subagents` and `--disallowed-tools` with the official_aux comma list. Push `--always-approve` only if `is_yolo`. Keep `--max-turns` 4 validate / 8 launch, `--effort low`, `--output-format plain`.
- **Flip** `run_args_include_plain_and_approve` (`:1040`): Ask (`is_yolo = false`) asserts **absence** of `--always-approve`. Same test (or a sibling in the official_aux family) still asserts plain / turn counts.
- Thread optional `session_id` through `run_workflow`, `run_workflow_with_app`, `run_workflow_inner`, and IPC `workflows_run`. Spawn site computes `is_yolo` from the invoking session. Omit/unknown → Ask.
- Cwd stays project-if-real-dir else `std::env::temp_dir()` (`:698–703`). Never inherit the app cwd. Do not force-temp when `project_path` is a real directory (workflow tool needs the project).

**`streaming_messages_json.rs`** (today `:181–195`: `--always-approve` unconditional; no `--no-subagents` / `--disallowed-tools`; cwd already `std::env::temp_dir()`. Fixed-prompt capability probe; no session)

- Extract `streaming_probe_args(include_partial, is_yolo) -> Vec<String>` (name may vary). Always `--no-subagents` + official_aux `--disallowed-tools`. Push `--always-approve` only if `is_yolo`. Keep `-p` `PROBE_PROMPT`, `--max-turns 1`, `--effort low`, `--output-format` `OUTPUT_FORMAT`, optional `--include-partial-messages`.
- Spawn site always passes `is_yolo = false` (no session → Ask). Keep cwd `std::env::temp_dir()`. Do not add `session_id` to `probe_streaming_messages_json` / `streaming_messages_json_probe`.

### N5 — batch uses invoking session, not `sessions.first()`

**`batch_agents.rs`** (today `:171–183` `run_batch_headless` uses `sessions.first()` then `s.permission_policy`; `batch_headless_args` already gates YOLO and already has the official_aux tool list; `:79` is `map_or(false,` — clippy `unnecessary_map_or`)

- `run_batch_headless` takes `session_id: Option<&str>` (or equivalent). Look up **that** id in the session index. Resolve **effective** policy (session + its project + global). Do **not** call `sessions.first()`.
- Missing/blank/unknown `session_id` → Ask args (still **run** the child if path/prompt are valid). Do not refuse with `no_session` solely because id was omitted; do not revive `sessions.first()` as a fallback.
- Keep `batch_headless_args(prompt, parent_policy: Option<&str>)`. The string passed in is the **effective** policy (`PermissionPolicy::as_str()`, e.g. `always_approve` / `ask`), not a raw index-first field. `None` / `ask` → no `--always-approve`.
- Fix `:79` `map_or(false, …)` (e.g. `is_some_and`) so the clippy `-D warnings` baseline **drops this site**. Do not touch `path_scope.rs:129` or `wecom.rs:210`.

**IPC + FE thread**

- `commands/misc_p1.rs:1271` `batch_agents_headless(project_path, prompt, timeout_ms)` gains optional `session_id: Option<String>`. Old callers that omit it fail closed / Ask, not `sessions.first()`.
- `src/lib/api/voice.ts:121` `batchAgentsHeadless` gains optional `sessionId`.
- `AppWorkbench.tsx` existing `api.batchAgentsHeadless({…})` (`:11218`) passes the focused `session.sessionId` when present. **No new `useState` / feature block** in `App.tsx` / `AppWorkbench.tsx` (growth freeze): one field on an existing invoke only.
- `commands/worktree_agents_p1.rs:146` `workflows_run` gains optional `session_id`. `src/lib/api/agents.ts` `workflowsRun` gains optional `sessionId`. Settings `WorkflowsDiscoveryBlock` / `WorkflowsSettingsBlock` has no session today — **omit** (Ask). Do not plumb a new Settings session field through App shell.

`#[tauri::command]` count stays **423**. Prefer optional args on existing commands; do not add a new command.

### Interactive parent stays ON
Do not add `--no-subagents` to the interactive parent ACP spawn (`acp_client.rs` `apply_subagents_to_command(&mut cmd, subagents_enabled)`). Out: “Disabling parent-session subagents.”

### Named tests (names normative)
Family of `official_aux_args_restricted_and_conditional_always_approve` (`official_aux.rs:2579`): each leftover args builder asserts Ask vs YOLO **separately** — `--no-subagents` present both; official_aux tokens present in `--disallowed-tools` value both; `--always-approve` absent on Ask, present on YOLO.

- `session_title`: `title_args_restricted_and_conditional_always_approve` — Ask vs YOLO; official_aux tokens; extras `web_search` / `web_fetch` still in the value; `--disable-web-search` still present.
- `agent_workflows`: **flip** `run_args_include_plain_and_approve` so Ask asserts **absence** of `--always-approve`; extend that test or add `workflow_run_args_restricted_and_conditional_always_approve` for family flags + YOLO presence. Keep plain / `4` / `8` asserts.
- `streaming_messages_json`: `streaming_probe_args_restricted_and_conditional_always_approve` — builder Ask vs YOLO (spawn site still always Ask).
- `batch_agents`: keep `args_restricted_in_ask_and_always_approve_in_yolo`. Add `batch_invoking_session_not_index_first`: two in-memory sessions (first in index YOLO `always_approve`, invoking id Ask) → resolved effective policy is Ask → `batch_headless_args` has **no** `--always-approve`. Also: unknown id → Ask; invoking YOLO + `project_trusted = false` → Ask; invoking YOLO + trusted / no project → YOLO. Do not load the live session store for this test (pure lookup over injected slices).

Title cwd: test or grep that the title spawn sets `current_dir` to `std::env::temp_dir()` (or a helper that returns that path).

`#[tauri::command]` count stays **423**. No file outside Files changes.

Grep (cwd `/workspace`, Proof lists `-n`):
- `rg -n 'sessions\.first\(\)' src-tauri/src/batch_agents.rs` → **0**
- `rg -n '--always-approve' src-tauri/src/session_title.rs src-tauri/src/agent_workflows.rs src-tauri/src/streaming_messages_json.rs` — only inside `is_yolo` / YOLO branches of the args builders (and the YOLO half of tests), **not** unconditional `.arg("--always-approve")` / `vec![… "--always-approve" …]`
- Official_aux tokens appear in each leftover `--disallowed-tools` value (`run_terminal_cmd`, `run_terminal_command`, `search_replace`, `write`, `Agent`, `spawn_subagent`, `bash`, `bash_tool`)
- `rg -n 'current_dir' src-tauri/src/session_title.rs` — spawn pins temp (not absent)
- `rg -n '--no-subagents' src-tauri/src/acp_client.rs` — no new parent-path disable (existing `apply_subagents_to_command` / setting-gated helper only)
- `rg -n 'effective_permission_policy\(' src-tauri/src/official_aux.rs src-tauri/src/models_aux.rs src-tauri/src/wallpaper_source.rs` — still global-only (`None, None, None`); this slice does not change those call sites

## Out
- Disabling parent-session subagents. Do not add `--no-subagents` to the interactive ACP parent spawn.
- Changing `official_aux` / `models_aux` / `wallpaper_source` policy source (global vs invoking session). Residual: an Ask session under a YOLO-global install still gets YOLO aux children. Not this slice.
- Expanding `--disallowed-tools` to host `is_edit_tool` ids (`apply_patch`, `create_file`, …). CLI honour of `--no-subagents` / `--disallowed-tools` remains Unverified.
- Held IDs. Do not reopen P1, P3, P4, P5, R1, R2, R3, R5, R6, S1, S3, C3, D1–D6.
- Slices 07–09: `agent_home_config.rs`, `path_scope.rs`, docs / i18n / `settingsCatalog` / `store.rs` defaults.
- New Settings keys, capabilities edits (unless a compile error from the optional IPC field requires a documented signature-only change), publishing / deploying.
- rustfmt-rewrite of the post-05 dirty set. Widening `allow_from`. Changing Ask default.

## Constraints
- **Files:** `src-tauri/src/session_title.rs`, `src-tauri/src/agent_workflows.rs`, `src-tauri/src/streaming_messages_json.rs`, `src-tauri/src/batch_agents.rs`, IPC `src-tauri/src/commands/misc_p1.rs`, `src-tauri/src/commands/worktree_agents_p1.rs`, FE `src/lib/api/voice.ts`, `src/lib/api/agents.ts`, and the existing `AppWorkbench.tsx` `batchAgentsHeadless` invoke (sessionId pass-through only). A small fail-closed lookup helper + its unit test may live in `batch_agents.rs` (preferred) or as a thin wrapper next to `effective_permission_policy` in `permission.rs` if that avoids duplication — do **not** change `store.rs` defaults or `resolve_composer_prefs` fall-through-to-global. No `Cargo.toml` / `Cargo.lock`. No `agent_home_config.rs`, `path_scope.rs`, docs, i18n, capabilities, `lib.rs` handler list (count stays 423).
- Ask remains default. Untrusted projects stay Ask.
- Do not add crates. Args-builder tests are pure (no CLI spawn, no network).
- App shell freeze: no new `useState` / feature blocks in `App.tsx` / `AppWorkbench.tsx`. Combined line count of those two files must not grow except the `sessionId` field on the existing batch invoke (keep the delta to that call).
- Rust style: new/changed hunks rustfmt-clean. **Do not rustfmt-rewrite pre-existing dirt.** Post-05 dirty set is the previous 16 minus `cli_install.rs` (**15 files**): `agent_home_config.rs`, `batch_agents.rs`, `cli_update.rs`, `mirror/mod.rs`, `mirror/rpc.rs`, `models_aux.rs`, `official_aux.rs`, `path_scope.rs`, `permission.rs`, `relay_stream_proxy.rs`, `secrets.rs`, `serve.rs`, `session_manager/control.rs`, `store.rs`, `wallpaper_source.rs`. `session_title.rs` / `agent_workflows.rs` / `streaming_messages_json.rs` / IPC files are not on that list — they must stay fmt-clean (do not add them to the dirty set). `batch_agents.rs` / `permission.rs` (if touched) are already dirty: new hunks clean, leftover dirt untouched.
- Clippy `-D warnings` after this slice: exactly `path_scope.rs:129`, `wecom.rs:210` (baseline minus `batch_agents.rs:79`).
- Do not claim cargo passed unless that session ran it.

## Data / state impact
- No settings / secret-store migration. `store.rs` `permission_policy` default stays **ask**.
- No new IPC commands. Optional `session_id` / `sessionId` on `batch_agents_headless` and `workflows_run` only. Omitted id → Ask child (no `--always-approve`).
- Title refine still uses the session id it already has. Streaming probe stays session-less → Ask.
- Settings workflow run without a session id stays Ask (Settings block has no session today).
- Batch from the workbench passes the focused session id when present so a YOLO invoking session still gets `--always-approve`; an Ask invoking session does not inherit YOLO from an older index-first row.
- Parent interactive sessions unchanged (subagents still follow `subagents_enabled`).
- Residual: `--no-subagents` on the workflow **child** may limit nested Grok subagents inside the `workflow` tool. Locked. Interactive `/workflow` on the parent is unchanged.

## Tests
- `cargo test --manifest-path src-tauri/Cargo.toml --lib session_title::tests` — existing title tests plus `title_args_restricted_and_conditional_always_approve`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib agent_workflows::tests` — flipped `run_args_include_plain_and_approve` (Ask absence) plus family/YOLO coverage; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib streaming_messages_json::tests` — existing probe tests plus `streaming_probe_args_restricted_and_conditional_always_approve`; `0 failed`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib batch_agents::tests` — existing args/soft-fail tests plus `batch_invoking_session_not_index_first`; `0 failed`.
- Grep criteria in Done when; each listing + count in Proof.
- Lint non-regression: `cargo fmt --all -- --check` still exits 1 with diffs **only** in the 15-file post-05 set (no new dirty files; `cli_install.rs` stays clean). `cargo clippy --all-targets -- -D warnings` exactly `path_scope.rs:129`, `wecom.rs:210`.
- Scope: `git diff --stat` vs this slice’s implementation baseline lists only Files.
- No `pnpm vitest` required (no new i18n / settings catalog). Implementation Proof runs the four lib test filters above.
- Full `cd src-tauri && cargo test` required at implementation; expected `0 failed` (or the same pre-existing parallel flake outside Files as prior slices, with serial `--test-threads=1` green). Do not claim it passed in this contract.

## Proof
Builder `D06-BUILD-1` (agent `bc-cdd8fe35-4a90-5ed7-850f-c8d66dbcf15c`), implemented in `/workspace` at HEAD `6444272b`, committed by coordinator as `0b536c3d` (code-only). Candidate identity (clean-tree) `f19f791bfe2e9f89e1de0415c7b89cf9ed3eec9f0c4cbcdae403748c1595f59c`. Changed paths: exactly the nine Files-list paths (+542/−135). Rust `rustc 1.98.1`. Command count 423.

Implementation: leftover children use official_aux `--no-subagents` / `--disallowed-tools`; `--always-approve` only if invoking-session effective policy is YOLO; title cwd is temp; batch looks up the invoking session id (no `sessions.first()`). Missing/unknown id → Ask and still runs. Settings `workflowsRun` omits sessionId (Ask). Parent ACP subagents unchanged.

### Done when → evidence
- `rg -n 'sessions\.first\(\)' src-tauri/src/batch_agents.rs` → **0**
- `--always-approve` in leftover files only inside `if is_yolo { args.push(...) }` plus Ask-absence / YOLO-presence asserts. No unconditional `.arg` / `vec![… "--always-approve" …]`
- Official_aux tokens present in `TITLE_DISALLOWED_TOOLS` (plus `web_search`/`web_fetch`), `WORKFLOW_DISALLOWED_TOOLS`, `STREAMING_DISALLOWED_TOOLS`, `batch_headless_args`
- `rg -n 'current_dir' src-tauri/src/session_title.rs` → `:209` `cmd.current_dir(title_child_cwd())` (temp)
- `rg -n '--no-subagents' src-tauri/src/acp_client.rs` → **0** (parent still `apply_subagents_to_command`)
- Aux residuals still `effective_permission_policy(..., None, None, None)` in official_aux / models_aux / wallpaper_source
- `#[tauri::command]` count **423**

### Tests → evidence
- `session_title::tests`: `13 passed; 0 failed` (`title_args_restricted_and_conditional_always_approve`, `title_child_cwd_is_temp_dir`)
- `agent_workflows::tests`: `10 passed; 0 failed` (flipped `run_args_include_plain_and_approve` + `workflow_run_args_restricted_and_conditional_always_approve`)
- `streaming_messages_json::tests`: `4 passed; 0 failed` (`streaming_probe_args_restricted_and_conditional_always_approve`)
- `batch_agents::tests`: `8 passed; 0 failed` (`batch_invoking_session_not_index_first`)
- Full `cd src-tauri && cargo test`: `1662 passed; 0 failed; 1 ignored`
- `cargo fmt --all -- --check` exit 1; dirty set is the 15-file post-05 list (`cli_install.rs` clean; leftover modules clean)
- Clippy `-D warnings` exit 101 at exactly `path_scope.rs:129`, `wecom.rs:210` (`batch_agents.rs:79` gone)

Caveats: `misc_p1.rs` / `worktree_agents_p1.rs` include rustfmt wrap of those assigned files (keeps them off the dirty set). AppWorkbench adds `sessionId` plus that id on the existing `useCallback` dep (no new `useState`). Residual: aux children still global-only; CLI flag honour Unverified.

## Review
Plan approval: `D06-PLAN-1` APPROVE_PLAN — reviewer `bc-5a47d34b-6f8a-53db-af91-a1bf1a716fd3`, contract `fa773d0f…bdff`, candidate `4e993693…bc85`.
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

### D06-PLAN-1 — APPROVE_PLAN (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-5a47d34b-6f8a-53db-af91-a1bf1a716fd3`, worktree `/tmp/loop-review/D06-PLAN-1` @ `74d31909`.
Contract `fa773d0f…bdff` (match). Candidate before/after `4e993693…bc85` (unchanged, clean-tree). Porcelain empty. Code vs `fbb03fc8` is pack-only.
Judgments: (a) BUILD Goal through Tests matches SLICES Now 06 (P2 leftover, N4, N5); (b) locked constraints present and not weakened; (c) live leftover spawn-site “today” descriptions accurate; (d) fail-closed / parent-subagents-on / aux residual Out; (e) one coherent slice with observable tests/greps. No blockers.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/slice-06-restrict-headless-9f74` off `origin/main` `fbb03fc8`.
Worker / role / phase: Reviewer / implementation / slice 06
Dispatch ID / launch state / input identity: `D06-IMPL-1` / launching / candidate `f19f791bfe2e9f89e1de0415c7b89cf9ed3eec9f0c4cbcdae403748c1595f59c` (code HEAD `0b536c3d`), contract `fa773d0f40ee8a972e8313128fdc9d844922669b66db3b6f8a380781b625bdff`
Pending result / last consumed dispatch: none / `D06-BUILD-1`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir.
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/`.
Baseline snapshot: slice 05 shipped candidate — HEAD `fbb03fc8` (merge; code `6eecaf7a` + pack), clean-tree, CANDIDATE `4e993693b764fef77241fbeab1c8335db1e82e292707ca104f65567d6758bc85`
Contract identity: `fa773d0f40ee8a972e8313128fdc9d844922669b66db3b6f8a380781b625bdff`
Candidate snapshot: HEAD `0b536c3d` (code commit), clean-tree, CANDIDATE `f19f791bfe2e9f89e1de0415c7b89cf9ed3eec9f0c4cbcdae403748c1595f59c`
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events:
- E1 / `D06-PLAN-1` / plan / APPROVE_PLAN / contract `fa773d0f…bdff`, candidate `4e993693…bc85` / no gaps / rejection count 0
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: if interrupted before `D06-IMPL-1` returns, re-dispatch `D06-IMPL-1`. Do not publish.
Advance phase: implementation candidate committed; review launching
Next slice ID / draft: 06 (`D06-IMPL-1` launching)
Environment note: rustc 1.98.1 / webkit2gtk present, same as 02–05.

## Status
Ready for review (code commit `0b536c3d`, candidate `f19f791b…f59c`)

## Next
Independent Reviewer `D06-IMPL-1`. After APPROVE_IMPLEMENTATION: archive 06 and draft 07. After REJECT: Builder repairs.

**Resume action:** launch `D06-IMPL-1` (isolated worktree, no write). Do not re-ship 01–05. Do not implement Later-outside work. Do not publish.
