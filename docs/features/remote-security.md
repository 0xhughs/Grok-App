# Remote control security

- Phone mirror defaults to **read-only**; enable “Allow phone to send” for writes (in-app confirm + persistent warning banner while write is on).
- Phone mirror HTTP **defaults to loopback** (`127.0.0.1`). Same-LAN access is opt-in (“Allow same Wi-Fi”, in-app confirm) and rebinds `0.0.0.0`; copy/QR then use the detected LAN IPv4. Token path still required; HTTP stays unencrypted. Cloudflared tunnel publication is strictly opt-in (`tunnel_enabled` / Settings → Publish cloudflared tunnel) and is never started automatically.
- While write is on, the Connect panel lists **allowlisted write RPC categories** and shows a **broad-surface** warning (full allowlist is open; filesystem / desktop-only commands stay blocked).
- Optional **max phone clients** (1–16, default 4): extra WebSocket upgrades get HTTP 503 (soft-fail). Connect panel shows a live cap bar/chip, full/near-full honesty, zero-client empty state when host is up, and never invents clients while stopped.
- Toggling write access writes an audit line to `app.log` (no tokens/URLs). Local write-ACL audit ring (localStorage) also records enable/disable, rotate, host start/stop — never secrets.
- **Regenerate link** requires in-app confirm (mentions connected client count), rotates the token, disconnects old QR sessions; host logs `token_tail` only.
- Auth rejection and host start logs **redact** path tokens / public URLs (`/t/<redacted>/…`, `token_tail`).
- IM allow-from and LINE signature checks ship in 0.1.9+.
- **WeCom webhook authentication**: Official WeCom webhooks authenticate via Tencent signature (`msg_signature`, `timestamp`, `nonce`). Shared-token fallback (`x-grok-wecom-token`) is **disabled by default** (fails closed with 401 Unauthorized); it requires explicit opt-in via instance option `allow_shared_token: true`.

## Security ops surface (overview)

Settings → **Remote control** → **IM** → Bridge overview shows a unified **Security ops** checklist (pure helpers in `src/lib/remoteSecurityOps.ts`):

| Check | Honesty |
|-------|---------|
| Allow-from ACL | Aggregate open (`*`) / restricted / empty across channel instances; link to edit allow-from |
| Inbound rate limit | Soft per-chat + global limiter is always-on in-process; rate-hit posture is warn, never silent drop |
| Bridge health | Listening / degraded / error / stopped from host status |
| Phone mirror write | Default off (read-only); warn when write is enabled |
| Remote YOLO | Off by default. The Settings toggle uses an in-app GlassModal / `setAppDialog` confirm (renderer UI only — not the host gate) |
| Live claim | Never invent live WS/Gateway without Bridge linked |

- **Copy summary** exports a redacted multi-line report (no tokens/URLs).
- **Dangerous-write confirms** inventory lists known in-app confirms (mirror write / LAN bind / rotate / stop / audit clear · remote YOLO · channel delete · timeline clear) — all GlassModal / `setAppDialog`, never `window.confirm`. Those confirms are **renderer UI only**. They are **not** a host IPC confirm and **not** a Tauri command ACL.
- Dangerous IPC named in the host gate (global YOLO, CLI path, mirror start/publish/remote-YOLO, plugin install `--trust`, serve start, ACP address flip) is rejected unless the invoking window label is exactly `main`. A compromised `session-*` / `pet` / `theme-editor` renderer does not get a host dialog; it gets a label reject.
- Risk badge: `ok` · `warn` · `danger` (open ACL + write, or write + auth error → danger).

## CLI-install verification

Setup can download the Grok Build CLI from official mirrors. There is **no** fabricated known-good CLI hash table and **no** published-sidecar default. Official mirrors often omit SHA-256 sidecars; a missing sidecar is not cryptographic verification.

A first-seen digest **change** is a **hard error**, unless the existing allow-unverified UI or `GROK_CLI_ALLOW_UNVERIFIED` override is on. The first-seen store is written mode `0600`. Setup still classifies that first-seen change as `checksum_missing` (kind collapse) — there is no distinct Setup error kind for “hash changed” vs “sidecar missing”.

## CI pin-check reality

Workflow `uses:` pins are 40-hex peeled commit ids from `git ls-remote --tags --heads`, with a `# <ref>` comment. `scripts/check_workflow_pins.py` fails closed on fabricated / malformed / tag-object / mismatched / stale pins (`UNKNOWN_SHA`, `TAG_OBJECT`, `REF_MISMATCH`; listing failure is `NETWORK`).

`dtolnay/rust-toolchain` is pinned to the **`refs/heads/stable` tip** (there is no `refs/tags/stable`). If that branch moves, the checker reports `UNKNOWN_SHA` (fail-closed). That is the sanctioned exception — not a claim that every pin is an immutable tag, and not a claim that CI is always green.
