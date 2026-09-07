# BUILD.md

Slice: 09 Docs match the leftover posture
Archive: slices/09-docs-match-the-leftover-posture.md

## Goal
`docs/features/remote-security.md`, `README_EN.md`, and `SECURITY.md` state React-vs-host confirm, CLI-install verification, and CI pin-check reality. Remote IM user-facing copy and `docs/llm-wiki/remote-im.md` stop offering `*` for allow-from. All 15 `settings-remoteIm.ts` catalogs stay in lockstep with `en`. The Remote IM panel save-time check refuses empty and `*`-entry values, matching the slice 02 Rust bridge. New-instance `defaultAcl().allowFrom` is empty, not `"*"`. Ask stays default. Command count stays 423.

## Done when
Close **D7 leftover** only, plus the already-authorized `*` copy + save-time refuse (SLICES Now 09, amended after `D02-DRAFT-1`). Do not reopen Held IDs. Do not implement Later-outside. Do not change Rust ACL (slice 02). Do not change the CLI installer (slice 05). Do not change CI pins (slice 01). Do not start release review. Do not publish.

Live inspect baseline: code HEAD `62f55936` / candidate `87743f68…82db`. Verify live; do not trust stale line numbers blindly.

### D7 leftover — three public docs state leftover posture

Each of `docs/features/remote-security.md`, `README_EN.md`, and `SECURITY.md` must state **all three** facts. Short honest paragraphs. Do not rewrite unrelated marketing. Do not claim GH Actions ran in the implementation session unless that session ran them.

**1. React-vs-host confirm (slice 03 residual)**
- GlassModal / `setAppDialog` confirms are **renderer UI only**. They are **not** a host IPC confirm and **not** a Tauri command ACL.
- Dangerous IPC named in slice 03 is gated by **window label `main`** on the host. A compromised `session-*` / `pet` / `theme-editor` renderer does not get a host dialog; it gets a label reject.
- `remote-security.md:23` must **not** read as “enable requires GlassModal” as if that were the host gate. Keep the inventory that those in-app confirms exist and never use `window.confirm` (`:27`), and add the host/main-only sentence.

**2. CLI-install verification (slice 05 residual)**
Must say, in substance:
- There is **no** fabricated known-good CLI hash table and **no** published-sidecar default.
- Official mirrors often omit SHA-256 sidecars. Missing sidecar is not cryptographic verification.
- First-seen digest **change** is a **hard error**, with the existing allow-unverified UI/env override. First-seen store is 0600.
- Setup still classifies that first-seen change as `checksum_missing` (kind collapse). Docs must not invent a distinct Setup error kind.
Must **not** say the installer ships a known-good table, a published-sidecar default, or that Setup distinguishes first-seen change from a missing sidecar.

**3. CI pin-check reality (slice 01 residual)**
Must say, in substance:
- Workflow `uses:` pins are 40-hex peeled commit ids from `git ls-remote --tags --heads`, with a `# <ref>` comment.
- `scripts/check_workflow_pins.py` fails closed on fabricated / malformed / tag-object / mismatched / stale pins (`UNKNOWN_SHA`, `TAG_OBJECT`, `REF_MISMATCH`; listing failure is `NETWORK`).
- `dtolnay/rust-toolchain` is pinned to **`refs/heads/stable` tip** (no `refs/tags/stable`). If that branch moves, the checker reports `UNKNOWN_SHA` (fail-closed). This is the sanctioned exception, not “all pins are immutable tags”.
Must **not** use `git ls-remote <url> <sha>` as a verification method in the new prose. Must **not** claim CI is always green.

### Allow-from copy — stop offering `*`

**`docs/llm-wiki/remote-im.md`**
- §3.2 `:162`: `allow_from` is comma-separated **explicit** platform user ids; default **empty / required**; catch-all is refused. Keep `/whoami`. Remove “支持 `*`” and default `` `*` + 警告文案 ``.
- Channel table defaults that are `` `*` `` or “建议非 `*`” (`:341`, `:360`, `:385`, `:445`, `:498`, `:548`, `:566`) become empty / required explicit ids. Slack/QQ/LINE rows that already have no `*` default stay, except they must not newly offer `*`.
- Do **not** change `allow_chat` (`:163`) semantics.
- Do **not** change DingTalk stream `"topic": "*"` (not an ACL; not in this file’s Rust).

**i18n — `en` is key authority; 15 locales lockstep**

Rewrite Family A (25 keys) and Family B (6 `*.hint.*Acl` keys) in every `settings-remoteIm.ts`. No new keys. No hardcoded UI copy in components. Placeholders stay (`{n}` on `security.openCount`).

Normative `en` replacements (implementer may tighten wording but **must** satisfy the offer-regex and R4 checks below):

- `err.allowFromRequired`: `Add at least one explicit allow-from sender id before enabling this channel. Empty or catch-all entries are refused.`
- `field.allowFromHelp`: `Comma-separated platform user ids. Send /whoami in IM to learn your id. Catch-all entries are refused.`
- Eight `<channel>.allowFromHelp`: same rule — explicit ids only; no “or *”, no “test only *”.
- Nine `guide.step*`: “set allow-from to explicit sender ids” — **no** “avoid * in production”.
- `health.aclOpen`: `Catch-all allow-from is refused`
- `health.hint.openAcl`: `This channel lists a catch-all allow-from. The bridge refuses it. Replace it with explicit user ids.`
- `security.openCount`: `{n} catch-all refused`
- `security.acl.open_acl`: `Catch-all refused`
- `security.detail.aclOpen`: `At least one channel has a catch-all allow-from. The bridge refuses those entries. Replace them with explicit sender ids.`
- `security.detail.aclEmpty`: `Allow-from is empty — set explicit sender ids before enabling.`
- Six `health.hint.*Acl`: catch-all is refused; restrict to explicit platform ids. Do not say “open allow-from” as if it grants access.

R4: rewritten `err.allowFromRequired` (all 15 locales) must contain the phrase `explicit sender id` (singular or plural, case-insensitive) and must **not** match this case-insensitive offer regex: `\* for any|or \*|\(or \*|\* for all|only for testing|for test only|avoid \*`. Prefer no `*` character in any rewritten Remote IM allow-from string. The copy lockstep test uses the same regex.

Internal type name `open_acl` in `remoteSecurityOps.ts` may stay. Do **not** re-architect `summarizeAllowFrom` / checklist risk math (Out). Copy is what users see.

### Save-time refuse (TS; Feature Parity with slice 02)

**Wildcard rule (normative):** after the existing `parseAllowFromList` split (comma / semicolon / newline) and trim, an entry is a catch-all **iff** it is exactly `*` (same as Rust `is_wildcard_entry`). A token that merely contains `*` (e.g. `alice*`) is a literal id and is **not** refused. `"*"`, `" * "`, `"alice, *"`, `"*, alice"` refuse. This is list-entry parity with the bridge, not `String.includes("*")`.

Export from `src/lib/remoteSecurityOps.ts` (name normative):

`allowFromBlocksSave(raw: unknown): boolean` — `true` when `parseAllowFromList(raw)` is empty **or** any entry has `wildcard: true`; else `false`.

**`performSave` (`RemoteImChannelPanel.tsx`):**
1. Effective allow-from for this call = `override.acl?.allowFrom` if provided, else `override.values?.allow_from` if that string is present, else `acl.allowFrom`.
2. If `allowFromBlocksSave(effective)` → `setFormError(t("settings.remoteIm.err.allowFromRequired")); return false`. **Before** `remoteImSecretsPut`.
3. Persist that effective value to `options.allow_from` and `nextAcl.allowFrom`.
4. Keep the existing empty-field UX (same key, rewritten text). No new modal. No `window.confirm`. No native `<select>`.

**Scan auto-save:** if Weixin/`ownerOpenId` is present, pass it as the effective allow-from for that `performSave` (override). Do not leave `defaultAcl()` / leftover `*` in the gate. Feishu scan without an owner id stays fail-closed (user must type ids). `scan.autoSaveFailed` stays the fallback flash when save returns false.

**`defaultAcl()` (`store.ts`):** `allowFrom: ""` (not `"*"`). Tightens the UI default; does not widen ACL. `createDefaultInstance` inherits this. No settings migration. Leftover stored `"*"` still fail closed on enable (slice 02) and on next Save (this slice).

`#[tauri::command]` count stays **423**. No file outside Files changes.

### Named tests (names normative)

**`remoteSecurityOps.test.ts`**
- `allow_from_blocks_save_for_empty_and_wildcard`
  - blocks: `null`, `undefined`, `""`, `"   "`, `",,"`, `"*"`, `" * "`, `"alice, *"`, `"*, alice"`
  - allows: `"alice"`, `"alice, bob"`, `"alice*"` (literal, not an entry `*`)
  - `allowFromBlocksSave` is the helper `performSave` calls (import/use the named export).

**`store.test.ts`**
- `default_acl_allow_from_is_empty` — `defaultAcl().allowFrom === ""` and `createDefaultInstance("feishu").acl.allowFrom === ""`. Existing presenter / delete tests still pass.

**Copy / lockstep (vitest; may live in `remoteSecurityOps.test.ts`)**
- `settings_remote_im_copy_never_offers_allow_from_wildcard` — for every locale in `LOCALES`, every Family A+B key’s catalog value does **not** match the offer regex (below). `security.openCount` still contains `{n}`. `err.allowFromRequired` in `en` contains `explicit sender id` (case-insensitive).

Existing `remoteSecurityOps` / `store` / i18n tests still pass. Do **not** flip `summarizeAllowFrom("*") === "open_acl"` unless you also rewrite all dependent tests — prefer leave classification, rewrite copy.

Grep (cwd `/workspace`, Proof lists `-n` / counts):

- `rg -n -i '\* for any|or \*|\(or \*|\* for all|only for testing|for test only|avoid \*' src/i18n/messages/*/settings-remoteIm.ts` → **0**
- `rg -n 'allowFrom: "\*"' src/lib/remoteIm/store.ts` → **0**
- `rg -n 'fn allowFromBlocksSave|export function allowFromBlocksSave' src/lib/remoteSecurityOps.ts` → **≥1**
- `rg -n 'allowFromBlocksSave' src/components/RemoteImChannelPanel.tsx` → **≥1**, and the call is **before** `remoteImSecretsPut` in `performSave`
- `rg -n 'defaultAcl' -A 8 src/lib/remoteIm/store.ts` — `allowFrom` is `""`
- `rg -n '支持 `\*`|`\*` \+ 警告|建议非 `\*`' docs/llm-wiki/remote-im.md` → **0**
- `rg -n 'allow_from' docs/llm-wiki/remote-im.md` — Proof lists each hit; no allow-from default/hint offers `*` as a valid grant
- `rg -n 'GlassModal|host confirm|main' docs/features/remote-security.md` — React-vs-host + main-only are present; “enable requires GlassModal” is not left as a host-gate claim
- `rg -n 'known-good|first-seen|checksum_missing|published sidecar' README_EN.md SECURITY.md` — all four concepts present (wording may vary; “known-good” as **absent/removed**)
- `rg -n 'check_workflow_pins|refs/heads/stable|UNKNOWN_SHA' README_EN.md SECURITY.md` — all three present
- `rg -c '#\[tauri::command\]' src-tauri/src` → **423**

## Out
- Marketing copy unrelated to these defaults (README feature bullets, other README locales, contributor galleries).
- Held IDs. Do not reopen P1, P3, P4, P5, R1–R3, R5, R6, S1, S3, C3, D1–D6.
- Rust `allow_from` / `outbound.rs` / `runtime.rs` / `ALLOW_FROM_BLOCKED_ERR` (slice 02). DingTalk stream `"topic": "*"`.
- CLI installer / `cli_install.rs` / Setup error kinds / `GROK_CLI_REQUIRE_CHECKSUM` default (slice 05).
- CI workflow pins / `check_workflow_pins.py` (slice 01).
- `allow_chat` / `adminFrom` / array-shaped ACLs / widening `allow_from`.
- Reclassifying `summarizeAllowFrom` / checklist danger math / `channelHealth.ts` openAcl detection / per-channel `*Config.ts` fixtures that still use `"*"` as leftover stored ACL.
- `docs/llm-wiki/setup.md` (already has missing-sidecar honesty; first-seen sentence lives in README_EN / SECURITY this slice).
- `settingsCatalog` new rows. New Settings keys. New IPC. `CHANGELOG.md` / What's New (leave Unreleased empty; release review).
- rustfmt-rewrite of the 15 dirty Rust files. Any `src-tauri/**` edit. App shell growth. Publishing / deploying. Release review.

## Constraints
- **Files:** `docs/features/remote-security.md`, `README_EN.md`, `SECURITY.md`, `docs/llm-wiki/remote-im.md`, all 15 `src/i18n/messages/{de,en,es,fil,fr,id,it,ja,ko,pt-BR,ru,ta,uk,zh,zh-TW}/settings-remoteIm.ts`, `src/components/RemoteImChannelPanel.tsx`, `src/lib/remoteSecurityOps.ts`, `src/lib/remoteSecurityOps.test.ts`, `src/lib/remoteIm/store.ts`, `src/lib/remoteIm/store.test.ts`. No `Cargo.toml` / `Cargo.lock`. No `src-tauri`. No `App.tsx` / `AppWorkbench.tsx`. Combined line count of those two files must not grow. No new `useState` there.
- Ask remains default. Untrusted projects stay Ask. Do not change `store.rs` `permission_policy` (Rust settings store).
- i18n: `createT` / `t()` only; `en` authority; 15 locales lockstep; `messages.test.ts` key parity + placeholder survival. Prefer **no new keys**.
- Dialogs: existing `setFormError` on the channel panel. No `window.confirm` / `prompt` / `alert`. No native `<select>`. No transparent menus.
- settings IA: no new setting → no `settingsCatalog` edit.
- Prefer no Rust. Save-time check is TypeScript.
- rustfmt: do not touch the post-06 15-file dirty Rust set.
- `CHANGELOG.md`: do **not** rewrite shipped `## [X.Y.Z]`. Prefer **no** CHANGELOG edit (`## [Unreleased]` exists and is empty; What's New is release review).
- Do not claim tests passed unless that session ran them.
- Do not widen `allow_from`. This slice **tightens** UI/docs to the slice 02 deny.

## Data / state impact
- No settings / secret-store migration. No new IPC. Command count **423**.
- New channel instances get empty `allowFrom`. Save & connect refuses empty and catch-all **before** vault write.
- Leftover stored `"*"` / `*`-entry ACLs: still refused at bridge enable (02); next panel save also refuses; health/security copy no longer claims anyone can talk.
- `session_data_mode` default unchanged (shared). Ask default unchanged.
- Residual: `channelHealth` flags only exact `"*"` / empty as `openAcl`; mixed `"alice, *"` leftover is still enable-blocked by Rust; `allow_chat` `*` wiki line; other README locales; `setup.md` first-seen sentence; internal `open_acl` type name.

## Tests
- `pnpm typecheck` (or `pnpm exec tsc -b --pretty false`) — 0 errors on Files.
- `pnpm vitest run src/i18n/messages.test.ts` — lockstep / placeholders / non-empty; `0 failed`.
- `pnpm vitest run src/lib/remoteSecurityOps.test.ts` — existing plus `allow_from_blocks_save_for_empty_and_wildcard` and `settings_remote_im_copy_never_offers_allow_from_wildcard`; `0 failed`.
- `pnpm vitest run src/lib/remoteIm/store.test.ts` — existing plus `default_acl_allow_from_is_empty`; `0 failed`.
- Grep criteria in Done when; each listing + count in Proof.
- Scope: `git diff --stat` vs implementation baseline lists only Files.
- No `cargo test` / rustfmt / clippy required (no Rust).
- No `whatsNew.test.ts` unless CHANGELOG is edited (it must not be).
- Do not claim these passed in this contract.

## Proof
none

## Review
Plan approval: `D09-PLAN-1` APPROVE_PLAN — reviewer `bc-00529b2b-87b0-5be3-8efd-8b39160c8438`, contract `0daea699…064c`, candidate `87743f68…82db`.
Implementation approval: none
Each result records dispatch ID, reviewer identity, verdict, contract identity, snapshot identity, evidence, and criterion-specific blockers.

### D09-PLAN-1 — APPROVE_PLAN (recorded verbatim summary)
Reviewer: Cursor Task generalPurpose subagent, fresh context, agent ID `bc-00529b2b-87b0-5be3-8efd-8b39160c8438`, worktree `/tmp/loop-review/D09-PLAN-1` @ `3b4e1cda`.
Contract `0daea699…064c` (match). Candidate before/after `87743f68…82db` (unchanged, clean-tree). Porcelain empty. Code vs `62f55936` is pack-only.
Judgments: (a) BUILD matches SLICES Now 09; (b) locked constraints not weakened; (c) live “today” descriptions accurate; (d) Out complete; (e) one slice + list-entry `*` rule; (f) vitest/typecheck satisfiable; (g) empty defaultAcl + Weixin override is Feature Parity. No blockers. Observation: `channelSchemaCatalog.ts` ACL default still `"*"` — do not expand Files.

## Loop state
Execution mode / tool adapter: **Cursor Cloud Agent** (adapter substitution, recorded 2026-09-06; full rationale and veto clause in `slices/01-restore-real-ci-pins.md` Loop state). Coordinator = this Cursor Cloud Agent session (sole writer of protocol files). Builder = `Task(generalPurpose)` with BUILDER.md inlined, workspace inherit (`/workspace`). Reviewer = `Task(generalPurpose)` with REVIEWER.md inlined, fresh context per review, isolated `git worktree add --detach /tmp/loop-review/<dispatch> <HEAD>` created after confirming the checkout is clean; tool-layer write restriction unavailable — mitigated by worktree isolation, explicit no-write instruction, and coordinator identity recompute after every review. Task results are terminal on return. No second coordinator.
Coordinator: Cursor Cloud Agent session, branch `cursor/slice-06-restrict-headless-9f74` off `origin/main` `fbb03fc8`.
Worker / role / phase: Builder / implementation / slice 09
Dispatch ID / launch state / input identity: `D09-BUILD-1` / launching / candidate `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db` (code HEAD `62f55936`), contract `0daea6996de17caa3e33ddb0df7151b32378b1c44a4b65a0693603b0dfe0064c`
Pending result / last consumed dispatch: none / `D09-PLAN-1`
Snapshot capture and recheck commands / coverage / exclusions:
- Tool: `bash grokbuild-followup-project-loop/artifacts/identity.sh both [REPO]` (read-only). Candidate = sha256 over `git ls-tree -r HEAD` (mode/type/blob/path) with `grokbuild-followup-project-loop/` excluded, valid only when `git status --porcelain=v1` outside the pack dir is empty; otherwise the script emits a SHA-256 manifest (mode, digest, path, symlink target) of tracked+untracked covered paths and uses its digest. Contract = sha256 over AGENTS.md, LOOP.md, BUILDER.md, REVIEWER.md, `artifacts/identity.sh`, SLICES.md minus Run status/Release evidence/Shipped, and BUILD.md top through `## Tests`.
- Recheck: rerun the same command; compare `CANDIDATE=` and `CONTRACT=`.
- Coverage: entire tracked tree outside the pack dir.
- Exclusions: `target/`, `src-tauri/target/`, `node_modules/`, `dist/`, `grokbuild-followup-project-loop/`.
Baseline snapshot: slice 08 shipped candidate — HEAD `62f55936` (code), clean-tree, CANDIDATE `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db`
Contract identity: `0daea6996de17caa3e33ddb0df7151b32378b1c44a4b65a0693603b0dfe0064c`
Candidate snapshot: HEAD `62f55936` (code commit), clean-tree, CANDIDATE `87743f6806d8c38e72aa364cf761b5d5a0a0fd6ee7532ea8cc89dc1f515a82db`
Rejection count: 0
Consecutive no-progress repairs: 0
Open acceptance gaps / prior failing evidence: none
Repair awaiting review: false
Review events:
- E1 / `D09-PLAN-1` / plan / APPROVE_PLAN / contract `0daea699…064c`, candidate `87743f68…82db` / no gaps / rejection count 0
Budget limit / consumed / measurement: Not configured; do not invent a budget
Blocker / resume status / resume action / recheck condition / deadline: if interrupted before `D09-BUILD-1` returns, re-dispatch `D09-BUILD-1`. Do not publish.
Advance phase: plan approved; implementation launching
Next slice ID / draft: 09 (`D09-BUILD-1` launching)
Environment note: rustc 1.98.1 / webkit2gtk present, same as 02–08.

## Status
Building

## Next
Builder `D09-BUILD-1` implements the accepted 09 contract. After proof: Ready for review / `D09-IMPL-1`.

**Resume action:** launch `D09-BUILD-1` (implement slice 09 only). Do not re-ship 01–08. Do not implement Later-outside work. Do not publish.
