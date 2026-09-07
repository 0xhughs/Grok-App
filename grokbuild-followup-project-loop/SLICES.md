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
- 02 Deny wildcard IM senders on the live bridge — R4, N2 — code commit `fd142233`, candidate `2a3620d1…390b`, implementation approval `D02-IMPL-1`; archive `slices/02-deny-wildcard-im-senders.md`. Residual: UI/`docs` still offer `*` until slice 09; stored `"*"` ACLs fail closed on enable.
- 03 Gate dangerous IPC — D1, N1, N10 — code commit `0aed78ab`, candidate `d72f7320…52a6`, implementation approval `D03-IMPL-1`; archive `slices/03-gate-dangerous-ipc.md`. Residual: session/composer/project policy still callable from `session-*`; React GlassModal is not a host confirm (slice 09 docs).
- 04 Serve secret names — R7, N3 — code commit `51330f48`, candidate `308f6af6…9b8c`, implementation approval `D04-IMPL-1`; archive `slices/04-serve-secret-names.md`. Residual: official CLI has no `/health`; missing route is Inconclusive (advertise + keep, including non-loopback).
- 05 Honest CLI installer — C2, N8 — code commit `6eecaf7a`, candidate `4e993693…bc85`, implementation approval `D05-IMPL-1`; archive `slices/05-honest-cli-installer.md`. Residual: Setup still classifies first-seen change as `checksum_missing`; docs/i18n until slice 09.
- 06 Restrict leftover headless children — P2 leftover, N4, N5 — code commit `0b536c3d`, candidate `f19f791b…f59c`, implementation approval `D06-IMPL-1`; archive `slices/06-restrict-leftover-headless-children.md`. Residual: `official_aux` / `models_aux` / `wallpaper_source` still resolve YOLO from global only; CLI honour of `--no-subagents` / `--disallowed-tools` remains Unverified.
- 07 0600 every agent-home secret write — S2, N6 — code commit `e5600725`, candidate `66debe65…4338`, implementation approval `D07-IMPL-1`; archive `slices/07-0600-every-agent-home-secret-write.md`. Residual: non-N6 `config.toml` writers (`extensions` / `models_aux` / `relay_stream_proxy` / `official_aux` sibling home) still use umask `fs::write`.
- 08 Path scope and silent replay — S4 leftover, N11, N9 — code commit `62f55936`, candidate `87743f68…82db`, implementation approval `D08-IMPL-1`; archive `slices/08-path-scope-and-silent-replay.md`. Residual: desktop attachments unfiltered; `.grok` / `agent-home-official` `config.toml` not denied; stale `stream.rs` replay comment.

## Now
### 09 Docs match the leftover posture
Goal: remote-security.md, README_EN.md, SECURITY.md state React-vs-host confirm, CLI-install verification, and CI pin-check reality. Remote IM user-facing copy and `docs/llm-wiki/remote-im.md` stop offering `*` for allow-from (i18n keys in `settings-remoteIm.ts` across all 15 locales, `en` authority) and the Remote IM panel save-time check refuses `*`-containing values, matching the slice 02 bridge default. (Amended by coordinator after `D02-DRAFT-1`: within authority under AGENTS "Docs must match code defaults after each slice that changes a default" and the locked R4 decision "error text must not recommend `*`".)
Provides: D7 leftover
Depends on: 01, 02, 03, 05
Target membership: inside
Out: Marketing copy unrelated to these defaults.
