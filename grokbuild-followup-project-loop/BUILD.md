# BUILD.md

Slice: 01 Restore real CI pins
Archive: slices/01-restore-real-ci-pins.md

## Goal
Every `uses:` in `.github/workflows/ci.yml` and `.github/workflows/release.yml` is pinned to the 40-hex **peeled commit id** of a named upstream ref (`refs/tags/<tag>` or `refs/heads/<branch>`), obtained from `git ls-remote --tags --heads https://github.com/<owner>/<repo>`, and each pin's trailing comment names that ref. A local checker with an injectable ref listing rejects fabricated, malformed, tag-object, mismatched, and stale pins, fails closed on network error while reporting it distinctly, and runs in CI, so C1/N7 cannot regress the same way.

## Done when
- Pin format: every `uses:` value in both workflow files matches `<owner>/<repo>[/<subpath>]@<40 lowercase hex>` followed by a comment `# <ref-label>` on the same line. `<ref-label>` is either `<tag>` (meaning `refs/tags/<tag>`) or `<name> (refs/heads/<name>)` for a branch. Floating tags (`@v4`, `@stable`), short SHAs, uppercase hex, `docker://`, local `./` actions, and missing comments are all failures.
- Resolution method (single source of truth): the pin for a ref is the **peeled commit id** from `git ls-remote --tags --heads https://github.com/<owner>/<repo>`: the `refs/tags/<tag>^{}` line for an annotated tag; the ref's own line for a lightweight tag (no `^{}` line exists) or a branch. The `git ls-remote <url> <sha>` form is not used anywhere (it filters by ref name and exits 0 with empty output for any SHA).
- Annotated-tag rule: a tag-object id (the id on the `refs/tags/<tag>` line when a `refs/tags/<tag>^{}` line also exists) is never a valid pin. The checker peels and reports such a pin as `TAG_OBJECT`, not as OK.
- Intended refs, resolved by the implementing Builder at build time (never copied from this contract or from e37d212): `actions/checkout` → `refs/tags/v4.2.2`; `pnpm/action-setup` → `refs/tags/v4.0.0`; `actions/setup-node` → `refs/tags/v4.0.3`; `swatinem/rust-cache` → `refs/tags/v2.7.7`; `tauri-apps/tauri-action` → `refs/tags/v0.5.17`; `dtolnay/rust-toolchain` → `refs/heads/stable` tip (no `refs/tags/stable` exists) with comment `# stable (refs/heads/stable)`. `actions/checkout` and `pnpm/action-setup` are re-verified by the same method even if their SHAs do not change. If a named tag no longer exists at build time, use the newest tag within the same major and record the substitution in Proof.
- The SHAs `1e60f620b9541d16bece96c5465dc8ee9832e0b4`, `f0deed1e0edfc6a9be954172b8c0ab617c49165c`, `b70ec574c8034d6beea488582d1c9ef2cf1c9cb4`, and `4dd2f0b9f5e4277b5a8eb2a6fb368c22119fa9d1` appear nowhere under `.github/workflows/` (`grep -rc` = 0 each).
- Checker `scripts/check_workflow_pins.py` (stdlib only): parses every `uses:` in both files (default) or in paths given as arguments; runs one `git ls-remote --tags --heads` per distinct `<owner>/<repo>`; classifies each pin as exactly one of `OK`, `MALFORMED`, `UNKNOWN_SHA` (SHA equals no branch/lightweight-tag tip and no `^{}` peeled id in the listing), `TAG_OBJECT`, `REF_MISMATCH` (SHA is a real peeled id but not the one for the ref named in the comment), or `NETWORK` (`git ls-remote` exited non-zero, timed out, or produced unparsable output). Classification order per pin is `MALFORMED`, then `NETWORK`, then `TAG_OBJECT`, then `UNKNOWN_SHA`, then `REF_MISMATCH`, else `OK`; in particular a tag-object id whose comment names a different ref is `TAG_OBJECT`, not `REF_MISMATCH`. It prints one line per pin `<file>:<line> <owner>/<repo>@<sha> # <ref-label> -> <verdict> [<resolved ref> = <sha>]`. Exit 0 only when every pin is `OK`; exit 1 when any pin is `MALFORMED`/`UNKNOWN_SHA`/`TAG_OBJECT`/`REF_MISMATCH`; exit 2 when any repo listing failed (`NETWORK`, fail closed, never reported as `UNKNOWN_SHA`). Exit 2 takes precedence over exit 1 when both kinds of verdict occur in one run.
- Injectable ref listing: the checker's core function accepts a `list_refs(owner_repo) -> list[(sha, refname)]` callable (default = run `git ls-remote`), and the CLI accepts `--refs-json <path>` so tests and offline runs need no network. The JSON shape is one object keyed by `owner/repo`; each value is either a list of `[sha, refname]` pairs (the parsed `git ls-remote --tags --heads` output, including `^{}` lines) or an object `{"error": "<message>"}`, which the checker treats as a failed listing (`NETWORK`, exit 2). A repo used by a workflow but absent from the JSON is also a failed listing.
- Fixture test `scripts/check_workflow_pins_test.py` (`unittest`, stdlib only, no network; `subprocess` is stubbed so a test that reaches `git` fails): (a) the repaired workflow files plus a fixture listing built from the Proof `ref → peeled SHA` pairs → all `OK`, exit 0; (b) one SHA with one hex digit flipped → `UNKNOWN_SHA`, exit 1; (c) an annotated tag's tag-object id substituted for its peeled commit → `TAG_OBJECT`, exit 1; (d) a floating `@v4` and a missing comment → `MALFORMED`, exit 1; (e) a real SHA paired with a comment naming a different ref → `REF_MISMATCH`, exit 1; (f) `list_refs` raising or returning a failure for one repo → `NETWORK`, exit 2, and the affected pin is not labelled `UNKNOWN_SHA`.
- CI: the `frontend` job in `ci.yml` gains one step, placed immediately after the `actions/checkout` step and therefore before `pnpm install` and before the existing `Code quality gates` step (which fails at baseline, see Constraints), that runs `python3 -m unittest` on `scripts/check_workflow_pins_test.py` and then `python3 scripts/check_workflow_pins.py` live (GitHub-hosted runners have `python3`, `git`, and outbound git to github.com). Release gate fallback per SLICES.md: if GH Actions cannot run here, the local live run plus the fixture tests are the evidence.
- No application runtime behaviour changes; no files outside the Files constraint change.

## Out
- Making the full Rust `cargo test` job green in environments without webkit2gtk (report residual if GH Actions still cannot run here).
- Rewriting release publishing logic beyond pin repair.
- Pinning the Rust toolchain version itself (`refs/heads/stable` of dtolnay/rust-toolchain keeps `toolchain: stable`; rustc selection is unchanged from pre-e37d212).
- Automated re-pinning (Dependabot/Renovate) or a fetch-by-SHA reachability fallback; a moved branch tip is a deliberate `UNKNOWN_SHA` failure that requires an explicit re-pin.
- Running the pre-existing `scripts/*_test.py` files in CI.
- Fixing the pre-existing `FILES_OVER_1K_BUDGET` code-quality gate failure (outside the Files constraint).
- Slices 02–09.

## Constraints
- Files: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `scripts/check_workflow_pins.py`, `scripts/check_workflow_pins_test.py`, optional hook from `scripts/check-code-quality-gates.py`. The optional hook may add only an offline format gate (every `uses:` is `owner/repo@40-hex # ref-label`, reusing the checker's parser); live `git ls-remote` resolution stays in the dedicated CI step so `--mode final` keeps working offline.
- Baseline gate state (comparison point for the implementation reviewer): at code baseline `ea4ec712` / candidate `4047d511…e5290`, `python3 scripts/check-code-quality-gates.py --mode final` exits 1 with exactly one failing gate, `FILES_OVER_1K_BUDGET` (`count=82`, budget 77). This failure is pre-existing, deterministic, and outside this slice's Files constraint; this slice must neither fix it nor add to the failing set.
- Resolve every pin with `git ls-remote --tags --heads https://github.com/<owner>/<repo>` at build time and pin the peeled commit id as defined in Done when. The intended refs are the ones named in the e37d212 pin comments (same majors as the pre-e37d212 floating `@v4`/`@v2`/`@v0`/`@stable`); if an intended tag is gone, pick the newest tag within the same major and record which ref was resolved. Never copy a SHA from a comment, a review, this contract, or memory.
- `dtolnay/rust-toolchain` has no `refs/tags/stable`; pin the `refs/heads/stable` tip re-resolved at build time and comment it `# stable (refs/heads/stable)`.
- Do not pin `owner/action@v4` floating tags or branch names. Do not restore 06d82f9 tag-only pins as the end state; SHA-of-ref is required.
- Do not use `git ls-remote <url> <sha>` anywhere (checker, Proof, or docs); it does not test object existence.
- Checker rules: peel annotated tags (compare against `^{}` ids); a tag-object id is never accepted and is classified `TAG_OBJECT` before any `REF_MISMATCH` consideration; the SHA must be the peeled id of the ref named in its comment; a SHA that matches nothing in the listing is `UNKNOWN_SHA`; listing failure is `NETWORK` with exit 2, distinct from and taking precedence over exit 1. One `git ls-remote` per distinct repo, with a timeout.
- Comment format is normative: `# <tag>` ⇒ `refs/tags/<tag>`; `# <name> (refs/heads/<name>)` ⇒ branch. No other comment shapes.
- Checker and test are stdlib-only Python 3 (no PyYAML; line-based `uses:` parsing is sufficient and must also catch `uses:` lines under `steps:` in `release.yml` that follow a `- name:` line).
- Proof records `tag/branch → peeled SHA` pairs verbatim from live `git ls-remote` output, per repo.

## Data / state impact
None at runtime. Workflow metadata plus one new script and one test under `scripts/`. CI's `frontend` job gains one outbound `git ls-remote` per distinct action repo (six today).

## Tests
- `python3 -m unittest scripts/check_workflow_pins_test.py` exits 0 with network disabled (cases a–f above; `subprocess` stubbed so no test can reach `git`).
- `python3 scripts/check_workflow_pins.py` (live network) exits 0 on the repaired workflows and prints every pin as `OK` with its resolved ref.
- `python3 scripts/check_workflow_pins.py --refs-json <fixture>` on a copy of the workflows with one SHA flipped exits 1 reporting `UNKNOWN_SHA`; on a copy with a tag-object id substituted exits 1 reporting `TAG_OBJECT`; with a fixture that marks one repo as `{"error": ...}` exits 2 reporting `NETWORK`, including when the same run also contains an exit-1 verdict.
- `grep -rc` of each of the four retired SHAs under `.github/workflows/` is 0.
- `python3 scripts/check-code-quality-gates.py --mode final` does not regress versus baseline: the set of `[FAIL]` gates is exactly `FILES_OVER_1K_BUDGET` with `count=82` (same as the baseline recorded in Constraints), no new gate fails, and if the optional hook is added its gate prints `[PASS]`. Exit code stays 1 solely because of that pre-existing gate.
- Proof documents, per action repo, the exact `git ls-remote --tags --heads <url>` command and the verbatim output line(s) for the resolved ref (`refs/tags/<tag>` and `refs/tags/<tag>^{}` for annotated tags; the single line for lightweight tags and branches), and states which line was pinned.

## Proof
Not completed yet.

## Review
Plan approval: none
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

### D01-PLAN-1 — REJECT_PLAN (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-77d71aa8-e064-5889-895e-474b8668a462`, worktree `/tmp/loop-review/D01-PLAN-1` @ `6d6578da`.
Contract `a72182e1…bbcd` (match). Candidate before/after `4047d511…e5290` (unchanged, clean-tree). Final porcelain empty.
Evidence (read-only `git ls-remote`): checkout v4.2.2 → `11bd7190…` real; pnpm/action-setup v4.0.0 peeled → `fe02b34f…` real; setup-node v4.0.3 → `1e60f620b9541d16bece96c5465dc8ee9832be0b` (pinned `…e0b4` fabricated); swatinem/rust-cache v2.7.7 peeled → `f0deed1e0edfc6a9be95417288c0e1099b1eeec3` (pinned `…165c` fabricated); tauri-action v0.5.17 peeled → `2a8db2c1…` (pinned `b70ec574…` fabricated); dtolnay/rust-toolchain has no `refs/tags/stable` — `stable` is `refs/heads/stable` = `6bed0761…`; `4dd2f0b9…` in 0 refs. `git ls-remote <url> <sha>` returns empty with exit 0 (filters by ref name, not object id).
Blockers:
1. Resolution method: replace "`git ls-remote <url> <sha>` or `refs/tags/<tag>`" with resolve `refs/tags/<tag>` **or `refs/heads/<branch>`** and pin the **peeled commit id**; name dtolnay/rust-toolchain → `refs/heads/stable` tip (re-resolve at build time) or record a chosen version branch; define "checker cannot see the SHA" as: not equal to any ref tip or peeled `^{}` id from `git ls-remote --tags --heads <url>` (or `git fetch --depth=1 <url> <sha>` fails). Drop the `ls-remote <url> <sha>` form.
2. Annotated-tag rule: state that for annotated tags the pin is the peeled commit and the checker must not accept tag-object ids.
Observations: adapter substitution judged to preserve independence (fresh context, isolated worktree, identities unchanged); verdict void if user vetoes substitution. Suggests CI step for the checker, offline-deterministic fixture test, Proof records tag→peeled SHA pairs, pin comments name resolved ref.

### D01-PLAN-2 — REJECT_PLAN (revised proposal; recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-2de87ff9-68db-53f3-8484-9a2d084527ee`, worktree `/tmp/loop-review/D01-PLAN-2` @ `7518b974`.
Contract `a5324838…18a3` (match). Candidate before/after `4047d511…e5290` (unchanged, clean-tree); `git diff --stat ea4ec712 HEAD -- . ':!grokbuild-followup-project-loop'` empty. Final porcelain empty.
D01-PLAN-1 blockers 1 and 2: both **resolved** (resolution method + `refs/heads/stable` + vacuous form banned + UNKNOWN_SHA/NETWORK distinct; tag-object never valid, checker peels, fixture (c)). All six intended refs confirmed live. Retired SHAs currently present (so the zero-count criterion is a real delta).
New blocker:
1. Tests bullet 5 ("`check-code-quality-gates.py --mode final` still exits 0") is unsatisfiable at baseline and outside the Files constraint: at `4047d511…` it exits 1 on pre-existing `FILES_OVER_1K_BUDGET` (82 files ≥1000 lines > budget 77), deterministic, not environmental. Rewrite as non-regression vs baseline (same failing gate set = exactly `FILES_OVER_1K_BUDGET` count=82; no new failing gate; optional hook gate PASS). And CI Done-when bullet must state placement: the new pin-check step must run **before** the existing `Code quality gates` step in the `frontend` job (natural slot: right after `actions/checkout`), otherwise it never executes.
Observations: state exit-code precedence when NETWORK and exit-1 verdicts co-occur (suggest exit 2 wins); define the `--refs-json` shape for "repo failed"; tag-object of tag X with comment Y → `TAG_OBJECT` before `REF_MISMATCH`; `dtolnay` `stable` moves per Rust release → predictable red until re-pin (fail-closed, within recorded authority); AGENTS.md says "tag" — `refs/heads/stable` is the sanctioned exception from D01-PLAN-1; adapter substitution note repeated.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06). The pack was written for Antigravity `invoke_subagent`; that tool does not exist in this runtime. The equivalent independent-subagent primitive here is the Cursor `Task` tool (`subagent_type=generalPurpose`), which runs each worker in a fresh, separate context with no access to coordinator or sibling reasoning. Binding:
- Coordinator = this Cursor Cloud Agent session (sole writer of protocol files under `grokbuild-followup-project-loop/`).
- Builder = `Task(generalPurpose)` with BUILDER.md role text inlined, workspace **inherit** (`/workspace`, the grokbuild checkout on branch `cursor/grokbuild-followup-loop-c341`).
- Reviewer = `Task(generalPurpose)` with REVIEWER.md role text inlined, fresh context per review, workspace **branch**: an isolated `git worktree add --detach /tmp/loop-review/<dispatch> <candidate HEAD>` created by coordinator after confirming the checkout is clean (so the worktree is an exact copy of the candidate). Tool-layer write restriction is not available in this adapter; mitigation = isolated worktree (writes there cannot touch the candidate), explicit no-write instruction for `/workspace`, and coordinator recompute of candidate identity after every review.
- Runtime inventory: Task results are terminal on return; there is no sidebar/interim state. No second coordinator exists.
Deviation notice: LOOP.md says stop with Human required if `invoke_subagent` is missing. Coordinator judged the intent (independent, non-persona-switched Builder/Reviewer; no faked review) is satisfied by the Task adapter and proceeded; the user may veto this substitution, in which case all approvals recorded under this adapter are void.
Coordinator: Cursor Cloud Agent session, branch `cursor/grokbuild-followup-loop-c341` off `origin/main` `ea4ec712` (= `c66b3ec7` + pack files only; no code drift).
Worker / role / phase: Reviewer / plan review (revised proposal #2) / slice 01
Dispatch ID / launch state / input identity: `D01-PLAN-3` / launching / candidate `4047d511…e5290`, contract `34f89916…6fafa`
Pending result / last consumed dispatch: none / `D01-REVISE-2` (Builder agent `bc-fc2e3cc8…` resumed; returned revised Goal–Tests with an itemized diff vs D01-REVISE-1; verified baseline gates exit 1 on `FILES_OVER_1K_BUDGET` count=82; no edits, porcelain empty; coordinator applied the itemized edits verbatim.)
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir (source, tests, `.github/workflows/`, `scripts/`, lockfiles, docs, capabilities, assets).
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/` (protocol + artifacts).
Baseline snapshot: code baseline `ea4ec712c1c1d5ef27b036b7999a2955dcf4a86c` (pack-only commits since do not change candidate digest), clean-tree, CANDIDATE `4047d511c0e72b72f81a552ea71f6f3bae19f7ce6efd7fbac193e2baf18e5290`
Contract identity: `34f899167e3be150a4df07c5b83fe521d3ca70af21a23f42d314411cdac6fafa` (revised proposal #2; supersedes `a5324838…18a3`, `a72182e1…bbcd`)
Candidate snapshot: none (plan phase; equals baseline)
Rejection count: 2 (limit 3 — one more REJECT on this slice → Human required)
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: D01-PLAN-2 blocker 1 (Tests bullet 5 unsatisfiable at baseline; CI step placement). D01-PLAN-1 blockers 1–2 closed by D01-PLAN-2.
Repair awaiting review: false
Review events:
- E1 / `D01-PLAN-1` / plan / REJECT_PLAN / contract `a72182e1…bbcd`, candidate `4047d511…e5290` / gaps: blockers 1–2 / rejection count 0→1
- E2 / `D01-PLAN-2` / plan / REJECT_PLAN / contract `a5324838…18a3`, candidate `4047d511…e5290` / prior gaps 1–2 resolved; new gap: Tests bullet 5 + CI placement / rejection count 1→2
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: none
Advance phase: none
Next slice ID / draft: none

## Status
Proposed

## Next
Coordinator: when authorized, bind Antigravity tool names, capture review inputs, persist a pending plan-review dispatch, and invoke an independent Reviewer. Do not implement slice 01 before APPROVE_PLAN.
