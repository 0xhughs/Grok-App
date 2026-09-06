# SLICES.md

## Product
Close the leftover and new findings in SECURITY-AUDIT-0xhughs-grokbuild-c66b3ec.md on the unofficial 0xhughs/grokbuild fork. Held Highs from e37d212 stay held. Local Ask-mode use remains viable. Interactive Grok subagents stay on.

Current release boundary: C1/N7, R4/N2, D1/N1/N10, R7/N3, C2/N8, P2/N4/N5, S2/N6, S4/N11/N9, D7. Do not reopen Held P1, P3, P4, P5, R1, R2, R3, R5, R6, S1, S3, C3, D2, D3, D4, D5, D6.

## Users
The owner running this pack in Google Antigravity against a local clone of https://github.com/0xhughs/grokbuild.

## Product principles
- Fail closed. Do not reintroduce `allow_from = "*"`, fabricated pins, or Ask-path silent execution.
- Grade the live Rust `remote_im` path, not the unused Node `remote-bridge`.
- Proof is tests plus default-config inspection. Do not claim cargo tests passed unless this session ran them.
- Subagent isolation for this loop is Antigravity’s, not grok-app’s.

## Loop target
Slices 01–09 inclusive. Stop when Now is `None — target complete` and release gates pass. Do not implement Later-outside work. Do not publish or deploy.

## Run status
Running

## Open decisions
None remaining. Locked from the audit:
- C1: replace fabricated SHAs with `git ls-remote` resolved tag SHAs; add a pin checker.
- R4: `"*"` is deny on the Rust bridge; error text must not recommend `*`.
- D1: `side_browser_eval` must reject first-party labels; dangerous IPC needs host-side or main-only gating.
- R7: set both `GROK_AGENT_SECRET` and `GROK_SERVE_SECRET`; keep `--secret` off argv.
- C2: no fabricated known-good table; first-seen change is a hard error with UI override.
- P2 leftovers: `session_title`, `agent_workflows`, `streaming_messages_json` follow the same restricted-child contract as official_aux.
- S2: all agent-home secret writes go through `write_private_agent_home_file`.

## Release gates
- Every target ID is Held with a named test or inspect note in shipped slice proof.
- CI “Set up job” can resolve every `uses:` SHA (or the pin-check script fails a mangled SHA locally if GH Actions cannot run here).
- Rust ACL test denies `allow_from=*`. `side_browser_eval("main")` errors.
- Default install posture from the first harden pack is unchanged except where this target tightens it.
- README_EN.md, SECURITY.md, docs/features/remote-security.md match the new defaults.

## Release evidence
Pending finalization.
Failed release reviews for this target: 0
Pending release result: none
Release review events / last consumed dispatch: none

## Shipped
- 01 Restore real CI pins — C1, N7 — code commit `73193a0e`, candidate `8c79e574…341e`, implementation approval `D01-IMPL-1`; archive `slices/01-restore-real-ci-pins.md`. Note: `dtolnay/rust-toolchain` pinned to `refs/heads/stable` tip (no tag exists) — sanctioned exception; checker reports UNKNOWN_SHA when the branch moves (fail-closed).

## Now
### 02 Deny wildcard IM senders on the live bridge
Goal: Rust `remote_im` treats `*` and empty as deny. Error text does not recommend `*`.
Provides: R4, N2
Depends on: 01
Target membership: inside
Out: Deleting the legacy Node `remote-bridge/` package.

## Later
### 03 Gate dangerous IPC
Goal: `side_browser_eval` cannot target `main`/`session-*`/`pet`/`theme-editor`. YOLO, CLI path, mirror publish, plugin `--trust`, and serve start require host-side confirm or main-only command permissions.
Provides: D1, N1, N10
Depends on: 01
Target membership: inside
Out: Redesigning the 423-command surface in one slice.

### 04 Serve secret names
Goal: Child serve gets `GROK_AGENT_SECRET` and `GROK_SERVE_SECRET`. `--secret` stays off argv. Non-loopback advertise only after an unauthenticated probe fails.
Provides: R7, N3
Depends on: 01
Target membership: inside
Out: Changing official CLI source.

### 05 Honest CLI installer
Goal: `KNOWN_CLI_HASHES` is generated from real downloads or removed. First-seen hash change is a hard error with UI override. Hash store is 0600.
Provides: C2, N8
Depends on: 01
Target membership: inside
Out: Hosting a new artifact bucket.

### 06 Restrict leftover headless children
Goal: `session_title`, `agent_workflows`, `streaming_messages_json` get `--no-subagents --disallowed-tools`, pinned cwd, and `--always-approve` only from the invoking session’s policy. Batch uses that session, not `sessions.first()`.
Provides: P2 leftover, N4, N5
Depends on: 01
Target membership: inside
Out: Disabling parent-session subagents.

### 07 0600 every agent-home secret write
Goal: The six bare `fs::write` sites for `config.toml` and MCP OAuth tokens use `write_private_agent_home_file`.
Provides: S2, N6
Depends on: 01
Target membership: inside
Out: Rewriting the whole agent-home module.

### 08 Path scope and silent replay
Goal: Deny `agent-home/config.toml`, `~/.netrc`, `~/.kube`, `~/.docker/config.json`, `~/.npmrc`. Mirror attachments go through `path_scope`. Load-replay auto-answer is cancel unless the tool call is journaled.
Provides: S4 leftover, N11, N9
Depends on: 03
Target membership: inside
Out: Verifying live Grok Build `@path` semantics.

### 09 Docs match the leftover posture
Goal: remote-security.md, README_EN.md, SECURITY.md state React-vs-host confirm, CLI-install verification, and CI pin-check reality.
Provides: D7 leftover
Depends on: 01, 03, 05
Target membership: inside
Out: Marketing copy unrelated to these defaults.
