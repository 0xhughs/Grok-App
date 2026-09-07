//! Headless probe for Grok Build `--output-format streaming-messages-json`
//! (CLI 0.2.117+).
//!
//! Spawns a short restricted Ask-mode turn, writes NDJSON to a temp file, and
//! returns the body (size-capped) for Settings diagnostics.
//! Soft-fails when the CLI is missing or older than 0.2.117 (no spawn).
//! Never logs stdout body or secrets — only reason / line count / duration.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::cli_probe;
use crate::process_util;
use crate::proxy;
use crate::store;

/// First CLI that accepts `--output-format streaming-messages-json`.
pub const MIN_CLI_VERSION: (u64, u64, u64) = (0, 2, 117);

/// Format flag value.
pub const OUTPUT_FORMAT: &str = "streaming-messages-json";

/// Deterministic probe prompt (matches frontend constant).
pub const PROBE_PROMPT: &str = "Reply with exactly: SMJ_PROBE_OK";

/// Cap raw NDJSON returned to the UI (bytes).
pub const MAX_RAW_BYTES: usize = 512 * 1024;

/// Headless probe timeout.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(90);

/// Official_aux leftover-child denials.
const STREAMING_DISALLOWED_TOOLS: &str = "run_terminal_cmd,run_terminal_command,search_replace,write,Agent,spawn_subagent,bash,bash_tool";

/// Build headless argv for the streaming-messages-json probe (without binary).
pub fn streaming_probe_args(include_partial: bool, is_yolo: bool) -> Vec<String> {
    let mut args = vec![
        "-p".into(),
        PROBE_PROMPT.to_string(),
        "--no-subagents".into(),
        "--disallowed-tools".into(),
        STREAMING_DISALLOWED_TOOLS.into(),
        "--max-turns".into(),
        "1".into(),
        "--effort".into(),
        "low".into(),
        "--output-format".into(),
        OUTPUT_FORMAT.into(),
    ];
    if include_partial {
        args.push("--include-partial-messages".into());
    }
    if is_yolo {
        args.push("--always-approve".into());
    }
    args
}

fn min_version_str() -> String {
    let (a, b, c) = MIN_CLI_VERSION;
    format!("{a}.{b}.{c}")
}

/// Result of a soft-fail-aware headless probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingMessagesJsonProbeResult {
    /// True when NDJSON was captured (may still be empty of messages).
    pub ok: bool,
    /// `ok` | `cli_missing` | `cli_too_old` | `version_unknown` | `spawn_failed` | `empty` | `timeout` | `read_failed`
    pub reason: String,
    pub cli_path: Option<String>,
    pub cli_version: Option<String>,
    /// `Some(false)` when known older than 0.2.117; `None` when unparsed.
    pub version_supported: Option<bool>,
    pub min_version: String,
    /// Absolute path of the temp NDJSON file when written.
    pub output_path: Option<String>,
    /// Capped raw NDJSON (never logged by Host).
    pub raw_ndjson: Option<String>,
    pub line_count: u32,
    pub duration_ms: u64,
    pub include_partial: bool,
    pub truncated: bool,
}

fn version_supported(raw: &str) -> Option<bool> {
    let token = cli_probe::extract_version_token(raw)?;
    let parsed = crate::app_update::parse_semver(&token)?;
    Some(parsed >= MIN_CLI_VERSION)
}

fn count_nonempty_lines(s: &str) -> u32 {
    s.lines().filter(|l| !l.trim().is_empty()).count() as u32
}

fn soft_fail(
    reason: &str,
    cli_path: Option<String>,
    cli_version: Option<String>,
    version_supported: Option<bool>,
    include_partial: bool,
    duration_ms: u64,
) -> StreamingMessagesJsonProbeResult {
    StreamingMessagesJsonProbeResult {
        ok: false,
        reason: reason.into(),
        cli_path,
        cli_version,
        version_supported,
        min_version: min_version_str(),
        output_path: None,
        raw_ndjson: None,
        line_count: 0,
        duration_ms,
        include_partial,
        truncated: false,
    }
}

/// Run a short headless probe with `--output-format streaming-messages-json`.
///
/// Soft-fail contract:
/// - CLI missing → `cli_missing`
/// - Known version &lt; 0.2.117 → `cli_too_old` (no spawn)
/// - Version unparsed → still attempt spawn (unknown flag risk is acceptable for
///   an explicit diagnostics probe; older CLIs surface clap errors as `spawn_failed`)
pub fn probe_streaming_messages_json(include_partial: bool) -> StreamingMessagesJsonProbeResult {
    let started = Instant::now();
    let settings = store::load_settings();
    let probe = cli_probe::probe_cli(settings.manual_cli_path.as_deref());

    if !probe.found {
        return soft_fail(
            "cli_missing",
            None,
            None,
            None,
            include_partial,
            started.elapsed().as_millis() as u64,
        );
    }

    let cli_path = match probe.path.clone() {
        Some(p) => p,
        None => {
            return soft_fail(
                "cli_missing",
                None,
                probe.version.clone(),
                None,
                include_partial,
                started.elapsed().as_millis() as u64,
            );
        }
    };

    let cli_version = probe.version.clone();
    let supported = cli_version
        .as_deref()
        .and_then(version_supported)
        .or_else(|| probe.version.as_deref().and_then(version_supported));

    if supported == Some(false) {
        tracing::info!(
            target: "streaming_messages_json",
            version = cli_version.as_deref().unwrap_or("?"),
            min = %min_version_str(),
            "streaming-messages-json probe soft-fail: cli_too_old"
        );
        return soft_fail(
            "cli_too_old",
            Some(cli_path),
            cli_version,
            Some(false),
            include_partial,
            started.elapsed().as_millis() as u64,
        );
    }

    let tmp_dir = std::env::temp_dir().join("grok-app-smj-probe");
    if let Err(e) = fs::create_dir_all(&tmp_dir) {
        tracing::warn!(
            target: "streaming_messages_json",
            error = %e,
            "failed to create probe temp dir"
        );
        return soft_fail(
            "spawn_failed",
            Some(cli_path),
            cli_version,
            supported,
            include_partial,
            started.elapsed().as_millis() as u64,
        );
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out_path: PathBuf = tmp_dir.join(format!("probe-{stamp}.ndjson"));

    let mut cmd = Command::new(&cli_path);
    // Probe has no session → Ask (never YOLO).
    cmd.args(streaming_probe_args(include_partial, false));
    // Keep cwd neutral; do not inherit project secrets paths into logs.
    cmd.current_dir(std::env::temp_dir());
    process_util::apply_no_window_std(&mut cmd);
    if let Some(path_env) = process_util::enriched_path_env() {
        cmd.env("PATH", path_env);
    }
    proxy::apply_to_std_command(&mut cmd);

    let spawn_started = Instant::now();
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(
                target: "streaming_messages_json",
                error = %e,
                "streaming-messages-json probe spawn failed"
            );
            return soft_fail(
                "spawn_failed",
                Some(cli_path),
                cli_version,
                supported,
                include_partial,
                started.elapsed().as_millis() as u64,
            );
        }
    };

    let elapsed = spawn_started.elapsed();
    if elapsed > PROBE_TIMEOUT {
        // Process already finished; flag slow runs for UI honesty.
        tracing::info!(
            target: "streaming_messages_json",
            ms = elapsed.as_millis() as u64,
            "streaming-messages-json probe exceeded soft timeout"
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // stderr may contain clap errors on old CLIs — never log body (could hold paths).
    let stderr_empty = output.stderr.is_empty();
    let success = output.status.success();

    if !success && stdout.trim().is_empty() {
        tracing::info!(
            target: "streaming_messages_json",
            success,
            stderr_empty,
            "streaming-messages-json probe empty after non-zero exit"
        );
        // Older CLI rejecting the flag → treat as soft fail.
        let reason = "spawn_failed";
        return soft_fail(
            reason,
            Some(cli_path),
            cli_version,
            supported,
            include_partial,
            started.elapsed().as_millis() as u64,
        );
    }

    if stdout.trim().is_empty() {
        return soft_fail(
            "empty",
            Some(cli_path),
            cli_version,
            supported.or(Some(true)),
            include_partial,
            started.elapsed().as_millis() as u64,
        );
    }

    let (raw, truncated) = if stdout.len() > MAX_RAW_BYTES {
        (stdout[..MAX_RAW_BYTES].to_string(), true)
    } else {
        (stdout, false)
    };

    if let Err(e) = fs::write(&out_path, raw.as_bytes()) {
        tracing::warn!(
            target: "streaming_messages_json",
            error = %e,
            "failed to write probe temp file"
        );
        // Still return body to UI even if disk write failed.
        let line_count = count_nonempty_lines(&raw);
        return StreamingMessagesJsonProbeResult {
            ok: true,
            reason: "ok".into(),
            cli_path: Some(cli_path),
            cli_version,
            version_supported: supported.or(Some(true)),
            min_version: min_version_str(),
            output_path: None,
            raw_ndjson: Some(raw),
            line_count,
            duration_ms: started.elapsed().as_millis() as u64,
            include_partial,
            truncated,
        };
    }

    let line_count = count_nonempty_lines(&raw);
    tracing::info!(
        target: "streaming_messages_json",
        line_count,
        duration_ms = started.elapsed().as_millis() as u64,
        truncated,
        include_partial,
        "streaming-messages-json probe ok"
    );

    StreamingMessagesJsonProbeResult {
        ok: true,
        reason: "ok".into(),
        cli_path: Some(cli_path),
        cli_version,
        version_supported: supported.or(Some(true)),
        min_version: min_version_str(),
        output_path: Some(out_path.to_string_lossy().to_string()),
        raw_ndjson: Some(raw),
        line_count,
        duration_ms: started.elapsed().as_millis() as u64,
        include_partial,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_version_is_0_2_117() {
        assert_eq!(MIN_CLI_VERSION, (0, 2, 117));
        assert_eq!(OUTPUT_FORMAT, "streaming-messages-json");
        assert!(PROBE_PROMPT.contains("SMJ_PROBE_OK"));
    }

    #[test]
    fn version_supported_gates() {
        assert_eq!(version_supported("grok 0.2.116"), Some(false));
        assert_eq!(version_supported("0.2.117"), Some(true));
        assert_eq!(version_supported("grok 0.3.0"), Some(true));
        assert_eq!(version_supported("not-a-version"), None);
    }

    #[test]
    fn count_nonempty_lines_skips_blanks() {
        assert_eq!(count_nonempty_lines("a\n\nb\n"), 2);
        assert_eq!(count_nonempty_lines(""), 0);
    }

    #[test]
    fn streaming_probe_args_restricted_and_conditional_always_approve() {
        let ask_args = streaming_probe_args(false, false);
        assert!(ask_args.contains(&"-p".into()));
        assert!(ask_args.contains(&PROBE_PROMPT.to_string()));
        assert!(ask_args.contains(&"--max-turns".into()));
        assert!(ask_args.contains(&"1".into()));
        assert!(ask_args.contains(&"--effort".into()));
        assert!(ask_args.contains(&"low".into()));
        assert!(ask_args.contains(&"--output-format".into()));
        assert!(ask_args.contains(&OUTPUT_FORMAT.to_string()));
        assert!(!ask_args.contains(&"--include-partial-messages".into()));
        assert!(ask_args.contains(&"--no-subagents".into()));
        assert!(ask_args.contains(&"--disallowed-tools".into()));
        assert!(!ask_args.contains(&"--always-approve".into()));
        let dt_idx = ask_args
            .iter()
            .position(|x| x == "--disallowed-tools")
            .unwrap();
        let dt_val = &ask_args[dt_idx + 1];
        for tok in [
            "run_terminal_cmd",
            "run_terminal_command",
            "search_replace",
            "write",
            "Agent",
            "spawn_subagent",
            "bash",
            "bash_tool",
        ] {
            assert!(dt_val.contains(tok), "missing {tok}");
        }

        let yolo_partial = streaming_probe_args(true, true);
        assert!(yolo_partial.contains(&"--include-partial-messages".into()));
        assert!(yolo_partial.contains(&"--no-subagents".into()));
        assert!(yolo_partial.contains(&"--disallowed-tools".into()));
        assert!(yolo_partial.contains(&"--always-approve".into()));
        let yolo_idx = yolo_partial
            .iter()
            .position(|x| x == "--disallowed-tools")
            .unwrap();
        let yolo_dt = &yolo_partial[yolo_idx + 1];
        for tok in [
            "run_terminal_cmd",
            "run_terminal_command",
            "search_replace",
            "write",
            "Agent",
            "spawn_subagent",
            "bash",
            "bash_tool",
        ] {
            assert!(yolo_dt.contains(tok), "missing {tok}");
        }
    }
}
