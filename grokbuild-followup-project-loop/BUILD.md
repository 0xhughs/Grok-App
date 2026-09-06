# BUILD.md

Slice: 02 Deny wildcard IM senders on the live bridge
Archive: slices/02-deny-wildcard-im-senders.md

## Goal
The in-process Rust `remote_im` bridge (the bridge the app actually runs) never grants any IM sender access unless that sender's id is explicitly listed in the channel's `allow_from` ACL. `"*"` — alone, padded, or mixed into a list — is treated exactly like a missing or empty ACL: every sender is denied and the channel is refused at enable/start time. The enable-time error text tells the user to add explicit sender ids and no longer offers `*`. Rust unit tests mirror the Held Node `remote-bridge/src/r4.test.ts` assertions so R4/N2 cannot regress silently.

## Done when
- ACL semantics in `src-tauri/src/remote_im/outbound.rs`: `allow_from_list(acl)` reads `allowFrom` then `allow_from` (unchanged lookup order) and yields the **empty list** for every one of: key missing; JSON `null`; non-string value (array — including `[]`, `["*"]`, `["alice"]` — number, bool, object; arrays are not a supported shape and are *not* newly parsed, so this is unchanged fail-closed behaviour, not a widening); `""`; whitespace-only; `"*"`; `" * "`; a comma list whose entries are all empty (`",,"`); and any comma list that contains a `*` entry anywhere (`"alice, *"`, `"*, alice"`) — a wildcard entry poisons the whole list (exact parity with `r4.test.ts` lines 40–41; the alternative "drop `*`, keep the others" was rejected because it silently rewrites the user's ACL instead of surfacing the misconfiguration at enable time). Otherwise it yields the trimmed, non-empty comma-separated ids. Whether Builder keeps the `Option<Vec<String>>` signature (always `Some`) or narrows it to `Vec<String>` is an implementation choice; either way no "open" value is representable.
- `sender_allowed(acl, sender)` returns `true` **only** when `sender` equals one of the ids yielded by `allow_from_list`. The `None => true` arm and the `|| x == "*"` disjunct are gone: `rg -c 'None => true' src-tauri/src/remote_im/outbound.rs` = 0 and `rg -c 'x == "\*"' src-tauri/src/remote_im/outbound.rs` = 0.
- `allow_from_blocks_enable(acl)` returns `true` for every input that yields the empty list above (in particular `"*"`, `" * "`, `"alice, *"`, `["*"]`, `""`, `"   "`, missing) and `false` only for an ACL with at least one explicit id and no `*`. This is the function `runtime.rs:70` already calls, so a `*`-only or `*`-containing channel is refused before any connector starts.
- Enable-time error text: the literal at `src-tauri/src/remote_im/runtime.rs:71–72` (currently `"allow_from is empty: add your user id (or * for any) in Settings → Remote IM before enabling this channel"`) is replaced by a named constant `pub(crate) const ALLOW_FROM_BLOCKED_ERR: &str` (in `outbound.rs` or `runtime.rs`) used by `start_runtime`. Normative pattern for the new text: it must contain the phrase `explicit sender id` (singular or plural, case-insensitive), must name where to fix it (`Settings → Remote IM`), and must **not** match the case-insensitive regex `\*|wildcard|\bany\b|anyone|for all`. Proposed text: `allow_from must list explicit sender ids; empty or catch-all entries are refused. Add the platform user ids allowed to talk to this bot in Settings → Remote IM before enabling this channel`. A unit test (`enable_error_text_requires_explicit_ids_and_never_offers_wildcard`) asserts both halves of the pattern on the constant.
- Grep criteria over the live bridge: `rg -c -i '\* for any|or \*|\(or \*|\* for all' src-tauri/src/remote_im/` = 0; `rg -c '(allowFrom|allow_from)":\s*"\*"' src-tauri/src/remote_im/` = 0 (this excludes the unrelated DingTalk stream `"topic": "*"` at `channels/dingtalk.rs:68` and `protocol_start_tests.rs:62`, which are not ACLs and are untouched); no test under `src-tauri/src/remote_im/` asserts that a sender is allowed under a `*` ACL (`rg -n 'assert!\(sender_allowed\(.*"\*"' src-tauri/src/remote_im/` = 0 matches).
- Tests in `outbound.rs` `mod tests` (names normative; add/flip as stated):
  - `missing_allow_from_denies_by_default` is **flipped and renamed** to `allow_from_fails_closed_for_missing_empty_and_wildcard`: asserts `!sender_allowed` for `{}`, `{"allowFrom": null}`, `{"allowFrom": ""}`, `{"allowFrom": "   "}`, `{"allowFrom": ",,"}`, `{"allowFrom": "*"}`, `{"allowFrom": " * "}`, `{"allow_from": "*"}`, `{"allowFrom": "alice, *"}` for both `alice` and `mallory`, `{"allowFrom": "*, alice"}` for `alice`, `{"allowFrom": ["*"]}`, `{"allowFrom": []}`, `{"allowFrom": ["alice"]}` for `alice`; sender ids `anyone`/`attacker`/`owner` as in the current test. The two current `assert!(sender_allowed(... "*" ...))` lines (outbound.rs:381, :388) are removed.
  - New `allow_from_explicit_list_allows_only_listed_senders`: `{"allowFrom": "alice, bob ,,"}` allows `alice` and `bob`, denies `mallory`, denies `""`, denies the literal sender id `*`; `{"allow_from": "alice"}` (snake_case alias) allows `alice`.
  - New `allow_from_list_never_yields_open`: for each deny input above, `allow_from_list` yields an empty list (asserted via `.is_empty()` on the returned list / `Some(list)`), and for `"alice, bob"` yields exactly `["alice","bob"]`.
  - `blocks_enable_without_explicit_allow_from` is **flipped**: the line `assert!(!allow_from_blocks_enable(&json!({ "allowFrom": "*" })))` becomes `assert!(allow_from_blocks_enable(...))`; add `" * "`, `"alice, *"`, `"   "`, `["*"]` → blocks; `"alice"` and `"alice, bob"` → does not block. The comment "only an explicit wildcard or a non-empty list may start" is corrected.
  - New `enable_error_text_requires_explicit_ids_and_never_offers_wildcard` (pattern above).
- Fixture in `src-tauri/src/remote_im/engine.rs:2182` (`handle_slash_p_does_not_deadlock_on_pending_lookup`): `acl: json!({ "allowFrom": "*" })` becomes `acl: json!({ "allowFrom": "peer@im.wechat" })` — the test's own `sender_id` — so the message still passes the `sender_allowed` gate at `engine.rs:300` and continues to exercise the pending-lookup path it guards. Replacing `*` with an unrelated id would make the deadlock regression test vacuous; the reviewer should confirm the fixture id equals the fixture `sender_id`.
- Doc comment on `allow_from_list` (outbound.rs:269–277) no longer states `` `*` → open (None) ``; it states that `*` is deny.
- Bridge-level error honesty (inspect criterion): when `start_runtime` skips every enabled instance because of the ACL guard and therefore has nothing to start, the `Err` it returns (today the generic `"no enabled channel with credentials"`, runtime.rs:97) is the ACL error text, so the bridge `lastError` shown in the UI names the real cause. The per-instance `set_instance_last_error` call at runtime.rs:75 stays.
- All three call sites of `sender_allowed` are covered by the change with no edits of their own: `engine.rs:300` (inbound messages), `engine.rs:396` (card/callback actions), `channels/telegram.rs:173` (Telegram callback queries). No other file in `src-tauri/src/remote_im/` implements its own `*` check (`rg '"\*"' src-tauri/src/remote_im/` shows only the two DingTalk topic strings after the change).
- No file outside the Files constraint changes.

## Out
- Deleting or modifying the legacy Node `remote-bridge/` package (already Held for R4; its `r4.test.ts` is the reference, not a target).
- UI / i18n copy that offers `*` (`src/i18n/messages/<15 locales>/settings-remoteIm.ts`: `settings.remoteIm.err.allowFromRequired`, `settings.remoteIm.field.allowFromHelp`, eight `settings.remoteIm.<channel>.allowFromHelp`, plus `health.aclOpen`, `health.hint.openAcl`, `security.openCount`, `security.acl.open_acl`, `security.detail.aclOpen/aclEmpty` and nine `guide.step*` strings — 25 keys × 15 locales) and the UI save-time check at `src/components/RemoteImChannelPanel.tsx:356–359` that accepts `*` as non-empty. Reason: ~375 translated strings plus a UI-behaviour change is not "small"; the security boundary is the Rust bridge, which fails closed regardless of what the UI accepts. Handed to slice 09 (SLICES.md Later 09 Goal amended by coordinator).
- `docs/llm-wiki/remote-im.md` (internal agent wiki listing `*` as an allow_from default/hint at lines 162, 341, 360, 385, 445, 498, 548, 566) and `docs/features/remote-security.md:19` (describes the UI's "open (`*`)" aggregation, which remains an accurate description of the UI until slice 09) — slice 09.
- Adding array-shaped `allow_from` parsing, `allow_chat`/`adminFrom` semantics, per-channel ACL changes, or any new ACL feature.
- Automatic rewriting or migration of stored ACLs (no config is edited on the user's behalf).
- Slices 03–09.

## Constraints
- Files: `src-tauri/src/remote_im/outbound.rs`, `src-tauri/src/remote_im/runtime.rs`, `src-tauri/src/remote_im/engine.rs` (test fixture line only). No other file.
- No changes to `remote-bridge/`, `src/`, `src/i18n/`, `docs/`, `README*.md`, `SECURITY.md`, `Cargo.toml`, `Cargo.lock`, or CI workflows.
- No widening of `allow_from`: every input that is denied today stays denied; the only behavioural delta is that inputs containing `*` move from allow-everyone to deny. No new input shape (arrays, globs, prefixes) becomes accepted.
- Held gates untouched: `allow_remote_yolo` (R2) plumbing through `start_runtime(allow_remote_yolo)` and `Engine::new(..)` is unchanged; Ask remains the default permission policy; nothing in `mirror/`, `permission.rs`, or capabilities is edited.
- Keep `require_mention`, `secret_or_opt`, `OutboundRouter` and all non-ACL code in `outbound.rs` byte-identical apart from the ACL functions, their doc comments, and `mod tests`.
- Error text must satisfy the normative pattern in Done when; it must not reference `/whoami` (the Rust engine has no such command — `rg whoami src-tauri/src/remote_im/engine.rs` = 0).
- Rust style: `cargo fmt` clean; `cargo clippy --all-targets -- -D warnings` clean (CI runs both with `-D warnings`, `.github/workflows/ci.yml` rust job).
- Do not claim any cargo command passed unless it was run in the Builder session; record exact commands and result lines in Proof.

## Data / state impact
- Existing users whose stored ACL is `"*"` (or contains `*`): after upgrade the bridge denies every sender for that channel. This is intentional fail-closed behaviour.
- Start/enable path for such a channel (code path: `bridge.rs:181 start_async` → `runtime.rs:51 start_runtime` → `runtime.rs:70 allow_from_blocks_enable`): the instance is skipped, `tracing::error!` logs the new text, and `config::set_instance_last_error(id, Some(text))` stores it; the Settings → Remote IM channel card shows it under `health.lastError` (`RemoteImChannelPanel.tsx:958–960`). If other enabled channels have explicit ids they start normally. If no instance survives, `start_runtime` returns `Err` (per Done when, the ACL text), `start_async` sets bridge `last_error`, `running=false`, phase `error` (`bridge.rs:215–221`), and `remote_im_bridge_start` returns that `Err` to the UI. App launch auto-start (`bridge.rs:252 try_autostart_async` → `start_async`) follows the same path; failure is logged at `warn` (`mod.rs:210–217`) and the bridge stays stopped. No panic, no partial start, no config rewrite.
- Until slice 09 the UI still accepts `*` at save time (`RemoteImChannelPanel.tsx:356–359` only checks non-empty) and its help/error copy still offers `*`; a user who types `*` will save successfully and then see the Rust enable-time error on the channel card. Recorded as a known, fail-closed inconsistency handed to slice 09.
- No stored data format changes; no migration; no secrets touched.

## Tests
- Targeted ACL tests (must be run in-session): `cargo test --manifest-path src-tauri/Cargo.toml --lib remote_im::outbound::tests` — expected: the five tests named in Done when plus the two pre-existing non-ACL tests (`register_always_injects_instance_id`, `require_mention_honors_acl_and_group_reply_all`) pass; `0 failed`.
- Whole live-bridge module: `cargo test --manifest-path src-tauri/Cargo.toml --lib remote_im::` — expected `0 failed`, and `remote_im::engine::tests::handle_slash_p_does_not_deadlock_on_pending_lookup` is listed as `ok` (proves the fixture change kept it live).
- Full suite as CI runs it: `cd src-tauri && cargo test` — expected `0 failed` (Linux leg; webkit2gtk/gtk and a current stable toolchain are installed in this environment so it links).
- Lint as CI runs it: `cd src-tauri && cargo fmt --all -- --check` exits 0; `cd src-tauri && cargo clippy --all-targets -- -D warnings` exits 0.
- Negative proof that the flip is real: Proof includes the `git diff` hunks showing outbound.rs:381 and :388 (`assert!(sender_allowed(... "*" ...))`) removed and outbound.rs:397 negation removed, and states that the pre-change test suite would fail against the new code (the two assertions are contradictory with the new semantics).
- Grep criteria (each exact command and count recorded in Proof): `rg -c 'None => true' src-tauri/src/remote_im/outbound.rs` → 0; `rg -c 'x == "\*"' src-tauri/src/remote_im/outbound.rs` → 0; `rg -c -i '\* for any|or \*|\(or \*|\* for all' src-tauri/src/remote_im/` → 0; `rg -c '(allowFrom|allow_from)":\s*"\*"' src-tauri/src/remote_im/` → 0; `rg -n 'assert!\(sender_allowed\(.*"\*"' src-tauri/src/remote_im/` → no matches; `rg -n '"\*"' src-tauri/src/remote_im/` → exactly the two DingTalk topic lines (`channels/dingtalk.rs`, `protocol_start_tests.rs`).
- Scope check: `git diff --stat <baseline>..HEAD -- . ':!grokbuild-followup-project-loop'` lists exactly the three Files.
- No `pnpm vitest` run is required (no TS/i18n change); `remote-bridge/` tests are not part of this slice's proof.

## Proof
Not completed yet.

## Review
Plan approval: none
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/grokbuild-followup-loop-c341` off `origin/main` `ea4ec712` (= `c66b3ec7` + pack files only).
Worker / role / phase: Reviewer / plan review / slice 02
Dispatch ID / launch state / input identity: `D02-PLAN-1` / launching / candidate `8c79e574…341e`, contract `f60bdebb…66ed`
Pending result / last consumed dispatch: none / `D02-DRAFT-1` (Builder agent `bc-ff3de374-17be-59d4-a9b5-103f02fd3258`; returned Proposed page + grounding notes + a proposed SLICES 09 amendment; no edits, porcelain empty. Coordinator persisted the page verbatim and applied the 09 amendment in SLICES.md as a within-authority contract edit — see SLICES Later 09.)
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir (source, tests, `.github/workflows/`, `scripts/`, lockfiles, docs, capabilities, assets).
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/` (protocol + artifacts).
Baseline snapshot: slice 01 shipped candidate — HEAD `73193a0e03e46920abc40e2908a3ab288d64bf5b` (code), clean-tree, CANDIDATE `8c79e57482ff2996eb5fd37b802156219ae1ad6bb4f8ffa6bd78d32573d5341e`
Contract identity: `f60bdebb90c30d06df8f19dcb401ffc3ca83b4edf9993dba870a52d975fb66ed`
Candidate snapshot: none (plan phase; equals baseline)
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events: none
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: none
Advance phase: none (slice 01 archive written and verified; 02 selected)
Next slice ID / draft: 03 (after 02 ships)
Environment note: `cargo test` is linkable here — webkit2gtk-4.1 2.52.6, gtk+-3.0 3.24.41, libsoup-3.0, javascriptcoregtk-4.1, ayatana-appindicator3, librsvg installed via apt on 2026-09-06; a dependency requires Rust edition 2024 so `rustup toolchain install stable` (≥1.85) was installed and set default. Warm-up `cargo test --no-run` running in tmux session `cargo-warm` (log `/tmp/cargo-warm.log`).

## Status
Proposed

## Next
Independent Reviewer plan review `D02-PLAN-1` in isolated worktree. Do not implement slice 02 before APPROVE_PLAN.
