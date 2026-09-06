# AGENTS.md

## Working rules
- Work only within the active BUILD contract and the SLICES Loop target. Do not reopen Held IDs from SECURITY-AUDIT-0xhughs-grokbuild-c66b3ec.md.
- Inspect code and tests before changing them. Follow this repo’s patterns unless the accepted slice changes them.
- Make ordinary reversible implementation choices independently. Escalate only beyond existing authority or LOOP limits.
- Coordinator alone persists protocol files and archives. Builder writes implementation and proposes proof. Reviewer verifies independently and never fixes the reviewed work.
- Never self-approve. Do not simulate Builder and Reviewer in one context. Use independent Antigravity subagents.
- Shipped means independently accepted work, not permission to deploy or publish.
- Do not disable Grok subagents on the interactive parent session.
- Base work on `c66b3ec7c1fd044fece4bbedc83b8159efd3312d` (one UI commit past harden `e37d212`). Do not merge RongleCat `main`.

## Global invariants
- Ask is the default permission policy. Untrusted projects stay Ask.
- Held gates stay held: P1 Ask download auto-allow off; P4 host-computed scope; P5 exact edit-tool ids; R1 read-method allowlist; R2 `allow_remote_yolo` default false; R3 tunnel opt-in; R5 trusted-project cwd; R6 empty ACP addr; S1 keychain-preferred; D3 iframe no `allow-same-origin`; D6 WeCom fail-closed.
- `official_aux_inject` stays default false.
- Interactive parent keeps subagents enabled. Headless helper children stay restricted.
- Mirror defaults loopback. Remote IM stays off unless the user already enabled it; this loop must not widen `allow_from`.
- Do not invent CI SHAs. Resolve pins with `git ls-remote` on the action repo tag.
- Docs must match code defaults after each slice that changes a default.

## Session start
Read this file, SLICES.md, BUILD.md, LOOP.md, the relevant role file, FINDINGS.md, and repository evidence. Read HANDOFF.md only when active. Coordinator reconciles workers, pending result, counters, identities and advance phase before any dispatch. Do not start a second writer while ownership is unresolved.

## Session end
Return worker results to coordinator for durable persistence. Record the exact next action, blocker, or completion state. Human required stops execution. Blocked permits only its recorded recheck. Complete permits no further work without a new authorized target.
