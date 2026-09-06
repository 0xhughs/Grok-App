# KICKOFF.md

Paste the block below as the first message in a new Antigravity session. Replace the two paths.

```text
Start executing the grokbuild follow-up project loop now.

Pack directory:
<PACK_DIR>

Grokbuild repo (HEAD c66b3ec7c1fd044fece4bbedc83b8159efd3312d):
<GROKBUILD_REPO>

Read AGENTS.md, SLICES.md, BUILD.md, LOOP.md, BUILDER.md, REVIEWER.md, FINDINGS.md, and SECURITY-AUDIT-0xhughs-grokbuild-c66b3ec.md in the pack directory. Treat slices 01 to 09 as the authorized Loop target. Do not reopen Held IDs. Do not implement Later-outside work. Do not publish or deploy.

You are the coordinator. Use independent Antigravity subagents:
- invoke_subagent name=builder, workspace inherit or share on <GROKBUILD_REPO>
- invoke_subagent name=reviewer, workspace branch or an exact isolated copy, no write tools

If those agents are not registered, define them from <PACK_DIR>/.agents/agents/builder.md and reviewer.md, then invoke. If invoke_subagent is unavailable, stop and record Human required. Do not persona-switch in this session.

Begin with slice 01 plan review (restore real CI pins). Then implement, test, review, repair, and advance through 01–09 automatically. Make routine implementation decisions independently. Do not ask whether to continue after each accepted slice.

Preserve AGENTS.md invariants. Require real verification. Do not invent action SHAs — resolve with git ls-remote. Do not claim cargo test passed unless this session ran it. Maintain durable progress, dispatch IDs, snapshot identities, and review records in BUILD.md / SLICES.md. Continue until the target passes its release gates or a documented blocker, required user decision, permission boundary, or retry/resource limit prevents further work. Persist the exact resume action if interrupted.

Copy or symlink <PACK_DIR>/.agents/agents/* into <GROKBUILD_REPO>/.agents/agents/ if Antigravity only discovers workspace agents from the repo root.
```
