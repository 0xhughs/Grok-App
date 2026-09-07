# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.2.x   | Yes       |
| 0.1.x   | Yes       |

## Reporting a Vulnerability

If you discover a security issue in Grok App (for example token leakage, unsafe
agent process spawning, or local secrets exposure), please report it privately:

- Open a GitHub Security Advisory on [RongleCat/grok-app](https://github.com/RongleCat/grok-app), or
- Contact the maintainer on X: [@cgnot996](https://x.com/cgnot996)

Please include:
- A clear description of the issue
- Steps to reproduce
- Impact assessment if known

Do **not** open a public issue for sensitive vulnerabilities until a fix is available.

## Local security notes

- **API keys** (`officialApiKey`, `relayApiKey`) prefer the **OS secret store** and are enabled by default where supported:
  - macOS: Keychain
  - Windows: Credential Manager
  - Linux: FreeDesktop Secret Service (when available)
  - Fallback: `secrets.json` under the app data root with mode `0600` when the OS store is unavailable or if keychain operations fail (preventing data loss)
- Non-secret metadata (`relayBaseUrl`, `defaultModel`) may remain in `secrets.json`. On first load after upgrade or when keychain is preferred, any plaintext keys still on disk are **migrated into the OS store** and cleared from the file (logged without values).
- Custom provider keys may also be written to the independent agent home (`agent-home/config.toml`); they are **not** moved into the OS keychain by this path — do not commit them.
- Prefer official Grok login / local CLI auth over pasting long-lived keys into chats.
- Automations and YOLO permission mode can run agent actions without per-step prompts — enable only if you trust the session.
- Support zip / Doctor export / **session diagnostic package** never include `secrets.json`, OS keychain material, or raw API keys (redacted logs and chat only).

## React-vs-host confirms

In-app GlassModal / `setAppDialog` confirms are **renderer UI only**. They are **not** a host IPC confirm and **not** a Tauri command ACL. Dangerous IPC is gated by window label `main` on the host. A compromised `session-*` / `pet` / `theme-editor` renderer does not get a host dialog; it gets a label reject.

## CLI-install verification

The setup wizard can download the Grok Build CLI. There is **no** known-good CLI hash table and **no** published sidecar default. Official mirrors often omit SHA-256 sidecars; a missing sidecar is not cryptographic verification. A first-seen digest **change** is a **hard error**, with the existing allow-unverified UI/env override. The first-seen store is mode `0600`. Setup still classifies that first-seen change as `checksum_missing` (kind collapse) — it does not invent a distinct Setup error kind.

## CI pin-check

Workflow `uses:` pins are 40-hex peeled commit ids from `git ls-remote --tags --heads`, with a `# <ref>` comment. `scripts/check_workflow_pins.py` fails closed on fabricated / malformed / tag-object / mismatched / stale pins (`UNKNOWN_SHA`, `TAG_OBJECT`, `REF_MISMATCH`; listing failure is `NETWORK`). `dtolnay/rust-toolchain` is pinned to the **`refs/heads/stable` tip** (no `refs/tags/stable`). If that branch moves, the checker reports `UNKNOWN_SHA` (fail-closed). This is the sanctioned exception, not a claim that CI is always green.
