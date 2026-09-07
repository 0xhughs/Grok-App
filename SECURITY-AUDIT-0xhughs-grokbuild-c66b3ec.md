# Security audit — 0xhughs/grokbuild @ c66b3ec

Static re-verification of the "harden project loop (slices 01-06)" landing against the locked close-as list from the v0.2.31 harden pack, plus a fresh hunt for issues in the current tree. Hostile-but-fair. Nothing below is accepted on the strength of a comment, test name, commit message, or walkthrough claim; every verdict traces to a function that was read.

## 0. Audit metadata

| Field | Value |
|---|---|
| Repo | https://github.com/0xhughs/grokbuild (unofficial fork of RongleCat/grok-app; Tauri 2 desktop host around official `grok agent stdio`) |
| `git rev-parse HEAD` | `c66b3ec7c1fd044fece4bbedc83b8159efd3312d` |
| `git log -1 --oneline` | `c66b3ec7 fix(ui): remove obsolete delete case in writeCategoryLabel` |
| HEAD == e37d212? | **No.** HEAD is one commit past `e37d21220a6794784034c60f0621039bc9c69069`. Drift: `git diff --stat e37d212..HEAD` = `src/components/MirrorConnectPanel.tsx | 2 --` (UI label switch-case removal, no security relevance). Everything below applies equally to `e37d212`. |
| Harden base diffed | `06d82f9bb3c2c7b18b49d155fab811c067f32fb3..HEAD` — 50 files, +3688 / −374 (full list in §1a) |
| Date | 2026-09-06 |
| Method | Static read of `src-tauri/`, `src/`, `remote-bridge/`, `scripts/`, `.github/`, `docs/`, `README_EN.md`, `SECURITY.md`. GitHub API used read-only to resolve action tag SHAs and to read the CI run log for HEAD. Public artifact bucket queried with `HEAD`/`GET` to check checksum-sidecar existence (no credentials, no third-party attack). |
| Tests run in this session | Frontend: `npx vitest run` → **573 files / 6964 tests passed** (188 s). remote-bridge: `pnpm test` → **4/4 passed** (`node --test dist/r4.test.js`). **Rust suite NOT run**: `pkg-config --exists webkit2gtk-4.1` / `gtk+-3.0` both fail in this environment; `cargo test` cannot link. No Rust test result in this report is anything other than "the test exists and I read what it asserts". |
| Not tested | No build/run of the app, no live IM / tunnel / mirror session, no signed-artifact verification, no `pnpm audit` / `cargo audit`, no dynamic exploit PoC. |
| CLI-dependent items | Anything relying on Grok Build CLI behaviour (flag acceptance for `--no-subagents` / `--disallowed-tools`, env-var name for `grok agent serve`, `rawInput` field names in permission requests, `@path` attachment handling) is marked **Unverified**. |

`slices/`, `walkthrough.md`, and pack archives are not in this repo and were not required. The application tree alone is graded.

## 1. Executive summary

1. The harden commit is real and substantial: 29 Rust files, both workflows, both capability manifests, the CSP, `HtmlBrowser`, remote-bridge, and docs were touched, and most permission/mirror gates now do what the pack said they should.
2. **The Highs on the local Ask path are closed.** P1 (download auto-allow in Ask), P4 (client-supplied scope), P5 (substring edit tools) are Held; P3 is Held for the shell path with one CLI-dependent title fallback.
3. **The mirror Highs are closed on the wire.** Read-only is an explicit allowlist (R1), remote YOLO refusal covers all four relaxed policies (R2), tunnel defaults off and is a separate toggle (R3), `session.connect` cwd must be a registered trusted project (R5).
4. **Two claimed fixes are not what the pack locked.** C1 (CI pinned to real 40-char SHAs) is **Regressed**: 4 of 6 SHAs are fabricated, CI at HEAD fails in "Set up job" with "Unable to resolve action", so the repo currently ships with zero CI. R4 (bridge `*`/empty `allow_from` is a hard error) is **Partial**: fixed only in the legacy Node `remote-bridge/`; the in-process Rust `remote_im` bridge that the app actually runs still treats `*` as allow-everyone and its own error text recommends it.
5. **R7 is Held on the letter (secret off argv) but the fork's own trap fired.** `serve.rs` sets `GROK_SERVE_SECRET` and actively strips `GROK_AGENT_SECRET`; official Grok Build documents `GROK_AGENT_SECRET`/`--secret`. If the CLI honours the documented name, the child starts without the secret the app advertises. This fails closed (unusable connection string, or an unauthenticated loopback server if the CLI allows secretless serve — Unverified), not open.
6. **C2 is Partial and partly cosmetic.** First-seen hash warn exists but is warn-only by default; the `KNOWN_CLI_HASHES` table for v0.2.111 does not match the real artifact (linux-x86_64 published binary hashes to `f158d0d4…`, table says `c903e1fa…`), which would fail-closed any pinned 0.2.111 install and protects nothing for the current stable.
7. **D1 is Partial.** Six plugin permissions moved to `main-only.json`; all 423 `#[tauri::command]`s remain callable from `main`, `session-*`, `pet`, and `theme-editor`. `side_browser_eval` can `eval()` into the **main** window from any of them (N1). YOLO toggle, mirror tunnel/remote-YOLO toggles, and `manual_cli_path` have React-only confirms.
8. Can a careful user run this locally? **Yes**, with mirror off or loopback-only, remote_im off, and the CLI installed by hand (not via the in-app downloader) until C1/C2 are repaired. Remote IM with `allow_from = "*"` and any Cloudflare tunnel publication must stay off.
9. Docs largely match code now; the one overclaim is that the harden materially protects CLI downloads.

## 1a. Files touched 06d82f9..HEAD (security-relevant subset marked ●)

```
● .github/workflows/ci.yml                   ● src-tauri/src/mirror/rpc.rs
● .github/workflows/release.yml              ● src-tauri/src/mirror/tunnel.rs
● README_EN.md                               ● src-tauri/src/models_aux.rs
● SECURITY.md                                ● src-tauri/src/official_aux.rs
● docs/features/remote-security.md           ● src-tauri/src/path_scope.rs
  remote-bridge/package.json                 ● src-tauri/src/permission.rs
● remote-bridge/src/config/load.ts           ● src-tauri/src/providers.rs
● remote-bridge/src/config/types.ts          ● src-tauri/src/relay_stream_proxy.rs
● remote-bridge/src/config/validate.ts       ● src-tauri/src/remote_im/channels/wecom.rs
● remote-bridge/src/core/acl.ts              ● src-tauri/src/secrets.rs
● remote-bridge/src/grok/args.ts             ● src-tauri/src/serve.rs
● remote-bridge/src/r4.test.ts                 src-tauri/src/serve_tests_ext.rs
● scripts/official-aux-mcp.mjs               ● src-tauri/src/session_manager/connect.rs
● src-tauri/capabilities/default.json        ● src-tauri/src/session_manager/control.rs
● src-tauri/capabilities/main-only.json      ● src-tauri/src/session_manager/events.rs
● src-tauri/tauri.conf.json                    src-tauri/src/session_manager/routing_tests_p2.rs
● src-tauri/src/acp_client.rs                ● src-tauri/src/store.rs
● src-tauri/src/agent_home_config.rs         ● src-tauri/src/wallpaper_source.rs
● src-tauri/src/batch_agents.rs              ● src/components/HtmlBrowser.tsx
● src-tauri/src/cli_install.rs                 src/components/HtmlBrowser.test.tsx
● src-tauri/src/commands/session_p1.rs       ● src/components/MirrorConnectPanel.tsx
● src-tauri/src/commands/settings.rs         ● src/lib/api/mirror.ts
● src-tauri/src/commands/terminal.rs         ● src/lib/mirrorWriteSurface.ts
  src-tauri/src/lib.rs
● src-tauri/src/mirror/http.rs                 src-tauri/src/mirror/lan_bind_test.rs
● src-tauri/src/mirror/mod.rs
```

Files the pack implies but that are **unchanged** in this range: `src-tauri/src/remote_im/outbound.rs` / `runtime.rs` / `engine.rs` (the live bridge ACL — see R4), `src-tauri/src/session_title.rs`, `src-tauri/src/agent_workflows.rs`, `src-tauri/src/streaming_messages_json.rs` (headless children still on unconditional `--always-approve` — see P2), `src-tauri/src/agent_config_edit.rs` and siblings (`fs::write` of `config.toml` without 0600 — see S2), `src-tauri/src/side_browser_host.rs`.

## 2. Claimed-fix scorecard

Verdict key: **Held** = locked close-as implemented; **Partial** = implemented on some paths / weaker than locked; **Claim not held** = e37d212 said fixed, code does not implement the locked close-as; **Regressed** = worse than 06d82f9 on the graded property.

| ID | Prior sev | Claimed | Verdict | file:line | Note / attack path if not Held |
|---|---|---|---|---|---|
| P1 | High | Fixed | **Held** | `src-tauri/src/permission.rs:620-627`, `:690-694` | `may_auto_allow_download` returns false for `Ask`/`Deny`/`DontAsk` and for any chained command; `may_auto_allow` additionally short-circuits `!Ask` before calling it. Test `download_into_project_prompts_in_ask` (`permission.rs:1304`) asserts the Ask case. |
| P2 | High | Fixed | **Partial** (locked design holds for the 4 named children; 3 unlisted children still unconditional) | `official_aux.rs:215-236`, `models_aux.rs:875-886`, `wallpaper_source.rs:592-605`, `batch_agents.rs:67-79`, `scripts/official-aux-mcp.mjs:455-478`, `store.rs:787` | `official_aux_inject: false` default Held. Named children add `--no-subagents --disallowed-tools …` and only push `--always-approve` when policy is YOLO. **But** (a) `agent_workflows.rs:499` (no tool restriction, project cwd), `session_title.rs:185` (tool blocklist present, cwd unset), `streaming_messages_json.rs:184` still pass `--always-approve` unconditionally (see N4); (b) `batch_agents.rs:177-183` derives "parent policy" from `sessions.first()` of the global index, not the invoking session (N5); (c) `official_aux`/`models_aux`/`wallpaper` use the **global** policy, not the invoking session's — an Ask session in a YOLO-global config still gets YOLO aux children; (d) whether the CLI honours `--no-subagents`/`--disallowed-tools` is **Unverified**. |
| P3 | High | Fixed | **Held** (one CLI-dependent fallback) | `permission.rs:235-243`, `:368-384`, `:1065-1071`; `session_manager/events.rs:348-357`; `control.rs:1005-1020` | Key = `tool:normalize_shell_command(rawInput.command)`; `SessionAllowCache::allow` and `compute_resolved_scope` both refuse chained keys (`; & \| \` $( \n`). Fallback to `title` at `events.rs:350-356` only when no `rawInput.command` is present — the key is then the full normalized title, not its first word. Whether Grok Build ever omits `command` is Unverified. |
| P4 | High | Fixed | **Held** | `commands/session_p1.rs:289,295`; `control.rs:817-834`, `:1005-1020`; `mirror/rpc.rs:351-374` | Both IPC and mirror pass scope through, but `resolve_permission` binds it as `_scope` and recomputes from `pending_permission_ui.scope_key`. Info: `client_options` (`control.rs:880-894`) is still consulted for wire-option-id coercion when the host's pending option list is empty; the CLI validates option ids against its own request, so worst case is a cancelled turn. |
| P5 | Medium | Fixed | **Held** | `permission.rs:64-84` | Exact-match `matches!` on lower-cased tool id; no `contains("edit")`. |
| R1 | High | Fixed | **Held** | `mirror/rpc.rs:16-42`, `:90-92` | `dispatch` rejects anything not in `READ_METHODS`. `session.connect`, `resolvePlan`, `resolveAskUser`, `voice.transcribe` are in `WRITE_METHODS`. UI list `src/lib/mirrorWriteSurface.ts:33-39` matches. Side-effects of "read" methods (billing refresh in `account.status`, index reconcile in `sessions.list`) are host-internal, not attacker-controlled writes. |
| R2 | High | Fixed | **Held** | `mirror/mod.rs:59-61,87`; `mirror/rpc.rs:283-341` | `allow_remote_yolo` defaults `false` unless `GROK_MIRROR_ALLOW_REMOTE_YOLO`. `session.send` resolves the session's effective policy and refuses `always_approve`/`dont_ask`/`auto`/`accept_edits`. Test `allow_remote_yolo_enforcement` (`rpc.rs:662`) covers all four. Info: `session.send` `attachments[].path` (`rpc.rs:470-500`) is appended verbatim as `@path` prompt refs (`session_manager/types.rs:1652`); the host never reads them — file access is the CLI's permission problem (Unverified). |
| R3 | High | Fixed | **Held** | `mirror/mod.rs:56-58,75,86`, `:258-270`, `:847-860`; `mirror/http.rs:93`; `mirror/lan.rs` | `publish_tunnel` default false; `GROK_MIRROR_NO_TUNNEL=1` hard-disables. Separate IPC `mirror_set_publish_tunnel` (`mod.rs:1180`). `mirror_start` also accepts `publish_tunnel: Option<bool>` in the same call (`mod.rs:851`) — explicit, not silent. Host-side confirm: none (React only) — see D1/N2. |
| R4 | High | Fixed | **Partial → effectively Claim not held for the live bridge** | Node: `remote-bridge/src/core/acl.ts`, `config/validate.ts`, `config/load.ts`, `grok/args.ts`, `r4.test.ts` (Held). Rust: `src-tauri/src/remote_im/outbound.rs:278-305`, `remote_im/runtime.rs:70-71` | The app's shipping bridge is the in-process Rust `remote_im`. There, `allow_from_list` maps `"*"` → `None` and `sender_allowed` maps `None` → `true` (everyone). Empty is refused at enable time (`runtime.rs:70`), but the error string literally says "add your user id (or `*` for any)". Attack: any IM user who can reach the bot drives the agent. |
| R5 | High | Fixed | **Held** | `commands/session_p1.rs:29-40`; `session_manager/connect.rs` (`resolve_connect_cwd`); `mirror/rpc.rs:211-233`; `commands/terminal.rs` (`validate_terminal_project_path`) | Untrusted or unregistered `projectPath` → error; cwd resolution falls back to the general workspace, never to an arbitrary path. |
| R6 | Medium | Fixed | **Held** | `store.rs:358-361,768-769`; `acp_client.rs:5823-5919`; `commands/settings.rs:756-780` | Default `None`; `0.0.0.0`/unspecified rejected; non-loopback requires `confirm_remote_acp_server = true`. |
| R7 | Medium | Fixed | **Held on argv / Partial on function** | `serve.rs:504-506`, `:165`, `:629-656`, `:697` | `--secret` is not on argv; `cmd.env("GROK_SERVE_SECRET")` + `env_remove("GROK_AGENT_SECRET")`. Non-loopback bind logs `warn!` and sets `exposure_warning`. **Trap:** official docs name `GROK_AGENT_SECRET`/`--secret`; the app removes exactly that variable. See N3. `ps` cannot read the secret either way. |
| S1 | Medium | Fixed | **Held** | `store.rs:682`; `secrets.rs:94`, `:328`, `:522` | Default `keychain_platform_ok()`; fallback file is `0o600`; keychain error falls back to the private file rather than losing keys (`save_secrets_fallback_to_private_file` test at `secrets.rs:1181`). |
| S2 | Medium | Fixed | **Partial** | Held: `agent_home_config.rs` (`write_private_agent_home_file`), `providers.rs`. Not held: `agent_config_edit.rs:604`, `agent_privacy.rs:316`, `agent_codebase_indexing.rs:299`, `permission_rules.rs:440`, `agent_memory_embed.rs:665`, `mcp_oauth.rs:771,857` | Six other writers of `agent-home/config.toml` (which SECURITY.md:33 admits carries provider keys) and MCP OAuth token files use bare `fs::write`, inheriting umask (typically 0644). See N6. |
| S3 | Medium | Fixed | **Held** (minor gaps) | `store.rs:2894-2945`, `:2993`, `:3041`, `:3087` | JWT (`eyJ` 3-part), Telegram `id:secret`, Slack `xox[abprs]-`, GitHub `gh[posur]_`, Bearer spans. Gaps: `github_pat_` fine-grained tokens, Slack `xoxe-` refresh tokens, Discord bot tokens. |
| S4 | Medium | Fixed | **Held** (gaps) | `path_scope.rs:94-135` | Denies `.ssh`, `.aws`, `.gnupg`, `.config/gh`, `secrets.json`, `session-api.json`, `.grok`/`agent-home`/app-data `auth.json`, `remote-im/*.json`. Not denied: `agent-home/config.toml` (provider keys), `~/.netrc`, `~/.kube/config`, `~/.docker/config.json`, `~/.npmrc`. |
| C1 | Medium | Fixed | **Regressed** | `.github/workflows/ci.yml:17,19,20,55,56,68`; `release.yml:75,81,84,105,110,286,381,412` | All refs are 40-hex, but `actions/setup-node@1e60f620…`, `dtolnay/rust-toolchain@4dd2f0b9…`, `swatinem/rust-cache@f0deed1e…`, `tauri-apps/tauri-action@b70ec574…` do not exist upstream (GitHub API 422 "No commit found"). `actions/checkout@11bd7190…` and `pnpm/action-setup@fe02b34f…` are genuine. CI run 34060637460 at HEAD: every job fails in "Set up job" with `Unable to resolve action`. 06d82f9 had working tag pins; HEAD has no CI at all. See N7. |
| C2 | Medium | Fixed | **Partial** | `cli_install.rs:81-108`, `:161`, `:240-260`, `:355-375`, `:869-874`, `:944-985` | Published sidecar → fail-closed on mismatch (Held, but the code's own comment at `:363` says none are published, and none were found). `KNOWN_CLI_HASHES` for 0.2.111 is wrong (real `grok-0.2.111-linux-x86_64` = `f158d0d43367c3959c5ad213327255ac5991a0ec4c67bb475e09cc8cdba4a7b3`, table = `c903e1fa…`). First-seen store is `warn!`-only; `GROK_CLI_REQUIRE_CHECKSUM` is opt-in. Default install path is "download over TLS from allowlisted host, record hash, continue". See N8. |
| C3 | Low | Fixed | **Held** | `mirror/tunnel.rs:32` | `cloudflare/cloudflared@sha256:6efbe6aa…`. |
| D1 | Medium | Fixed | **Partial** | `capabilities/default.json:5`, `main-only.json:5-13`; `commands/terminal.rs:148-156`; `side_browser_host.rs:103-149`; `commands/settings.rs:314,392-393,782` | Moved to main-only: `core:webview:allow-create-webview`, `allow-create-webview-window`, `updater:allow-check/download/install`, `process:allow-restart`. Everything else (all **423** `#[tauri::command]`s) is reachable from `main`, `session-*`, `pet`, `theme-editor` — Tauri command allowlisting is not used. Dangerous IPC with React-only confirm: `settings_set{permission_policy}`, `settings_set{manual_cli_path}`, `mirror_start`, `mirror_set_publish_tunnel`, `mirror_set_allow_remote_yolo`, `plugin_install` (`--trust`), `side_browser_eval`. See N1, N2. |
| D2 | Medium | Fixed | **Held** (residual) | `src-tauri/tauri.conf.json:33` | `img-src` no longer `https:`; `connect-src` `ws:` narrowed to `127.0.0.1`/`localhost`. Residual: `connect-src … wss:` wildcard; `style-src 'unsafe-inline'`. |
| D3 | Medium | Fixed | **Held** | `src/components/HtmlBrowser.tsx:104` | `sandbox="allow-scripts"`, no `allow-same-origin`. Test asserts exactly this (run: passed). |
| D4 | Low | Fixed | **Held** | `mirror/http.rs:69-86`, `:119-128` | Token lives in the URL path (`/t/{token}/…`), not only in an inline script; token never logged. Residual: path-token leaks through `Referer` to any off-origin resource the SPA loads. |
| D5 | Medium | Fixed | **Held** | `relay_stream_proxy.rs:177`, `:358`, `:409-426` | Loopback bind; `Host` must be loopback, `Origin` (if present) loopback or Tauri scheme, `Sec-Fetch-Site: cross-site` → 403. |
| D6 | Medium | Fixed | **Held** | `remote_im/channels/wecom.rs:209-220`, `:316-333` | `allow_shared_token` read from instance options, default false; shared-token path only taken when true. |
| D7 | Low | Fixed | **Held** (one overclaim) | `README_EN.md:68,100`; `SECURITY.md:31-36`; `docs/features/remote-security.md:4,11,23,27` | Defaults described match code (keychain default, loopback mirror, tunnel opt-in, WeCom fail-closed, remote YOLO off). Overclaims: `remote-security.md:23` "enable requires GlassModal confirm" is React-only (host IPC has none); nothing in docs discloses that the CLI downloader's checksum path is currently warn-only. |
| R8 | Info | — | No regression | `mirror/http.rs:93`, `lan.rs:73` | LAN bind still explicit opt-in. |
| S5 | Info | — | No regression | `session_api.rs:685-696`, `:713-718` | `session-api.json` written 0600; bearer / `x-grok-token` gate unchanged. |
| C4 | Info | — | No regression | `release.yml` | SHA256SUMS still produced. |

## 3. New findings

Severity scale: Critical / High / Medium / Low / Info. "Preconditions" is the attacker's starting position.

### Group: Remote RCE / auth bypass

**N1 — `side_browser_eval` executes arbitrary JS in any webview, including `main`** — **High**
- File: `src-tauri/src/commands/terminal.rs:148-156`; `src-tauri/src/side_browser_host.rs:103-115` (`validate_label`), `:142-149` (`get_side_webview`), `:685-701` (`eval`).
- Attack path: any renderer holding the `default` capability (`main`, `session-*`, `pet`, `theme-editor` — `capabilities/default.json:5`) calls `invoke("side_browser_eval", { label: "main", script })`. `get_side_webview` uses `validate_label` (charset check only) and `app.get_webview(label)`, **not** `validate_side_label` (`:117-123`, which enforces the side-browser prefix). The script runs with the main window's origin and full IPC → every one of the 423 commands, including `settings_set{permission_policy:"always_approve"}`, `settings_set{manual_cli_path}`, `mirror_start{publish_tunnel:true, allow_remote_yolo:true}`.
- Preconditions: JS execution in any of the four first-party webviews (XSS in rendered agent output, theme pack, pet overlay, or a compromised secondary window). Side-browser webviews themselves load external URLs but carry no capability, so they cannot invoke — this is a **cross-window privilege escalation**, not a web-to-native one.
- Evidence: `validate_side_label` exists and is used by create/navigate paths; `eval`/`snapshot` skip it.
- Fix: route `eval`/`snapshot`/`install_download_hook` through `validate_side_label`; reject labels equal to any first-party window label; move `side_browser_*` to `main-only.json`.

**N2 — Remote IM (Rust) accepts `allow_from = "*"` and recommends it** — **High** (R4 Partial)
- File: `src-tauri/src/remote_im/outbound.rs:278-305`; `src-tauri/src/remote_im/runtime.rs:70-71`; `src-tauri/src/remote_im/engine.rs:2182` (test fixture uses `"*"`).
- Attack path: operator follows the in-product error text and sets `*`; any sender who can message the bot (public Telegram bot, Discord server member, Slack workspace guest…) drives the local agent. `allow_remote_yolo` (`runtime.rs:52,101`) limits *policy*, not *who*.
- Preconditions: any remote_im channel enabled with `*`.
- Evidence: `if raw == "*" { return None; }` → `sender_allowed` `None => true`.
- Fix: treat `*` exactly like empty (`Some(vec![])`) in `allow_from_list`; drop "(or `*` for any)" from `runtime.rs:71`; delete the `"*"` fixture. Port the Node `r4.test.ts` assertions to Rust.

### Group: Secret leak / auth failure

**N3 — `grok agent serve` secret set under a name the CLI may not read** — **Medium** (fails closed; functional break of R7)
- File: `src-tauri/src/serve.rs:504-506` (`env("GROK_SERVE_SECRET")`, `env_remove("GROK_AGENT_SECRET")`), `:190-202` (connection string advertises `GROK_SERVE_SECRET=…`).
- Attack path: none directly. Two outcomes depending on CLI behaviour (**Unverified**): (a) CLI requires a secret → child exits, feature dead; (b) CLI allows secretless serve → an **unauthenticated** `grok agent serve` on the configured bind while the UI displays a secret it believes is enforced. With `is_non_loopback_bind` true (`:165`, `:696-697`) that is an unauthenticated agent on the LAN with only a log warning.
- Preconditions: user enables Serve; non-loopback bind for outcome (b) to matter beyond localhost.
- Evidence: official Grok Build documents `GROK_AGENT_SECRET` / `--secret`; the code strips the documented variable and sets an undocumented one. `serve_tests_ext.rs` asserts the env var name the app chose, so the test cannot catch the mismatch.
- Fix: set **both** `GROK_AGENT_SECRET` and `GROK_SERVE_SECRET` (env, never argv); on start, probe the child (health endpoint with and without the secret) and refuse to advertise a connection string if an unauthenticated request succeeds; block non-loopback bind unless the probe proves auth.

**N6 — `agent-home/config.toml` and MCP OAuth token files written with umask permissions** — **Medium** (S2 Partial)
- File: `src-tauri/src/agent_config_edit.rs:604`, `agent_privacy.rs:316`, `agent_codebase_indexing.rs:299`, `permission_rules.rs:440`, `agent_memory_embed.rs:665`, `mcp_oauth.rs:771`, `:857`.
- Attack path: local unprivileged user on a shared host reads provider API keys from `config.toml` (SECURITY.md:33 acknowledges keys live there) or MCP OAuth tokens.
- Preconditions: multi-user machine or permissive umask.
- Evidence: `write_private_agent_home_file` (0600) was added in `agent_home_config.rs` and adopted by `providers.rs`, but not by these six writers, all of which rewrite the same file.
- Fix: replace each `fs::write` with `write_private_agent_home_file`; add a test that greps `src-tauri/src` for `fs::write(` on paths under `agent-home`.

### Group: Permission bypass

**N4 — Three headless children still run with unconditional `--always-approve`; `agent_workflows` has no tool restriction at all** — **Medium** (P2 residual not covered by the pack's four children)
- File: `src-tauri/src/agent_workflows.rs:490-507` (`workflow_run_args`: `--always-approve`, `--max-turns 4|8`, **no** `--no-subagents`/`--disallowed-tools`; `:716` `current_dir(&cwd)` = project), `src-tauri/src/session_title.rs:178-192` (`--always-approve` unconditional, but `--max-turns 2 --no-subagents --disable-web-search --disallowed-tools …`; no `current_dir`, so the child inherits the app's cwd), `src-tauri/src/streaming_messages_json.rs:182-195` (`--always-approve --max-turns 1`, fixed probe prompt, cwd = temp dir).
- Attack path: `agent_workflows` runs multi-step YOLO turns inside the project directory with every tool enabled regardless of the session's Ask policy — a workflow prompt or a repo file it reads can drive shell/write tools with no gate. `session_title` feeds the user's first 400 chars into a YOLO turn; the tool blocklist (Unverified CLI behaviour) is the only thing between a prompt-injected transcript and execution. The streaming probe is fixed-prompt and effectively inert.
- Preconditions: workflow feature used (agent_workflows); auto-title on (session_title); attacker controls content reaching the prompt.
- Evidence: `agent_workflows.rs:1040` test **asserts** `--always-approve` is present — a test that locks the risky behaviour in. `session_title` and `agent_workflows` are unchanged in `06d82f9..HEAD`.
- Fix: `agent_workflows`: gate `--always-approve` on the invoking session's effective policy and add the same `--no-subagents --disallowed-tools` set as `official_aux`; `session_title`: drop `--always-approve` (a text-only reply needs no approvals) and pin `current_dir` to temp; flip the `:1040` assertion.

**N5 — `run_batch_headless` picks the parent policy from `sessions.first()`** — **Medium**
- File: `src-tauri/src/batch_agents.rs:170-183`.
- Attack path: user has one old YOLO session anywhere in the index; every batch run from any Ask project inherits `--always-approve` because the global index's first entry is used as "the parent".
- Preconditions: mixed-policy session history (common).
- Evidence: `let first = sessions.first(); let parent_policy = first.map(|s| s.permission_policy…)` — no session id is passed in.
- Fix: thread the invoking `session_id` into `run_batch_headless` and resolve `effective_permission_policy` for that session and its project.

**N9 — Load-replay heuristic auto-approves any permission request when no prompt is in flight** — **Low / Unverified**
- File: `src-tauri/src/session_manager/events.rs:322-345`; `session_manager/stream.rs:148-158`.
- Attack path: if the CLI ever emits a `request_permission` while `prompt_in_flight == false && deferred_prompt_complete.is_none()` (background hook, `/loop` task, late subagent, post-`prompt_complete` tool), the host answers `allow_once` silently. Pre-exists 06d82f9 (only a 5-line touch in this range); listed because P1–P5 are graded "no silent execution in Ask" and this is the one remaining silent-allow path.
- Preconditions: CLI behaviour outside the strict request/response turn model — **Unverified**.
- Fix: on replay, respond `Cancelled`/deny instead of `allow_once`, or only auto-resolve when the tool call id matches a journaled historical call.

### Group: Supply chain

**N7 — Four fabricated action SHAs; CI is dead at HEAD** — **High** (C1 Regressed)
- File: `.github/workflows/ci.yml:20,56,68`; `.github/workflows/release.yml:84,105,110,286`.
- Attack path: not exploitation — loss of control. No lint/test/build gate has run since e37d212; release.yml cannot run either, so any future "release" would be a manual upload with no reproducible build path. If someone "fixes" this by re-resolving tags without verifying, they may pin a SHA an attacker wants.
- Preconditions: none.
- Evidence: `gh api repos/{owner}/{repo}/git/commits/<sha>` → 422 for the four SHAs; CI run `34060637460` annotations: "Unable to resolve action `dtolnay/rust-toolchain@4dd2f0b9…`", "`swatinem/rust-cache@f0deed1e…`", "`actions/setup-node@1e60f620…`". Real tag SHAs are resolvable via `git ls-remote --tags` on each action repo.
- Fix: replace with SHAs resolved from `git ls-remote https://github.com/<owner>/<repo> refs/tags/<tag>`; add a workflow step (or `scripts/check-code-quality-gates.py` rule) that verifies each `uses:` SHA exists via `git ls-remote` so a fabricated pin fails locally before push.

**N8 — CLI downloader: fabricated known-good table, warn-only default** — **Medium** (C2 Partial)
- File: `src-tauri/src/cli_install.rs:81-108` (table), `:944-985` (gate), `:869-874` (`require_published_checksum` default false), `:240-260` (first-seen `warn!`), `:266` and `:630` (hash store / sidecar written without 0600).
- Attack path: MITM is prevented by TLS + host allowlist (`:550-563`), so the real exposure is **bucket compromise or upstream replacement**: any binary served from the allowlisted bucket installs on first download with a warning nobody sees. A user who pins `0.2.111` gets a spurious fail-closed error because the table entry is wrong.
- Preconditions: compromise of the artifact bucket, or a stale/malicious mirror added to the allowlist.
- Evidence: published sidecar candidates (`:355-375`) return 404; downloaded `grok-0.2.111-linux-x86_64` hashes to `f158d0d4…`, table says `c903e1fa…`; `FirstSeenStatus::Changed` only logs.
- Fix: delete the fabricated table or regenerate it from real downloads with the generating command committed; make `Changed` a hard error with an explicit UI override; default `GROK_CLI_REQUIRE_CHECKSUM` on and surface the "unverified" state in Settings; write `first_seen_hashes.json` 0600.

### Group: Defense in depth

**N10 — YOLO, CLI path, mirror exposure toggles are React-confirm only** — **Medium** (D1 Partial)
- File: `src-tauri/src/commands/settings.rs:314,392-393` (`permission_policy` applied on any `settings_set`), `:782` (`validate_manual_cli_path` checks existence/executability only), `src-tauri/src/mirror/mod.rs:847-860,1180-1194`, `src-tauri/src/commands/extensions_p2.rs:474` (`plugin install … --trust`), `src/components/MirrorConnectPanel.tsx` (GlassModal confirms).
- Attack path: with N1 or any IPC reach, one call flips the whole app to YOLO, points `manual_cli_path` at an attacker-controlled executable (any existing regular executable passes validation), or publishes the mirror. No host-side confirm, no rate limit, no "was this window the main window" check.
- Preconditions: same as N1.
- Fix: put these commands behind a host-side confirm (native dialog or a signed one-shot nonce issued only to `main`) and/or restrict them via Tauri command permissions to `main`; require `manual_cli_path` to be inside a known install root or match a first-seen hash.

**N11 — `session.send` attachment paths pass unfiltered to the agent prompt** — **Low**
- File: `src-tauri/src/mirror/rpc.rs:470-500`; `src-tauri/src/session_manager/types.rs:1652-1672`.
- Attack path: a phone/mirror client with the token attaches `{"path":"/home/u/.ssh/id_ed25519"}`; the host does not read it, but the prompt gains `@/home/u/.ssh/id_ed25519` and the CLI's read tool decides. In an `Ask` session this prompts; in a remote-YOLO-enabled session it does not.
- Fix: apply `path_scope::is_allowed` to attachment paths in `param_attachments`.

**N12 — Stale doc comment claims env is passed as `env KEY=VAL` on the `wsl.exe` argv** — **Info**
- File: `src-tauri/src/wsl_backend.rs:4` vs `:434-486` (`start_wsl_tokio_command`) and `:492-519` (`WSLENV`).
- Reality: env is set on the Windows process and forwarded through `WSLENV`; nothing secret is on the command line. The header comment is wrong and could mislead a future maintainer into "restoring" argv env. Fix the comment.

### Group: Harden-commit regression

- **N7** (above) is the only strict regression: 06d82f9 pinned by tag and worked; HEAD pins by non-existent SHA and does not run.
- `agent_workflows.rs:1040` (N4) is a test that asserts the old bug.

## 4. Inventory

### 4a. Tauri IPC surface

| Capability file | Windows | Permissions | Commands reachable |
|---|---|---|---|
| `capabilities/default.json:5` | `main`, `session-*`, `pet`, `theme-editor` | `core:default`, window/webview geometry, `store`, `window-state`, `notification`, `deep-link` | **All 423** `#[tauri::command]` functions (`rg -c "#\[tauri::command\]" src-tauri/src` = 423). Tauri app-command allowlisting is not configured, so capability files only scope plugin permissions. |
| `capabilities/main-only.json:5-13` | `main` | `core:webview:allow-create-webview`, `allow-create-webview-window`, `updater:allow-check/download/install`, `process:allow-restart` | Plugin permissions only. |
| Side-browser webviews (`side_browser_host.rs:350`, `WebviewUrl::External`) | label prefix from `LABEL_PREFIX` | none (no capability matches) | none — but they are the *target* of N1. |

Dangerous commands still callable from every first-party window (subset): `settings_set` (YOLO, CLI path, ACP addr), `session_connect`, `terminal_pty_spawn`, `mirror_start`, `mirror_set_publish_tunnel`, `mirror_set_allow_remote_yolo`, `serve_start`, `plugin_install`, `side_browser_eval`, `side_browser_snapshot`, `batch_agents_*`, `fs_*`, `remote_im_*`. Host-side confirmation: none of them.

### 4b. Network listeners

| Listener | File:line | Bind | Auth | Started when |
|---|---|---|---|---|
| Mirror HTTP/WS | `mirror/http.rs:93-94` | `listen_ip(allow_lan)` → 127.0.0.1 default, 0.0.0.0 on LAN opt-in | path token `/t/{token}/…` (`:69-128`) | `mirror_start` |
| Mirror LAN discovery | `mirror/lan.rs:73` | UDP 0.0.0.0:0 (outbound probe for local IP) | n/a | LAN opt-in |
| Cloudflared tunnel | `mirror/tunnel.rs:32` | container/binary pinned by digest | mirror token | `publish_tunnel` true only |
| Media server | `media_server.rs:117-118` | 127.0.0.1:0 | token + `path_scope::require_allowed` | app start |
| Relay stream proxy | `relay_stream_proxy.rs:177` | 127.0.0.1:0 | Host/Origin/Sec-Fetch-Site gate (`:409-426`) | on demand |
| Session API | `session_api.rs:1084` | 127.0.0.1:0 | Bearer / `x-grok-token` (`:713-718`); endpoint file 0600 | when enabled |
| MCP OAuth callback | `mcp_oauth.rs:503` | 127.0.0.1:0 | state param | during OAuth |
| SSH skills browser | `ssh_remote/skills_browser.rs:352` | 127.0.0.1:0 | none beyond loopback | during SSH browse |
| WeCom webhook | `remote_im/channels/wecom.rs:322-339` | 127.0.0.1 unless `allow_external` | Tencent signature; shared token only if `allow_shared_token` | channel enabled |
| LINE webhook | `remote_im/channels/line.rs:43-57` | 127.0.0.1 unless `allow_external` | LINE signature | channel enabled |
| `grok agent serve` (child) | `serve.rs:504-506,628-656` | user-configured; non-loopback warns | secret via `GROK_SERVE_SECRET` env (see N3) | Serve enabled |
| ACP over TCP (client) | `acp_client.rs:5823-5919` | connects to `acp_server_addr`; 0.0.0.0 rejected; non-loopback needs confirm | — | when set |

### 4c. Headless spawn sites

| Site | File:line | `--always-approve` | `--no-subagents` / `--disallowed-tools` | cwd |
|---|---|---|---|---|
| official_aux (Rust) | `official_aux.rs:215-236` | only if **global** policy YOLO | yes | temp |
| official-aux MCP | `scripts/official-aux-mcp.mjs:455-478` | if `OFFICIAL_AUX_YOLO`/`OFFICIAL_AUX_POLICY` env (snapshotted at config generation) | yes | `os.tmpdir()` |
| models_aux | `models_aux.rs:875-886` | only if global YOLO | yes | — |
| wallpaper_source | `wallpaper_source.rs:592-605` | only if global YOLO | yes | — |
| batch_agents | `batch_agents.rs:67-79,170-183` | if "parent" (= `sessions.first()`) YOLO — N5 | yes | project |
| session_title | `session_title.rs:178-192` | **always** — N4 | yes (`--max-turns 2`, `--disable-web-search`) | unset (inherits app cwd) |
| agent_workflows | `agent_workflows.rs:490-507,716` | **always** — N4 | **no** | project |
| streaming_messages_json probe | `streaming_messages_json.rs:182-195` | **always** (fixed prompt, `--max-turns 1`) | no | temp |
| streaming_acp_ndjson probe | `streaming_acp_ndjson.rs` | caller flag | — | — |
| remote_im turns | `remote_im/grok_agent.rs:202-212`, `control_plane.rs:870-921` | from parent session policy + `allow_remote_yolo` | — | project |

### 4d. Permission path (Ask)

`events.rs:346-357` builds `scope_key(tool, rawInput.command | path | title)` → `may_auto_allow` (`permission.rs:659-700`): outside-project → only YOLO; Deny/DontAsk → false; session cache hit → true; AcceptEdits + exact edit tool → true; `!Ask` + in-project download → true; else prompt. Cache population only via `resolve_permission` → `compute_resolved_scope` (`control.rs:1005-1020`) from the host's own pending request; chained keys never cached (`permission.rs:1065-1071`). One silent path remains: load-replay `allow_once` (`events.rs:334-345`, N9).

## 5. Test and docs honesty

| Test named in walkthrough | Exists? | What it actually proves | Can pass with bug present? |
|---|---|---|---|
| `read_only_blocks_all_writes` | `mirror/rpc.rs:580` | Every `WRITE_METHODS` entry + 4 arbitrary names get the read-only error when `read_only=true`. Companion `all_dispatch_methods_classified` (`:603`) checks READ∪WRITE == a hard-coded 19-method list. | Yes, if a new `dispatch` arm is added and not added to the hard-coded list — the test does not introspect `dispatch`. Currently the list matches the 19 arms (`rpc.rs:96-417`). Not run (Rust). |
| `allow_remote_yolo_enforcement` | `mirror/rpc.rs:662` | All four relaxed policies refused with `allow_remote_yolo=false`; `ask` passes to NOT_READY; `true` passes. Real store round-trip. | No for the gate itself. Not run (Rust). |
| `download_into_project_prompts_in_ask` | `permission.rs:1304` | `may_auto_allow_download(Ask, …)` false and `may_auto_allow(Ask, …)` false for in-project curl. | No. Not run (Rust). |
| `r4.test.ts` | `remote-bridge/src/r4.test.ts` | Node bridge: default mode, `isSenderAllowed` rejects undefined/empty/`*`, `validateConfig` errors, no `--always-approve` in default args. **Ran: 4/4 pass.** | **Yes for the shipping product** — it tests the legacy Node bridge; the Rust `remote_im` ACL has no equivalent test and accepts `*` (N2). |
| `HtmlBrowser.test.tsx` | `src/components/HtmlBrowser.test.tsx` | `sandbox="allow-scripts"` and no `allow-same-origin`. **Ran: pass.** | No. |
| "workflow SHA test" | **Does not exist.** No test or script in the tree parses `.github/workflows/*.yml` for 40-hex pins (`rg` over `scripts/`, `src/lib/*.test.ts`, `scripts/check-code-quality-gates.py`). | — | The absence is why four fabricated SHAs shipped (N7). |
| `serve_tests_ext.rs` (R7) | yes | Asserts `GROK_SERVE_SECRET` in env and `--secret` absent from argv. | **Yes** — it encodes the app's chosen variable name, so it cannot detect the documented-name mismatch (N3). |
| `cli_install.rs:1200-1222` (C2) | yes | Table lookups round-trip against the table itself. | **Yes** — tautological; a wrong hash in the table passes (N8). |
| `agent_workflows.rs:1040` | yes | Asserts `--always-approve` **is present**. | Locks the risky behaviour in (N4). |

Suites run in this session (real output):

```
$ npx vitest run
 Test Files  573 passed (573)
      Tests  6964 passed (6964)
   Duration  187.68s

$ cd remote-bridge && pnpm test
# tests 4  # pass 4  # fail 0

$ cargo test   # NOT RUN — webkit2gtk-4.1 / gtk+-3.0 absent; crate cannot link here
```

Docs that still overclaim: `docs/features/remote-security.md:23` ("enable requires GlassModal confirm" — React-only); README/SECURITY are silent that in-app CLI install is checksum-warn-only and that CI is non-functional. `SECURITY.md:33` is honest that `agent-home/config.toml` holds keys — which makes the S2 gap (N6) a documented-but-unprotected secret store.

## 6. Subagent / isolation note

The GUI does not isolate a "Builder" from a "Reviewer". Every headless child (`official_aux`, `models_aux`, `wallpaper_source`, `batch_agents`, `session_title`, `agent_workflows`) runs under the same OS user, same `GROK_HOME` (shared mode = `~/.grok`), same keychain, and same network. Isolation is **process-wide flags only** (`--no-subagents`, `--disallowed-tools`, conditional `--always-approve`), and whether those flags are honoured is CLI behaviour the app does not control (Unverified). Policy for aux children is resolved from the **global** setting, not the invoking session, so an Ask session inside a YOLO-global install spawns YOLO helpers. No seccomp / sandbox / separate home for helpers. `--disallowed-tools` names (`run_terminal_cmd,run_terminal_command,search_replace,write,Agent,spawn_subagent,bash,bash_tool`) are a different list from `is_edit_tool` (`permission.rs:64-84`) — e.g. `apply_patch`, `create_file`, `delete_file`, `notebook_edit` are edit tools to the host but not disallowed for helpers.

## 7. Residual risk

- No build/run: interaction between capability files and Tauri's runtime command routing was read, not executed.
- No live IM, tunnel, or mirror session; no `cloudflared` pulled; no signed-artifact verification of releases.
- No `pnpm audit`, no `cargo audit`; `remote-bridge/` required `pnpm install --ignore-workspace` to build, which suggests it is not exercised by the (currently dead) CI.
- Rust unit tests not executed here; the Rust verdicts rest on reading the functions and the assertions, not on green output.
- All Grok Build CLI flag/env semantics: Unverified.
- The 423-command surface was inventoried by count and by dangerous-subset spot check, not by reading every command body.

## 8. Recommended next Loop target

Only items graded Partial / Claim not held / Regressed / new N#. One slice per row; each has a mechanical acceptance check.

| Slice | IDs | Scope | Accept when |
|---|---|---|---|
| 01 | C1 / N7 | Replace 4 fabricated SHAs in `ci.yml` + `release.yml` with `git ls-remote`-resolved tag SHAs; add `scripts/check-workflow-pins.py` that fails if any `uses:` SHA is not resolvable (run in CI and in `check-code-quality-gates.py`). | CI run at HEAD green in "Set up job"; pin check fails on a deliberately mangled SHA. |
| 02 | R4 / N2 | `remote_im/outbound.rs`: `"*"` → empty list; `runtime.rs:71` message stops recommending `*`; remove `"*"` fixture; Rust tests mirroring `r4.test.ts` (undefined, empty, `*` all denied). | Rust ACL tests exist and assert deny; `sender_allowed(json!({"allowFrom":"*"}), "x") == false`. |
| 03 | D1 / N1 / N10 | `side_browser_eval`/`snapshot`/`install_download_hook` use `validate_side_label` and reject first-party labels; move `side_browser_*`, `settings_set{permission_policy,manual_cli_path,acp_server_addr}`, `mirror_start`, `mirror_set_*`, `plugin_install`, `serve_start` behind a host-issued one-shot confirm nonce or Tauri command permissions scoped to `main`. | Test: `eval(label="main")` errors; test: `settings_set{permission_policy:"always_approve"}` from a non-main label errors. |
| 04 | R7 / N3 | `serve.rs`: set `GROK_AGENT_SECRET` **and** `GROK_SERVE_SECRET`; post-start probe refuses to advertise a connection string if an unauthenticated health request succeeds; refuse non-loopback bind until probe passes. | Test asserts both env names present, `--secret` absent; probe logic unit-tested with a fake server. |
| 05 | C2 / N8 | Remove or regenerate `KNOWN_CLI_HASHES` from real downloads (commit the generator); `FirstSeenStatus::Changed` → hard error with UI override; default `GROK_CLI_REQUIRE_CHECKSUM=on` semantics with Settings toggle; 0600 on hash store. | Table test compares against a checked-in `KNOWN_CLI_HASHES.sha256` produced by the generator; install with changed first-seen hash errors. |
| 06 | P2 / N4 / N5 | `session_title`, `agent_workflows`, `streaming_messages_json`: add `--no-subagents --disallowed-tools …`, pin cwd, drop unconditional `--always-approve` (gate on invoking session's effective policy); `run_batch_headless(session_id)`; aux children resolve policy from invoking session, not global. Flip `agent_workflows.rs:1040` to assert absence in Ask. | Each spawn-site test asserts flag set for Ask and YOLO parents separately. |
| 07 | S2 / N6 | Route all six `fs::write` sites for `agent-home/config.toml` and `mcp_oauth` token files through `write_private_agent_home_file`; add a repo test that fails on `fs::write(` under `agent-home` paths. | `stat -c %a` of `config.toml` after each writer == 600 in tests. |
| 08 | S4 / N11 / N9 | `path_scope`: deny `agent-home/config.toml`, `~/.netrc`, `~/.kube`, `~/.docker/config.json`, `~/.npmrc`; apply `is_allowed` to mirror `attachments[].path`; change load-replay auto-answer from `allow_once` to cancel unless the tool call id matches a journaled call. | Unit tests for each new deny; replay test asserts `Cancelled`. |
| 09 | D7 | `remote-security.md:23` qualify confirm as UI-level until Slice 03 lands; README/SECURITY note CLI-install verification state and CI status. | Docs diff reviewed against Slices 01/03/05 outcomes. |
