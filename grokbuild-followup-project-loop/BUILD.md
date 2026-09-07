# BUILD.md

Slice: 05 Honest CLI installer
Archive: slices/05-honest-cli-installer.md

## Goal
`KNOWN_CLI_HASHES` is **removed** (not regenerated). With no published sidecar, trust is TOFU: first digest is recorded; a later different digest is a **hard error** unless the existing UI/env override is on. The first-seen hash store is written **0600**. Published-sidecar mismatch still always aborts. `GROK_CLI_REQUIRE_CHECKSUM` stays opt-in. Ask stays default.

## Done when
**Pick (only this, not a menu): remove the known-good table.** Do not generate a replacement table, do not commit a generator, do not check in `KNOWN_CLI_HASHES.sha256`, do not download-and-pin current stable `1.0.13` (or any other version) as a new baked list.

Locked C2: no fabricated known-good table; first-seen change is a hard error with UI override. Live evidence (`D05-DRAFT-1`; not invented):

| What | URL | Result |
|---|---|---|
| stable pointer | `GET https://storage.googleapis.com/grok-build-public-artifacts/cli/stable` | `200` body `1.0.13` |
| sidecar candidates on both mirrors | HEAD `*.sha256` / `SHA256SUMS` / etc. | all **404** |
| table pin `grok-0.2.111-linux-x86_64` | in-tree `cli_install.rs:92–94` | `c903e1fa07d52436…` |
| same artifact | `GET …/cli/grok-0.2.111-linux-x86_64` | `f158d0d43367c395…` (**≠ table**; matches audit N8) |

Install resolves `CHANNEL = "stable"` → **1.0.13 today**. The table only lists 0.2.100/110/111 and cannot protect current stable.

### C2 — delete fabricated table
- Delete `KNOWN_CLI_HASHES` (`cli_install.rs:81–157`), `lookup_known_cli_hash` (`:161–171`), and `is_known_cli_hash` (`:174–181`).
- In `install_cli_latest` (`:944–987`), delete the `is_known_cli_hash` / `lookup_known_cli_hash` arms (`:958–965`). After a published sidecar `None`, every artifact goes through first-seen (then the existing missing-sidecar `require_published_checksum` gate). Sidecar `Some` + digest mismatch still `Err` and deletes the temp file (`:948–952`). No override on sidecar mismatch.
- Delete test `known_cli_hash_lookup_and_verification` (`:1199–1226`).
- Grep (cwd `/workspace`, Proof lists `-n`):
  - `rg -n 'KNOWN_CLI_HASHES|lookup_known_cli_hash|is_known_cli_hash' src-tauri/src` → **0**
  - `rg -n 'c903e1fa07d52436|a7f1c9d8e5b30214|fabricated' src-tauri/src/cli_install.rs` → **0**

### N8 — first-seen change is a hard error; store 0600; existing UI override
- Keep `FirstSeenStatus` (`:185–189`): `RecordedNew` | `MatchedExisting` | `Changed { previous, current }`.
- `pub fn check_or_update_first_seen_hash_in_file` (`:205–271`): on `None` → insert + write + `RecordedNew`; on case-insensitive match → `MatchedExisting` (no write); on **Changed → return `Changed` and do not write** (today `:253` overwrites and `:261–267` persists the new hash — that is the bug). Write failures on `RecordedNew` are `Err`, not `let _ =`.
- `pub fn accept_first_seen_hash_in_file(store_path, artifact_name, hash) -> Result<(), String>` — overwrite that key and write 0600. Only the override path calls this.
- `fn write_hash_store_0600(path, bytes)` (same module): Unix `OpenOptions` create/truncate `.mode(0o600)` + `set_permissions(0o600)` (same pattern as `agent_home_config.rs:587–595`). Non-Unix: `fs::write`. Do **not** edit `agent_home_config.rs` (slice 07). Do **not** route `~/.grok/first_seen_hashes.json` through `write_private_agent_home_file`.
- `pub enum FirstSeenGate { ProceedUnverified, RefuseChanged { previous: String, current: String }, AcceptedChange }`
- `pub fn first_seen_install_gate(status: FirstSeenStatus, allow_unverified: bool) -> FirstSeenGate`:
  - `RecordedNew` | `MatchedExisting` → `ProceedUnverified`
  - `Changed` && `allow_unverified` → `AcceptedChange`
  - `Changed` && `env_flag_truthy("GROK_CLI_ALLOW_UNVERIFIED")` → `AcceptedChange`
  - else `Changed` → `RefuseChanged`
- `pub fn first_seen_change_error(artifact_name, previous, current) -> String` must contain all of: `first-seen hash changed`, the artifact name, `previous=`, `current=`, `Allow unverified CLI install`, `GROK_CLI_ALLOW_UNVERIFIED`, and `No published SHA-256`. Must **not** contain `SHA-256 mismatch` / `checksum mismatch` or `GROK_CLI_REQUIRE_CHECKSUM`.

**Wire into `install_cli_latest` only**, sidecar-`None` branch (today `:957–985`). Replace `let _ = verify_or_record_first_seen_hash(...)` (`:968`) with:
1. `status = verify_or_record_first_seen_hash(&artifact_name, &digest)?`
2. `match first_seen_install_gate(status, allow_unverified)` — `RefuseChanged` → remove temp + `Err(first_seen_change_error)`; `AcceptedChange` → `accept_first_seen_hash_in_file` then continue; `ProceedUnverified` → continue
3. Existing `require_published_checksum(allow_unverified)` missing-sidecar refuse (`:971–978`) unchanged.

**UI override (reuse, do not add a setting):** `allow_unverified_cli_install` (default false). No new Settings key, no i18n, no `App.tsx` state, no `window.confirm`.

**Do not** default `GROK_CLI_REQUIRE_CHECKSUM` on. `require_checksum_policy_default_and_strict_env` stays.

**Named tests** in `cli_install.rs` `mod tests` (names normative):
- `first_seen_hashes_recording_and_warning_on_change` — **flip**: after `Changed` for `h2`, the file still contains `h1` and a follow-up `check_or_update` with `h2` is still `Changed`.
- `first_seen_install_gate_refuses_change_without_override`
- `first_seen_accept_change_writes_new_hash`
- `first_seen_store_mode_0600` (`#[cfg(unix)]`)
- `first_seen_change_error_lists_override_and_avoids_mismatch_classifier`

`#[tauri::command]` count stays **423**. No file outside Files changes.

Grep:
- `rg -n 'fn write_hash_store_0600|fn accept_first_seen_hash_in_file|fn first_seen_install_gate|fn first_seen_change_error' src-tauri/src/cli_install.rs` — each present.
- `rg -n 'fs::write\(store_path' src-tauri/src/cli_install.rs` → **0**
- `rg -n 'FirstSeenStatus::Changed' src-tauri/src/cli_install.rs` — enum, no-write arm, gate, tests (Proof lists each).

## Out
- Hosting a new artifact bucket. Do not upload, mirror, or commit CLI binaries or a new checksum host.
- Regenerating `KNOWN_CLI_HASHES` from downloads; adding a generator; checking in `KNOWN_CLI_HASHES.sha256`.
- Inventing or baking measured `1.0.13` / `0.2.111` hashes as a new known-good table.
- Held IDs. Do not reopen P1, P3, P4, P5, R1, R2, R3, R5, R6, S1, S3, C3, D1–D6.
- Defaulting `GROK_CLI_REQUIRE_CHECKSUM` on / changing `store.rs` defaults / new Settings keys / i18n / `settingsCatalog` / docs (slice 09).
- Overriding published-sidecar mismatch. Changing `MIRROR_BASES`. Widening `allow_from`. Disabling parent-session Grok subagents.
- Editing `agent_home_config.rs` (slice 07).
- Frontend / Setup wizard new error kinds (reuse existing `checksum_missing` via required error tokens).
- rustfmt-rewrite of pre-existing dirt in `cli_install.rs`. Slices 06–09. Publishing / deploying.

## Constraints
- **Files:** `src-tauri/src/cli_install.rs` only. No `Cargo.toml` / `Cargo.lock`. No `src/`, i18n, docs, capabilities, `lib.rs`, `session_p1.rs`, `store.rs`, `cli_update.rs`, `agent_home_config.rs`.
- Ask remains default. Do not change `store.rs` defaults.
- Do not add crates. Do not add network to unit tests (temp-dir store only).
- Rust style: new/changed hunks rustfmt-clean. **Do not rustfmt-rewrite pre-existing dirt** in `cli_install.rs` (already on the 16-file dirty list). rustfmt/clippy non-regression vs rustc 1.98.1 baseline: `cargo fmt --all -- --check` still exits 1 with diffs **only** in the same 16 files; `cargo clippy --all-targets -- -D warnings` still exactly `batch_agents.rs:79`, `path_scope.rs:129`, `wecom.rs:210`.
- Do not claim cargo passed unless that session ran it.

## Data / state impact
- No settings / secret-store migration. `allow_unverified_cli_install` default stays **false**.
- `~/.grok/first_seen_hashes.json`: new keys recorded at 0600; a digest change does **not** replace the stored hash unless Settings / `GROK_CLI_ALLOW_UNVERIFIED` / `cli_install_latest({allowUnverified:true})` accepted the change.
- First install of current stable (or any un-sidecared artifact): `RecordedNew`, install continues (unless `GROK_CLI_REQUIRE_CHECKSUM=1` without override).
- Re-install with a different digest for the same artifact name: hard `Err`, temp file removed, previous hash kept.
- Sidecar present + match: `checksum_verified: true`; first-seen not consulted.
- IPC `cli_install_latest` args unchanged.

## Tests
- `cargo test --manifest-path src-tauri/Cargo.toml --lib cli_install::tests` — existing tests minus deleted table test, plus the four new names and the flipped first-seen test; `0 failed`.
- Grep criteria in Done when; each listing + count in Proof.
- Lint non-regression: dirty set identical to the 16-file baseline; clippy exactly the three baseline lints.
- Scope: `git diff --stat` vs the 05 implementation baseline lists exactly `src-tauri/src/cli_install.rs`.
- No `pnpm vitest`. Implementation Proof runs `cli_install::tests`. Full `cd src-tauri && cargo test` required; expected `0 failed` (or the same pre-existing parallel flake outside Files as slice 04, with serial `--test-threads=1` green).

## Proof
none (Proposed; draft `D05-DRAFT-1` produced this page, no code)

## Review
Plan approval: none
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/grokbuild-followup-loop-c341` off `origin/main` `ea4ec712` (= `c66b3ec7` + pack files only).
Worker / role / phase: Reviewer / plan review / slice 05
Dispatch ID / launch state / input identity: `D05-PLAN-1` / launching / candidate `308f6af6…9b8c` (code HEAD `51330f48`), contract `05d3fd9c…fdf8`, draft `D05-DRAFT-1`
Pending result / last consumed dispatch: none / `D05-DRAFT-1`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir.
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/`.
Baseline snapshot: slice 04 shipped candidate — HEAD `51330f4874b96679d104da58914f84c3b529960b` (code), clean-tree, CANDIDATE `308f6af69624dcd1d62d764a64074687d5ba65f24b00db68ff4847e7ec739b8c`
Contract identity: `05d3fd9cb4c40d86f060ae401fdfc2353150da203fe6e015b91bd1d1f3f5fdf8`
Candidate snapshot: HEAD `51330f4874b96679d104da58914f84c3b529960b` (code commit), clean-tree, CANDIDATE `308f6af69624dcd1d62d764a64074687d5ba65f24b00db68ff4847e7ec739b8c`
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events: none
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: none
Advance phase: next selected (05); BUILD replaced with Proposed page
Next slice ID / draft: 06 (after 05 ships)
Environment note: rustc 1.98.1 / webkit2gtk present, same as 02–04.

## Status
Proposed (draft `D05-DRAFT-1`; pending `D05-PLAN-1`)

## Next
Independent plan review `D05-PLAN-1` in isolated worktree. On APPROVE_PLAN → Not started, Builder `D05-BUILD-1`. On REJECT_PLAN → Proposed, Builder revises.

**Resume action:** consume or launch `D05-PLAN-1` (Reviewer plan review for slice 05). Do not re-ship 01–04. Do not implement Later-outside work. Do not publish.
