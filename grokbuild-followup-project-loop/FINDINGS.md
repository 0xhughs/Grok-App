# FINDINGS.md — follow-up coverage

Authoritative audit: SECURITY-AUDIT-0xhughs-grokbuild-c66b3ec.md against HEAD `c66b3ec7c1fd044fece4bbedc83b8159efd3312d`.
Do not reopen Held rows. Close an in-target ID only in the assigned slice proof.

| ID | Sev | Prior verdict | Slice | Close-as |
|---|---|---|---|---|
| P1 | High | Held | — | Do not reopen |
| P2 leftover | High | Partial | 06 | Restricted argv + invoking-session policy on session_title, agent_workflows, streaming_messages_json; batch uses that session |
| P3 | High | Held | — | Do not reopen |
| P4 | High | Held | — | Do not reopen |
| P5 | Medium | Held | — | Do not reopen |
| R1 | High | Held | — | Do not reopen |
| R2 | High | Held | — | Do not reopen |
| R3 | High | Held | — | Do not reopen |
| R4 / N2 | High | Partial | 02 | Rust remote_im `"*"` → deny; error text stops recommending `*` |
| R5 | High | Held | — | Do not reopen |
| R6 | Medium | Held | — | Do not reopen |
| R7 / N3 | Medium | Partial | 04 | Set GROK_AGENT_SECRET and GROK_SERVE_SECRET; probe before advertise |
| S1 | Medium | Held | — | Do not reopen |
| S2 / N6 | Medium | Partial | 07 | All agent-home secret writes through write_private_agent_home_file |
| S3 | Medium | Held | — | Do not reopen |
| S4 leftover | Medium | Held+gaps | 08 | Extra deny paths |
| C1 / N7 | High | Regressed | 01 | Real ls-remote SHAs + pin checker |
| C2 / N8 | Medium | Partial | 05 | Real table or none; first-seen change hard-errors |
| C3 | Low | Held | — | Do not reopen |
| D1 / N1 / N10 | High/Med | Partial | 03 | Reject first-party eval targets; host/main gate on dangerous IPC |
| D2–D6 | — | Held | — | Do not reopen |
| D7 | Low | Held+overclaim | 09 | Docs match leftover posture |
| N9 | Low | New | 08 | Replay auto-answer → cancel unless journaled |
| N11 | Low | New | 08 | path_scope on mirror attachments |
| N12 | Info | New | — | Comment-only; fold into 09 if touched, else leave |

R8, S5, C4: preserve.
