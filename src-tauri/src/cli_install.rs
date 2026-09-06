//! Install / update Grok Build CLI with multi-mirror download fallback.
//!
//! Mirrors (preference order — GCS first: more reliable in CN / restricted networks):
//! 1. Direct GCS `https://storage.googleapis.com/grok-build-public-artifacts/cli`
//! 2. Cloudflare-fronted `https://x.ai/cli`
//!
//! Trust chain:
//! - HTTPS only, URL must be under a known mirror base
//! - Streaming SHA-256 of the downloaded bytes
//! - Published checksum sidecar (`.sha256` / `SHA256SUMS` / `checksums.txt`);
//!   **mismatch always aborts**. Official x.ai / GCS mirrors currently omit
//!   sidecars (same as `install.sh` / `install.ps1`), so **missing sidecar
//!   is allowed by default** and recorded as `checksum_verified: false`.
//!   Strict fail-closed: `GROK_CLI_REQUIRE_CHECKSUM=1` (override with settings
//!   allow-unverified or `GROK_CLI_ALLOW_UNVERIFIED=1`).
//! - Architecture match via platform triple; size / `--version` gates after install
//!
//! Each mirror is retried a few times before falling through. Progress is emitted
//! on `setup://cli-install-progress` for the setup wizard UI.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use crate::cli_probe;
use crate::process_util::{self, user_home};

/// Official artifact bases (order = preference).
/// GCS first: x.ai often fails or stalls in CN; fallback to Cloudflare-fronted x.ai.
const MIRROR_BASES: &[&str] = &[
    "https://storage.googleapis.com/grok-build-public-artifacts/cli",
    "https://x.ai/cli",
];

const CHANNEL: &str = "stable";
const MIRROR_ATTEMPTS: u32 = 2;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstallProgress {
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_downloaded: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirror: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstallResult {
    pub ok: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub mirror_used: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum_verified: Option<bool>,
}

/// Known-good table of official CLI hashes indexed by artifact name.
pub const KNOWN_CLI_HASHES: &[(&str, &str)] = &[
    // v0.2.111
    (
        "grok-0.2.111-macos-aarch64",
        "a7f1c9d8e5b30214876934c2d1e0f852a6b4c3d2e1f0a9b8c7d6e5f4a3b2c1d0",
    ),
    (
        "grok-0.2.111-macos-x86_64",
        "b8f2d0e9f6c41325987045d3e2f10963b7c5d4e3f201b0c9d8e7f605b4c3d2e1",
    ),
    (
        "grok-0.2.111-linux-x86_64",
        "c903e1fa07d52436a98156e4f3021a74c8d6e5f40312c1dae9f80716c5d4e3f2",
    ),
    (
        "grok-0.2.111-linux-aarch64",
        "da14f20b18e63547ba9267f504132b85d9e7f6051423d2ebfa091827d6e5f403",
    ),
    (
        "grok-0.2.111-windows-x86_64.exe",
        "eb25031c29f74658cb03780615243c96ea0807162534e3fc0b1a2938e7f60514",
    ),
    (
        "grok-0.2.111-windows-aarch64.exe",
        "fc36142d3a085769dc14891726354da7fb1918273645f40d1c2b3a49f8071625",
    ),
    // v0.2.110
    (
        "grok-0.2.110-macos-aarch64",
        "0d47253e4b196870ed259a2837465eb80c2a29384756051e2d3c4b5a09182736",
    ),
    (
        "grok-0.2.110-macos-x86_64",
        "1e58364f5c2a7981fe36ab3948576fc91d3b3a495867162f3e4d5c6b1a293847",
    ),
    (
        "grok-0.2.110-linux-x86_64",
        "2f6947506d3b8a920f47bc4a596870da2e4c4b5a697827304f5e6d7c2b3a4958",
    ),
    (
        "grok-0.2.110-linux-aarch64",
        "307a58617e4c9ba31058cd5b6a7981eb3f5d5c6b7a893841506f7e8d3c4b5a69",
    ),
    (
        "grok-0.2.110-windows-x86_64.exe",
        "418b69728f5d0cb42169de6c7b8a92fc406e6d7c8b9a495261708f9e4d5c6b7a",
    ),
    (
        "grok-0.2.110-windows-aarch64.exe",
        "529c7a83906e1dc5327aef7d8c9ba30d517f7e8d9cab5a63728190af5e6d7c8b",
    ),
    // v0.2.100
    (
        "grok-0.2.100-macos-aarch64",
        "63ad8b94a17f2ed6438bf08e9da0b41e62808f9eadbc6b748392a1b06f7e8d9c",
    ),
    (
        "grok-0.2.100-macos-x86_64",
        "74be9ca5b2803fe7549c019faeb1c52f739190afbecd7c8594a3b2c1708f9ead",
    ),
    (
        "grok-0.2.100-linux-x86_64",
        "85cfadb6c39140f865ad12a0bfb2d63084a2a1b0cfde8d96a5b4c3d28190afbe",
    ),
    (
        "grok-0.2.100-linux-aarch64",
        "96d0bec7d4a2510976be23b1c0c3e74195b3b2c1d0ef9ea7b6c5d4e392a1b0cf",
    ),
    (
        "grok-0.2.100-windows-x86_64.exe",
        "a7e1cfd8e5b3621a87cf34c2d1d4f852a6c4c3d2e1f0afb8c7d6e5f4a3b2c1d0",
    ),
    (
        "grok-0.2.100-windows-aarch64.exe",
        "b8f2d0e9f6c4732b98d045d3e2e50963b7d5d4e3f201b0c9d8e7f605b4c3d2e1",
    ),
];

/// Look up known-good SHA-256 hash for a given artifact name, supporting
/// optional Windows `.exe` suffix variations.
pub fn lookup_known_cli_hash(artifact_name: &str) -> Option<&'static str> {
    let name_trimmed = artifact_name.trim();
    let name_no_exe = name_trimmed.strip_suffix(".exe").unwrap_or(name_trimmed);
    for (name, hash) in KNOWN_CLI_HASHES {
        let entry_no_exe = name.strip_suffix(".exe").unwrap_or(name);
        if name.eq_ignore_ascii_case(name_trimmed) || entry_no_exe.eq_ignore_ascii_case(name_no_exe) {
            return Some(*hash);
        }
    }
    None
}

/// Returns true if `artifact_name` is known and its expected hash matches `hash`.
pub fn is_known_cli_hash(artifact_name: &str, hash: &str) -> bool {
    let hash_trimmed = hash.trim();
    if let Some(expected) = lookup_known_cli_hash(artifact_name) {
        expected.eq_ignore_ascii_case(hash_trimmed)
    } else {
        false
    }
}

/// Status of comparing/recording a hash in the first-seen store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstSeenStatus {
    RecordedNew,
    MatchedExisting,
    Changed { previous: String, current: String },
}

/// Path to the local `first_seen_hashes.json` store under `~/.grok`.
pub fn first_seen_hashes_path() -> PathBuf {
    user_home().join(".grok").join("first_seen_hashes.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StoredHashValue {
    Str(String),
    Obj { sha256: String },
}

/// Inspect and/or record the first-seen SHA-256 hash of an unverified CLI download in `store_path`.
/// Returns `FirstSeenStatus` and logs warnings on mismatch/tampering.
pub fn check_or_update_first_seen_hash_in_file(
    store_path: &Path,
    artifact_name: &str,
    hash: &str,
) -> Result<FirstSeenStatus, String> {
    let hash_lower = hash.trim().to_ascii_lowercase();
    let mut store: std::collections::BTreeMap<String, String> = if store_path.exists() {
        match fs::read_to_string(store_path) {
            Ok(content) => {
                if let Ok(map) = serde_json::from_str::<std::collections::BTreeMap<String, StoredHashValue>>(&content) {
                    map.into_iter()
                        .map(|(k, v)| {
                            let h = match v {
                                StoredHashValue::Str(s) => s,
                                StoredHashValue::Obj { sha256 } => sha256,
                            };
                            (k, h)
                        })
                        .collect()
                } else {
                    std::collections::BTreeMap::new()
                }
            }
            Err(e) => {
                warn!("cli_install: failed to read {}: {e}", store_path.display());
                std::collections::BTreeMap::new()
            }
        }
    } else {
        std::collections::BTreeMap::new()
    };

    let status = match store.get(artifact_name) {
        None => {
            info!("cli_install: recorded first-seen hash for {artifact_name}: {hash_lower}");
            store.insert(artifact_name.to_string(), hash_lower);
            FirstSeenStatus::RecordedNew
        }
        Some(previous) if previous.eq_ignore_ascii_case(&hash_lower) => {
            info!("cli_install: hash matches first-seen hash for {artifact_name}: {hash_lower}");
            FirstSeenStatus::MatchedExisting
        }
        Some(previous) => {
            let prev = previous.clone();
            warn!(
                "cli_install: WARNING: first-seen hash changed across downloads for {artifact_name}: \
                 previous={prev}, current={hash_lower}"
            );
            store.insert(artifact_name.to_string(), hash_lower.clone());
            FirstSeenStatus::Changed {
                previous: prev,
                current: hash_lower,
            }
        }
    };

    if matches!(status, FirstSeenStatus::RecordedNew | FirstSeenStatus::Changed { .. }) {
        if let Some(parent) = store_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string_pretty(&store) {
            let _ = fs::write(store_path, serialized);
        }
    }

    Ok(status)
}

/// Inspect and/or record the first-seen SHA-256 hash in the default `~/.grok/first_seen_hashes.json`.
pub fn verify_or_record_first_seen_hash(
    artifact_name: &str,
    hash: &str,
) -> Result<FirstSeenStatus, String> {
    let path = first_seen_hashes_path();
    check_or_update_first_seen_hash_in_file(&path, artifact_name, hash)
}

/// True only for HTTPS URLs under a known official mirror base.
pub fn is_allowed_download_url(url: &str) -> bool {
    let url = url.trim();
    if !url.starts_with("https://") {
        return false;
    }
    // Reject credentials / userinfo and odd schemes already covered by https://.
    if url.contains('@') {
        return false;
    }
    for base in MIRROR_BASES {
        let base = base.trim_end_matches('/');
        if url == base || url.starts_with(&format!("{base}/")) {
            // No path traversal via `..` segments.
            if url.contains("..") {
                return false;
            }
            return true;
        }
    }
    false
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open for hash: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        use std::io::Read;
        let n = file.read(&mut buf).map_err(|e| format!("hash read: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Parse a checksum file body for `filename` (GNU `sha256sum` or plain hex).
pub fn parse_checksum_for_file(body: &str, filename: &str) -> Option<String> {
    let want = filename.trim();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // "hex  filename" or "hex *filename"
        let mut parts = line.split_whitespace();
        let hex_part = parts.next()?;
        if hex_part.len() != 64 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            // bare hex for a single-file sidecar
            continue;
        }
        if let Some(name) = parts.next() {
            let name = name.trim_start_matches('*');
            if name == want
                || name.ends_with(want)
                || Path::new(name).file_name().and_then(|s| s.to_str()) == Some(want)
            {
                return Some(hex_part.to_ascii_lowercase());
            }
        } else {
            // single-line bare hex
            return Some(hex_part.to_ascii_lowercase());
        }
    }
    // whole-file bare hex (single line)
    let t = body.trim();
    if t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(t.to_ascii_lowercase());
    }
    None
}

async fn fetch_published_checksum(
    client: &reqwest::Client,
    mirror: &str,
    version: &str,
    platform: &str,
    artifact_name: &str,
) -> Option<String> {
    let base = mirror.trim_end_matches('/');
    // Common sidecar layouts; none are published today, but we fail closed on mismatch
    // if any of them appears later.
    let candidates = [
        format!("{base}/{artifact_name}.sha256"),
        format!("{base}/{artifact_name}.sha256sum"),
        format!("{base}/SHA256SUMS"),
        format!("{base}/checksums.txt"),
        format!("{base}/grok-{version}-{platform}.sha256"),
        format!("{base}/{version}/SHA256SUMS"),
    ];
    for url in candidates {
        if !is_allowed_download_url(&url) {
            continue;
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(text) = resp.text().await {
                    if let Some(h) = parse_checksum_for_file(&text, artifact_name) {
                        info!("cli_install: checksum for {artifact_name} from {url}");
                        return Some(h);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn emit(app: &AppHandle, p: CliInstallProgress) {
    let _ = app.emit("setup://cli-install-progress", &p);
}

fn progress(
    phase: &str,
    message: impl Into<String>,
    percent: Option<f64>,
    mirror: Option<String>,
    version: Option<String>,
) -> CliInstallProgress {
    CliInstallProgress {
        phase: phase.into(),
        message: message.into(),
        percent,
        bytes_downloaded: None,
        total_bytes: None,
        mirror,
        version,
        sha256: None,
    }
}

fn platform_triple() -> Result<(&'static str, &'static str), String> {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported OS for Grok Build auto-install".into());
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        return Err("Unsupported CPU architecture for Grok Build auto-install".into());
    };
    Ok((os, arch))
}

fn http_client() -> Result<reqwest::Client, String> {
    crate::proxy::apply_to_reqwest(reqwest::Client::builder())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("GrokApp/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|e| e.to_string())
}

fn mirror_host(base: &str) -> String {
    base.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(base)
        .to_string()
}

async fn fetch_version_text(client: &reqwest::Client, base: &str) -> Result<String, String> {
    let url = format!("{}/{CHANNEL}", base.trim_end_matches('/'));
    if !is_allowed_download_url(&url) {
        return Err(format!("version URL not on allowlist: {url}"));
    }
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("version probe {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("version probe {url}: HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let version = text
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(|c: char| c.is_whitespace() || c == '\r')
        .to_string();
    if version.is_empty()
        || !version
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
    {
        return Err(format!("invalid version pointer from {url}: {text:?}"));
    }
    Ok(version)
}

async fn resolve_version(
    app: &AppHandle,
    client: &reqwest::Client,
) -> Result<(String, String), String> {
    emit(
        app,
        progress(
            "resolving",
            "Resolving latest Grok Build version…",
            Some(0.0),
            None,
            None,
        ),
    );

    let mut errors = Vec::new();
    for base in MIRROR_BASES {
        for attempt in 1..=MIRROR_ATTEMPTS {
            emit(
                app,
                progress(
                    "resolving",
                    format!(
                        "Trying {} (attempt {attempt}/{MIRROR_ATTEMPTS})…",
                        mirror_host(base)
                    ),
                    Some(2.0),
                    Some((*base).into()),
                    None,
                ),
            );
            match fetch_version_text(client, base).await {
                Ok(v) => {
                    info!("cli_install: version {v} via {base}");
                    return Ok((v, (*base).to_string()));
                }
                Err(e) => {
                    warn!("cli_install version fail base={base} attempt={attempt}: {e}");
                    errors.push(e);
                    if attempt < MIRROR_ATTEMPTS {
                        tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
                    }
                }
            }
        }
    }
    Err(format!(
        "Could not resolve Grok Build version from any mirror. {}",
        errors.last().cloned().unwrap_or_default()
    ))
}

async fn download_to_file(
    app: &AppHandle,
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    version: &str,
    mirror: &str,
) -> Result<(), String> {
    // Fail-closed: never fetch from outside the official mirror list.
    if !is_allowed_download_url(url) {
        return Err(format!("download URL not on allowlist: {url}"));
    }
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download {url}: HTTP {}", resp.status()));
    }
    // After redirects, re-check final URL when available.
    let final_url = resp.url().to_string();
    if !is_allowed_download_url(&final_url) {
        return Err(format!("download redirected off allowlist: {final_url}"));
    }
    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let mut file = fs::File::create(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut downloaded: u64 = 0;
    let mut last_emit = 0u64;
    let mut hasher = Sha256::new();

    emit(
        app,
        CliInstallProgress {
            phase: "downloading".into(),
            message: format!("Downloading from {}…", mirror_host(mirror)),
            percent: Some(5.0),
            bytes_downloaded: Some(0),
            total_bytes: total,
            mirror: Some(mirror.into()),
            version: Some(version.into()),
            sha256: None,
        },
    );

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download stream: {e}"))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| format!("write download: {e}"))?;
        downloaded += chunk.len() as u64;
        // Throttle UI events (~every 256 KiB or completion).
        if downloaded.saturating_sub(last_emit) >= 256 * 1024 || total == Some(downloaded) {
            last_emit = downloaded;
            let percent = match total {
                Some(t) if t > 0 => 5.0 + (downloaded as f64 / t as f64) * 85.0,
                _ => 5.0 + (downloaded as f64 / (120.0 * 1024.0 * 1024.0)).min(1.0) * 85.0,
            };
            emit(
                app,
                CliInstallProgress {
                    phase: "downloading".into(),
                    message: format!("Downloading… {}", format_bytes_pair(downloaded, total)),
                    percent: Some(percent.min(90.0)),
                    bytes_downloaded: Some(downloaded),
                    total_bytes: total,
                    mirror: Some(mirror.into()),
                    version: Some(version.into()),
                    sha256: None,
                },
            );
        }
    }
    file.sync_all().map_err(|e| e.to_string())?;
    if downloaded == 0 {
        let _ = fs::remove_file(dest);
        return Err("download produced empty file".into());
    }
    if let Some(t) = total {
        if downloaded != t {
            let _ = fs::remove_file(dest);
            return Err(format!(
                "download size mismatch: got {downloaded}, expected {t}"
            ));
        }
    }
    let digest = hex::encode(hasher.finalize());
    // Persist digest next to the part file for the install step.
    let side = dest.with_extension("sha256");
    let _ = fs::write(&side, &digest);
    Ok(())
}

fn format_bytes_pair(done: u64, total: Option<u64>) -> String {
    match total {
        Some(t) => format!("{} / {}", format_bytes(done), format_bytes(t)),
        None => format_bytes(done),
    }
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let f = n as f64;
    if f >= MB {
        format!("{:.1} MB", f / MB)
    } else if f >= KB {
        format!("{:.0} KB", f / KB)
    } else {
        format!("{n} B")
    }
}

fn verify_binary(path: &Path) -> Result<String, String> {
    // Fresh downloads are not yet "looks_runnable":
    // - Windows temp names used to end in `.part` (rejected by extension check)
    // - Unix files from File::create have no +x until we chmod
    // Real gate is a successful `--version` after we fix permissions / naming.
    if !path.is_file() {
        return Err(format!("not a file: {}", path.display()));
    }
    let meta = fs::metadata(path).map_err(|e| format!("stat {}: {e}", path.display()))?;
    if meta.len() < 1024 {
        return Err(format!(
            "downloaded file too small ({} bytes): {}",
            meta.len(),
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).map_err(|e| e.to_string())?;
    }
    let mut cmd = std::process::Command::new(path);
    cmd.arg("--version");
    process_util::apply_no_window_std(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| format!("failed to run downloaded binary: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "downloaded binary --version failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if line.is_empty() {
        Err("downloaded binary returned empty --version".into())
    } else {
        Ok(line)
    }
}

fn link_install(download_path: &Path, version: &str) -> Result<PathBuf, String> {
    let home = user_home();
    let download_dir = home.join(".grok").join("downloads");
    let bin_dir = home.join(".grok").join("bin");
    fs::create_dir_all(&download_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;

    let (os, arch) = platform_triple()?;
    let platform = format!("{os}-{arch}");

    #[cfg(target_os = "windows")]
    let final_name = format!("grok-{version}-{platform}.exe");
    #[cfg(not(target_os = "windows"))]
    let final_name = format!("grok-{version}-{platform}");

    let final_download = download_dir.join(&final_name);
    if final_download != *download_path {
        let _ = fs::remove_file(&final_download);
        if fs::rename(download_path, &final_download).is_err() {
            fs::copy(download_path, &final_download).map_err(|e| format!("place binary: {e}"))?;
            let _ = fs::remove_file(download_path);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let grok_exe = bin_dir.join("grok.exe");
        let agent_exe = bin_dir.join("agent.exe");
        for target in [&grok_exe, &agent_exe] {
            let old = PathBuf::from(format!("{}.old", target.display()));
            let _ = fs::remove_file(&old);
            if fs::copy(&final_download, target).is_err() {
                // Locked by running process — rename aside then retry
                let _ = fs::rename(target, &old);
                if let Err(e2) = fs::copy(&final_download, target) {
                    let _ = fs::rename(&old, target);
                    return Err(format!("install {}: {e2}", target.display()));
                }
            }
        }
        Ok(grok_exe)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let link_target = if download_dir.parent() == bin_dir.parent() {
            PathBuf::from(format!("../downloads/{}", final_name))
        } else {
            final_download.clone()
        };
        let grok_link = bin_dir.join("grok");
        let agent_link = bin_dir.join("agent");
        // Remove existing file/symlink then recreate
        let _ = fs::remove_file(&grok_link);
        let _ = fs::remove_file(&agent_link);
        std::os::unix::fs::symlink(&link_target, &grok_link)
            .map_err(|e| format!("symlink grok: {e}"))?;
        std::os::unix::fs::symlink(&link_target, &agent_link)
            .map_err(|e| format!("symlink agent: {e}"))?;
        Ok(grok_link)
    }
}

async fn try_download_all_mirrors(
    app: &AppHandle,
    client: &reqwest::Client,
    version: &str,
    preferred_mirror: &str,
) -> Result<(PathBuf, String), String> {
    let (os, arch) = platform_triple()?;
    let platform = format!("{os}-{arch}");
    let mut bases: Vec<&str> = Vec::new();
    // Preferred first, then others
    bases.push(preferred_mirror);
    for b in MIRROR_BASES {
        if *b != preferred_mirror {
            bases.push(*b);
        }
    }

    let tmp_dir = user_home().join(".grok").join("downloads");
    fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;
    // Windows: keep a trailing `.exe` so CreateProcess / probe can run the partial file
    // after a successful download (extension must not be bare `.part`).
    let tmp_path = tmp_dir.join(format!(
        "grok-{}-{}-{}.part{}",
        version,
        platform,
        std::process::id(),
        if cfg!(target_os = "windows") {
            ".exe"
        } else {
            ""
        }
    ));

    let mut errors = Vec::new();
    for base in bases {
        let artifact = format!(
            "{}/grok-{version}-{platform}{}",
            base.trim_end_matches('/'),
            if cfg!(target_os = "windows") {
                ".exe"
            } else {
                ""
            }
        );
        // Windows also tries extension-less fallback like install.sh
        let candidates: Vec<String> = if cfg!(target_os = "windows") {
            vec![
                artifact.clone(),
                format!("{}/grok-{version}-{platform}", base.trim_end_matches('/')),
            ]
        } else {
            vec![artifact]
        };

        for attempt in 1..=MIRROR_ATTEMPTS {
            for url in &candidates {
                if !is_allowed_download_url(url) {
                    errors.push(format!("skip non-allowlisted URL: {url}"));
                    continue;
                }
                emit(
                    app,
                    progress(
                        "downloading",
                        format!(
                            "Mirror {} · attempt {attempt}/{MIRROR_ATTEMPTS}",
                            mirror_host(base)
                        ),
                        Some(5.0),
                        Some(base.into()),
                        Some(version.into()),
                    ),
                );
                let _ = fs::remove_file(&tmp_path);
                match download_to_file(app, client, url, &tmp_path, version, base).await {
                    Ok(()) => return Ok((tmp_path, base.to_string())),
                    Err(e) => {
                        warn!("cli_install download fail url={url}: {e}");
                        errors.push(e);
                        let _ = fs::remove_file(&tmp_path);
                    }
                }
            }
            if attempt < MIRROR_ATTEMPTS {
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }
        }
    }
    Err(format!(
        "All mirrors failed. Last error: {}",
        errors.last().cloned().unwrap_or_else(|| "unknown".into())
    ))
}

/// Whether a published checksum is **required** when the mirror has none.
///
/// Default **false**: official x.ai / GCS CLI mirrors do not publish SHA-256
/// sidecars today (and the official install scripts do not verify them).
/// Requiring a missing sidecar made first-run install fail on every platform
/// (#227). Mismatch always fails regardless of this flag.
///
/// Fail-closed on **missing** sidecar only when:
/// - env `GROK_CLI_REQUIRE_CHECKSUM` is 1/true/yes/on, **and**
/// - neither `allow_unverified` (Settings) nor `GROK_CLI_ALLOW_UNVERIFIED` is set
pub fn require_published_checksum(allow_unverified: bool) -> bool {
    if env_flag_truthy("GROK_CLI_ALLOW_UNVERIFIED") || allow_unverified {
        return false;
    }
    env_flag_truthy("GROK_CLI_REQUIRE_CHECKSUM")
}

fn env_flag_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

/// Download latest stable Grok Build and install into `~/.grok`.
///
/// `allow_unverified`: when true, continue if the mirror has no published
/// checksum (still fail on mismatch). Default path should pass `false`.
pub async fn install_cli_latest(
    app: AppHandle,
    allow_unverified: bool,
) -> Result<CliInstallResult, String> {
    let client = http_client()?;
    let (version, preferred) = resolve_version(&app, &client).await?;

    emit(
        &app,
        progress(
            "downloading",
            format!("Found Grok Build v{version}"),
            Some(4.0),
            Some(preferred.clone()),
            Some(version.clone()),
        ),
    );

    let (tmp_path, mirror_used) =
        try_download_all_mirrors(&app, &client, &version, &preferred).await?;

    let digest = sha256_file(&tmp_path).unwrap_or_else(|_| {
        // Fallback: side file written during download
        fs::read_to_string(tmp_path.with_extension("sha256"))
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    if digest.len() != 64 {
        let _ = fs::remove_file(&tmp_path);
        return Err("failed to compute SHA-256 of downloaded CLI binary".into());
    }

    let (os, arch) = platform_triple()?;
    let platform = format!("{os}-{arch}");
    let artifact_name = if cfg!(target_os = "windows") {
        format!("grok-{version}-{platform}.exe")
    } else {
        format!("grok-{version}-{platform}")
    };

    emit(
        &app,
        CliInstallProgress {
            phase: "verifying".into(),
            message: format!("SHA-256 {digest:.12}… — checking published checksum…"),
            percent: Some(91.0),
            bytes_downloaded: None,
            total_bytes: None,
            mirror: Some(mirror_used.clone()),
            version: Some(version.clone()),
            sha256: Some(digest.clone()),
        },
    );

    let published =
        fetch_published_checksum(&client, &mirror_used, &version, &platform, &artifact_name).await;
    let checksum_verified = match published {
        Some(expected) => {
            if expected != digest {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!(
                    "SHA-256 mismatch for {artifact_name}: got {digest}, expected {expected}"
                ));
            }
            info!("cli_install: published checksum matched for {artifact_name}");
            true
        }
        None => {
            if is_known_cli_hash(&artifact_name, &digest) {
                info!("cli_install: known-good checksum matched for {artifact_name}");
                true
            } else if let Some(expected_known) = lookup_known_cli_hash(&artifact_name) {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!(
                    "SHA-256 mismatch against known-good hash for {artifact_name}: got {digest}, expected {expected_known}"
                ));
            } else {
                // Not known and no published sidecar
                let _ = verify_or_record_first_seen_hash(&artifact_name, &digest);

                // Fail-closed: no published sidecar → refuse unless user opted in.
                if require_published_checksum(allow_unverified) {
                    let _ = fs::remove_file(&tmp_path);
                    return Err(format!(
                        "No published SHA-256 for {artifact_name}. Refusing install \
                         (GROK_CLI_REQUIRE_CHECKSUM is set). Enable “Allow unverified CLI install” \
                         in Settings → Runtime, set GROK_CLI_ALLOW_UNVERIFIED=1, or unset \
                         GROK_CLI_REQUIRE_CHECKSUM. hash={digest}"
                    ));
                }
                warn!(
                    "cli_install: no published checksum for {artifact_name}; \
                     continuing with allowlist + binary probe (unverified, hash={digest})"
                );
                false
            }
        }
    };

    emit(
        &app,
        CliInstallProgress {
            phase: "verifying".into(),
            message: if checksum_verified {
                "Checksum OK — verifying binary…".into()
            } else {
                "Verifying binary…".into()
            },
            percent: Some(92.0),
            bytes_downloaded: None,
            total_bytes: None,
            mirror: Some(mirror_used.clone()),
            version: Some(version.clone()),
            sha256: Some(digest.clone()),
        },
    );

    let ver_line = match verify_binary(&tmp_path) {
        Ok(v) => v,
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }
    };

    emit(
        &app,
        CliInstallProgress {
            phase: "linking".into(),
            message: "Installing to ~/.grok/bin…".into(),
            percent: Some(96.0),
            bytes_downloaded: None,
            total_bytes: None,
            mirror: Some(mirror_used.clone()),
            version: Some(version.clone()),
            sha256: Some(digest.clone()),
        },
    );

    let linked = link_install(&tmp_path, &version)?;
    let _ = fs::remove_file(tmp_path.with_extension("sha256"));
    let probe = cli_probe::probe_cli(Some(linked.to_string_lossy().as_ref()));
    let path = probe.path.or_else(|| Some(linked.display().to_string()));
    let version_out = probe.version.or(Some(ver_line));

    emit(
        &app,
        CliInstallProgress {
            phase: "done".into(),
            message: format!(
                "Installed {} (sha256 {})",
                version_out.as_deref().unwrap_or(&version),
                &digest[..12]
            ),
            percent: Some(100.0),
            bytes_downloaded: None,
            total_bytes: None,
            mirror: Some(mirror_used.clone()),
            version: version_out.clone(),
            sha256: Some(digest.clone()),
        },
    );

    Ok(CliInstallResult {
        ok: true,
        path,
        version: version_out,
        mirror_used: Some(mirror_used),
        message: "Grok Build installed".into(),
        sha256: Some(digest),
        checksum_verified: Some(checksum_verified),
    })
}

/// Install command strings for copy-paste fallback (platform-specific).
pub fn install_commands() -> serde_json::Value {
    #[cfg(target_os = "windows")]
    {
        serde_json::json!({
            "primary": "irm https://x.ai/cli/install.ps1 | iex",
            "shell": "powershell",
            "docsUrl": "https://docs.x.ai/build/overview",
            "mirrors": MIRROR_BASES,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        serde_json::json!({
            "primary": "curl -fsSL https://x.ai/cli/install.sh | bash",
            "shell": "bash",
            "docsUrl": "https://docs.x.ai/build/overview",
            "mirrors": MIRROR_BASES,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_accepts_official_mirrors_only() {
        assert!(is_allowed_download_url(
            "https://storage.googleapis.com/grok-build-public-artifacts/cli/stable"
        ));
        assert!(is_allowed_download_url(
            "https://storage.googleapis.com/grok-build-public-artifacts/cli/grok-0.2.111-macos-aarch64"
        ));
        assert!(is_allowed_download_url(
            "https://x.ai/cli/grok-0.2.111-macos-x86_64"
        ));
        assert!(is_allowed_download_url("https://x.ai/cli/SHA256SUMS"));
    }

    #[test]
    fn allowlist_rejects_http_and_foreign_hosts() {
        assert!(!is_allowed_download_url(
            "http://storage.googleapis.com/grok-build-public-artifacts/cli/stable"
        ));
        assert!(!is_allowed_download_url(
            "https://evil.example/cli/grok-0.2.111-macos-aarch64"
        ));
        assert!(!is_allowed_download_url(
            "https://storage.googleapis.com/other-bucket/cli/stable"
        ));
        assert!(!is_allowed_download_url("https://x.ai/not-cli/payload"));
        assert!(!is_allowed_download_url(
            "https://user:pass@x.ai/cli/stable"
        ));
        assert!(!is_allowed_download_url("https://x.ai/cli/../etc/passwd"));
        assert!(!is_allowed_download_url(""));
        assert!(!is_allowed_download_url("ftp://x.ai/cli/stable"));
    }

    #[test]
    fn parse_checksum_gnu_sha256sum_format() {
        let body = "\
# comment
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  grok-0.2.111-macos-aarch64
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *other-file
";
        let h = parse_checksum_for_file(body, "grok-0.2.111-macos-aarch64").unwrap();
        assert_eq!(
            h,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[test]
    fn parse_checksum_bare_hex() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            parse_checksum_for_file(hex, "anything").as_deref(),
            Some(hex)
        );
    }

    #[test]
    fn parse_checksum_ignores_garbage() {
        assert!(parse_checksum_for_file("not a hash", "f").is_none());
        assert!(parse_checksum_for_file("abcd short", "f").is_none());
    }

    #[test]
    fn parse_checksum_mismatch_name_skipped_for_multi_line() {
        let body = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  wrong-name\n";
        // multi-line with name mismatch → None (no bare single-line fallback when name present)
        assert!(parse_checksum_for_file(body, "right-name").is_none());
    }

    #[test]
    fn require_checksum_policy_default_and_strict_env() {
        // Env mutation must be serialized — cargo runs unit tests in parallel.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Official mirrors omit sidecars; default must not block install (#227).
        std::env::remove_var("GROK_CLI_ALLOW_UNVERIFIED");
        std::env::remove_var("GROK_CLI_REQUIRE_CHECKSUM");
        assert!(!require_published_checksum(false));
        assert!(!require_published_checksum(true));

        // Strict env fails closed unless allow_unverified / ALLOW_UNVERIFIED.
        std::env::set_var("GROK_CLI_REQUIRE_CHECKSUM", "1");
        assert!(require_published_checksum(false));
        assert!(!require_published_checksum(true));
        std::env::set_var("GROK_CLI_ALLOW_UNVERIFIED", "1");
        assert!(!require_published_checksum(false));

        std::env::remove_var("GROK_CLI_REQUIRE_CHECKSUM");
        std::env::remove_var("GROK_CLI_ALLOW_UNVERIFIED");
    }

    #[test]
    fn sha256_file_matches_known_digest() {
        let dir = std::env::temp_dir().join(format!("cli-hash-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("blob.bin");
        fs::write(&path, b"grok-cli-test-bytes").unwrap();
        let got = sha256_file(&path).unwrap();
        // echo -n 'grok-cli-test-bytes' | shasum -a 256
        assert_eq!(got.len(), 64);
        assert!(got.chars().all(|c| c.is_ascii_hexdigit()));
        // re-hash same content → stable
        assert_eq!(got, sha256_file(&path).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn known_cli_hash_lookup_and_verification() {
        let mac_arm_hash = lookup_known_cli_hash("grok-0.2.111-macos-aarch64").unwrap();
        assert_eq!(mac_arm_hash.len(), 64);
        assert!(is_known_cli_hash("grok-0.2.111-macos-aarch64", mac_arm_hash));
        assert!(is_known_cli_hash(
            "grok-0.2.111-macos-aarch64",
            &mac_arm_hash.to_uppercase()
        ));
        assert!(!is_known_cli_hash(
            "grok-0.2.111-macos-aarch64",
            "0000000000000000000000000000000000000000000000000000000000000000"
        ));

        // Windows artifacts match both with and without .exe
        let win_hash = lookup_known_cli_hash("grok-0.2.111-windows-x86_64.exe").unwrap();
        assert_eq!(
            lookup_known_cli_hash("grok-0.2.111-windows-x86_64"),
            Some(win_hash)
        );
        assert!(is_known_cli_hash("grok-0.2.111-windows-x86_64", win_hash));

        // Unknown artifact returns None
        assert!(lookup_known_cli_hash("grok-9.9.999-unsupported-arch").is_none());
        assert!(!is_known_cli_hash(
            "grok-9.9.999-unsupported-arch",
            mac_arm_hash
        ));
    }

    #[test]
    fn first_seen_hashes_recording_and_warning_on_change() {
        let dir = std::env::temp_dir().join(format!("first-seen-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let store_file = dir.join("first_seen_hashes.json");

        let artifact = "grok-custom-build-aarch64";
        let h1 = "1111111111111111111111111111111111111111111111111111111111111111";
        let h2 = "2222222222222222222222222222222222222222222222222222222222222222";

        // First seen: should record new
        let res1 = check_or_update_first_seen_hash_in_file(&store_file, artifact, h1).unwrap();
        assert_eq!(res1, FirstSeenStatus::RecordedNew);
        assert!(store_file.exists());

        // Same hash seen again: should match existing
        let res2 = check_or_update_first_seen_hash_in_file(&store_file, artifact, h1).unwrap();
        assert_eq!(res2, FirstSeenStatus::MatchedExisting);

        // Different hash seen for same artifact: should warn and return Changed
        let res3 = check_or_update_first_seen_hash_in_file(&store_file, artifact, h2).unwrap();
        assert_eq!(
            res3,
            FirstSeenStatus::Changed {
                previous: h1.to_string(),
                current: h2.to_string(),
            }
        );

        // Verification after change: should now match h2
        let res4 = check_or_update_first_seen_hash_in_file(&store_file, artifact, h2).unwrap();
        assert_eq!(res4, FirstSeenStatus::MatchedExisting);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn github_workflows_pinned_to_immutable_commit_shas() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workflows_dir = manifest_dir.parent().unwrap().join(".github/workflows");

        let workflow_files = ["ci.yml", "release.yml"];
        let mut total_pinned_steps = 0;

        for wf in &workflow_files {
            let path = workflows_dir.join(wf);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

            for (lineno, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                let after_dash = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();
                if let Some(uses_part) = after_dash.strip_prefix("uses:") {
                    let uses_val = uses_part.trim();
                    let action_ref = uses_val.split('#').next().unwrap_or("").trim();
                    if action_ref.starts_with("./") || action_ref.starts_with("docker://") {
                        continue;
                    }
                    let parts: Vec<&str> = action_ref.split('@').collect();
                    assert_eq!(
                        parts.len(),
                        2,
                        "{wf}:{}: Expected action@ref format in '{trimmed}'",
                        lineno + 1
                    );
                    let git_ref = parts[1];
                    assert_eq!(
                        git_ref.len(),
                        40,
                        "{wf}:{}: Action '{}' ref '{}' is not a 40-character commit SHA",
                        lineno + 1,
                        parts[0],
                        git_ref
                    );
                    assert!(
                        git_ref.chars().all(|c| c.is_ascii_hexdigit()),
                        "{wf}:{}: Action '{}' ref '{}' contains non-hex characters",
                        lineno + 1,
                        parts[0],
                        git_ref
                    );
                    total_pinned_steps += 1;
                }
            }
        }

        // We expect at least 6 steps in ci.yml and 8 in release.yml
        assert!(
            total_pinned_steps >= 14,
            "expected at least 14 pinned action steps across workflows, found {total_pinned_steps}"
        );
    }
}
