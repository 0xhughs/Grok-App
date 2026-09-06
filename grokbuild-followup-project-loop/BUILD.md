# BUILD.md

Slice: 01 Restore real CI pins
Archive: slices/01-restore-real-ci-pins.md

## Goal
Every `uses:` pin in `.github/workflows/ci.yml` and `release.yml` is a 40-character SHA that exists on the upstream action repo at the intended tag. A local checker fails fabricated pins so C1 cannot regress the same way.

## Done when
- C1/N7: `actions/setup-node`, `dtolnay/rust-toolchain`, `swatinem/rust-cache`, and `tauri-apps/tauri-action` pins resolve via `git ls-remote https://github.com/<owner>/<repo> <sha>` or `refs/tags/<tag>`. Do not invent hex. Do not keep the current fabricated SHAs.
- `actions/checkout` and `pnpm/action-setup` remain real SHAs; re-verify them the same way.
- A script (e.g. `scripts/check-workflow-pins.py` or an addition to `scripts/check-code-quality-gates.py`) parses both workflow files, rejects non-40-hex `uses:`, and fails if `git ls-remote` cannot see that SHA. A unit/fixture test feeds a mangled SHA and expects failure.
- No application runtime behaviour changes in this slice.

## Out
- Making the full Rust `cargo test` job green in environments without webkit2gtk (report residual if GH Actions still cannot run here).
- Rewriting release publishing logic beyond pin repair.
- Slices 02–09.

## Constraints
- Files: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, pin-check script under `scripts/`, optional hook from `scripts/check-code-quality-gates.py`.
- Resolve SHAs from `git ls-remote` of the action repository tags used before e37d212 if those tags still exist; otherwise pick the current matching major tag and record which tag was resolved.
- Do not pin `owner/action@v4` floating tags.
- Do not restore 06d82f9 tag-only pins as the end state; SHA-of-tag is required.

## Data / state impact
None. Workflow metadata only.

## Tests
- Pin-check script exits 0 on the repaired workflows.
- Pin-check script exits non-zero on a copy with one SHA flipped.
- Document the exact `git ls-remote` commands and output used to obtain each SHA in Proof.

## Proof
Not completed yet.

## Review
Pending plan review.
Plan approval: none
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

## Loop state
Execution mode / tool adapter: Google Antigravity. Coordinator is the primary agent. Builder and Reviewer are independent subagents via `invoke_subagent` (fallback `define_subagent` then invoke). Builder workspace: inherit or share the grokbuild checkout. Reviewer workspace: isolated `branch` worktree or an exact copy of the candidate; Reviewer tools must not include file-write / apply-patch. Record actual tool names and agent IDs only after first authorized dispatch.
Coordinator: none
Worker / role / phase: none
Dispatch ID / launch state / input identity: none
Pending result / last consumed dispatch: none
Snapshot capture and recheck commands / coverage / exclusions:
- Capture: `git -C <GROKBUILD_REPO> rev-parse HEAD`; `git -C <GROKBUILD_REPO> status --porcelain=v1`; if porcelain is non-empty, write a SHA-256 manifest of covered relative paths to `<PACK_DIR>/artifacts/snapshot-<dispatch>.manifest` and record `sha256sum` of that file.
- Recheck: repeat the same commands; compare HEAD and manifest digest.
- Coverage: `.github/workflows/`, `scripts/`, lockfiles if touched.
- Exclusions: `target/`, `node_modules/`, `dist/`, pack `artifacts/` manifests.
Baseline snapshot: none
Contract identity: none
Candidate snapshot: none
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events: none
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: none
Advance phase: none
Next slice ID / draft: none

## Status
Proposed

## Next
Coordinator: when authorized, bind Antigravity tool names, capture review inputs, persist a pending plan-review dispatch, and invoke an independent Reviewer. Do not implement slice 01 before APPROVE_PLAN.
