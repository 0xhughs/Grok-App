# LOOP.md

## Authority and roles
- Work only through the Loop target in `SLICES.md`, preserving `AGENTS.md` and user authorization. `Shipped` means independently accepted, not deployed.
- Coordinator is the sole writer of protocol files and archives. Builder writes application code/tests and returns proposed contract edits and proof. Reviewer reads and verifies; it returns verdicts and never implements fixes.
- Coordinator records Reviewer decisions verbatim. Only a valid independent Reviewer verdict authorizes `Not started` or `Shipped`. Any participant may report a blocker or escalation; coordinator persists it.
- Use one active worker per checkout. A second coordinator must not start while ownership is unresolved. Record coordinator and worker identities in `BUILD.md`; stop/confirm the former owner before takeover.

## Antigravity adapter
This pack is written for Google Antigravity native subagents. Do not persona-switch in the primary session.

- Coordinator = the primary Antigravity agent in this session (`mainAgent`).
- Builder = workspace agent `.agents/agents/builder.md` invoked as a subagent.
- Reviewer = workspace agent `.agents/agents/reviewer.md` invoked as a subagent with a fresh context every review.
- Spawn with `invoke_subagent` against those named agents. If they are not yet registered, `define_subagent` from the markdown files, then invoke.
- Builder workspace option: `inherit` or `share` the 0xhughs/grokbuild checkout.
- Reviewer workspace option: `branch` (isolated git worktree) or an exact copy of the candidate including untracked files. A checkout of HEAD alone is not enough when the candidate is dirty.
- Reviewer must not receive write/edit/apply-patch tools. Builder may write only in the grokbuild checkout, never in protocol files.
- After spawn, record the returned subagent ID in BUILD Loop state. Wait until that subagent is terminal before changing ownership. Interim sidebar status is not completion.
- If `invoke_subagent` is missing, stop with Human required: “independent Reviewer unavailable.” Do not fake the review.

## Dispatch and recovery
On the first authorized dispatch set SLICES Run status to `Running`. When blocked or escalated, mirror `Blocked` or `Human required` there; after resolution restore `Running` or `Finalizing` as appropriate.

1. On resume, read live files and check recorded worker status, pending result, counters, snapshot identities, blocker and advance phase. Reconcile runtime activity (`/agents` panel or equivalent inventory) before editing or dispatching.
2. Persist unique dispatch ID, role, phase, input identity and pending launch before starting the worker. Record returned agent ID. If launch was interrupted, discover or stop the old worker; never assume it did not start.
3. Builder receives the current contract and blockers. Reviewer receives current project files, candidate, baseline/diff and dispatch ID; its context is independent of Builder reasoning.
4. Wait for confirmed completion before changing ownership. Capture the result durably in BUILD `Pending result` (SLICES `Pending release result` for release reviews), then validate the dispatch, phase, input identity, and worker identity. Reject stale or duplicate messages as control events, without incrementing review-rejection counters.
5. Coordinator applies verdict, counters, event and `last consumed dispatch` together in one atomic `BUILD.md` replacement, then clears pending result. For release results, use the same atomic event/counter/consumed-ID transaction in SLICES Release evidence. A crash before replacement leaves a replayable pending result; a crash after it cannot apply the verdict twice.
6. Workers never edit protocol files. Reviewer checks must not mutate candidate inputs; run mutating checks in an exact isolated copy. Unexpected external edits invalidate affected approvals. Do not overwrite unrelated user edits when persisting state.

## Snapshot and contract identity
Before first dispatch, record reproducible capture/recheck commands, coverage, exclusions and baseline in `BUILD.md`. Coverage includes source, tests, configuration, lockfiles, relevant assets and required untracked files. Exclude only named irrelevant/generated/dependency paths; Reviewer verifies coverage. Do not include secret values in proof.

A clean Git commit ID is sufficient only after confirming no relevant staged, unstaged or untracked content differs. Otherwise use a SHA-256 content manifest with sorted relative paths, file bytes, types, executable modes, and symlink targets. Detect additions and deletions by regenerating the covered file list. Record the manifest location and digest. The baseline must be available for comparison. Do not create commits merely to manufacture identity.

Contract identity covers the substantive `AGENTS.md`, product/target/criteria in `SLICES.md`, `BUILD.md` slice ID and Goal through Tests, and operational rules in `LOOP.md` and both role files. Include snapshot capture/recheck commands, coverage and exclusions in contract identity even though their values live in Loop state. Exclude only mutable bookkeeping (Proof, Review, Loop state, Status, Next, run status, release evidence and Shipped/Now placement). State-only writes must not invalidate approvals.

Reviewer independently computes identities before and after its checks and records both plus commands, results and artifact paths. Accept approval only if they match dispatch inputs and coordinator recomputation. Before Builder starts, validate plan approval against its baseline and contract. Before archiving or releasing, validate implementation approval against current candidate and contract. Changed contract -> `Proposed`. Changed code after approval -> fresh implementation review. Intended Builder edits after valid plan approval produce the implementation candidate; they do not themselves require another plan review.

## State transitions
| From | Required event | To / next action |
|---|---|---|
| Proposed | Reviewer APPROVE_PLAN with matching inputs | Not started; Builder may start |
| Proposed | Reviewer REJECT_PLAN | Proposed; Builder revises proposal, then fresh plan review |
| Not started | Valid plan approval, Builder dispatch | Building |
| Building | Builder completes proof and stops | Ready for review; independent review |
| Ready for review | Reviewer REJECT_IMPLEMENTATION | Building; Builder repairs, then review |
| Ready for review | Reviewer APPROVE_IMPLEMENTATION with matching inputs | Shipped; coordinator advances |
| Any unfinished state | Temporary recoverable blocker | Blocked; persist recovery fields |
| Any state | Authority, evidence, retry or budget escalation | Human required; stop |

A revised proposal always returns to plan review. Builder may never alter accepted criteria to make a failure pass. If a necessary contract change is within existing authority, persist it and return to `Proposed`; otherwise escalate.

## Durable retry and budget policy
Default limits: 3 Reviewer rejections per slice; 2 consecutive repair attempts with no material improvement to failing evidence. Count both plan and implementation rejections. Plan approval, restart, worker replacement and phase change do not reset rejection count.

For each unique accepted review result, append event ID/dispatch, phase, verdict, input identities, acceptance gaps, evidence comparison and updated counters to `Review events`. Increment rejection count once for each REJECT verdict. After a repair, Reviewer compares evidence against prior open gaps: no material improvement increments no-progress; material improvement resets only no-progress. Missing comparison evidence must be resolved before another repair dispatch.

Check limits after recording the event and before another attempt. At either limit set `Human required`. Freeze final counter totals on implementation approval for the archive. Reset counters only for a new slice or explicit human authorization with a recorded reason.

Record an execution budget only if configured. Do not invent one.

## Block and resume
Use `Blocked` only for a temporary dependency with a concrete safe recheck and bounded retry time/attempt limit. Save prior status, exact next action, blocker, evidence, recheck condition and deadline in `Loop state`. Confirm any worker is stopped before resuming another.

On re-entry, perform only the allowed recheck. If unchanged, remain blocked without dispatching work. If the limit expires or required permission/resource cannot be obtained, set `Human required`. If resolved, validate identities and restore prior status/action. Clear blocker fields only after recording resolution.

`Human required` resumes only after the specific missing decision/resource/authorization is supplied and evidenced.

## Archive, advance and completion
1. After valid implementation approval, persist `Shipped` and advance phase `archive pending`. No worker may write code during advance.
2. Copy the accepted BUILD page to its stable archive path; never overwrite differing history. Persist `archive written` after verification.
3. Add the slice once to Shipped in `SLICES.md`. If authorized target work remains, select exactly one next Now slice, record advance phase `next selected`, and dispatch a Builder draft-proposal phase for that next ID (no code edits). Only after archive verification replace BUILD with the next slice’s Proposed page and zeroed counters. Plan review is required before code changes.
4. After the last target slice, set Now to `None — target complete`, retain the last Shipped BUILD as a receipt, set advance phase `release pending`, and set run status to `Finalizing`.
5. Independent Reviewer checks applicable release gates against the final candidate and contract. Store verdict in SLICES. Set run status `Complete` only after coordinator rechecks identities.
6. A gate failure cannot produce Complete: set run status `Needs repair` and draft one Proposed repair slice only if it fits existing target/authority. Bound repeated release repairs to 3 failed release reviews per target.

Success requires: all target slices accepted, Now empty, matching final evidence, applicable gates passed or explicitly not applicable, no unresolved blocker/escalation, and run status Complete. It does not authorize publishing, deployment, future scope, or continued background execution.
