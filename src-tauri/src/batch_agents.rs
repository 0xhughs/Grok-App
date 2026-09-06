//! Multi-project batch headless one-shot (soft-fail per project).
//!
//! Runs `grok -p <prompt>` with a project cwd and returns a short text summary.
//! Never panics on CLI missing / timeout / non-zero exit — returns structured soft-fail.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::cli_probe;
use crate::process_util;
use crate::proxy;
use crate::store;

/// Default soft timeout for a single batch headless turn (ms).
pub const DEFAULT_HEADLESS_TIMEOUT_MS: u64 = 120_000;
/// Hard clamp so a stuck CLI cannot hang the host forever.
pub const MAX_HEADLESS_TIMEOUT_MS: u64 = 300_000;
/// Soft cap on captured stdout characters returned to the FE.
pub const MAX_TEXT_CHARS: usize = 8_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchHeadlessResult {
    pub ok: bool,
    pub reason: Option<String>,
    pub text: Option<String>,
    pub duration_ms: u64,
    pub cli_path: Option<String>,
    pub cli_version: Option<String>,
}

fn soft_fail(
    reason: &str,
    cli_path: Option<String>,
    cli_version: Option<String>,
    text: Option<String>,
    duration_ms: u64,
) -> BatchHeadlessResult {
    BatchHeadlessResult {
        ok: false,
        reason: Some(reason.to_string()),
        text,
        duration_ms,
        cli_path,
        cli_version,
    }
}

fn clamp_timeout_ms(ms: Option<u64>) -> u64 {
    let v = ms.unwrap_or(DEFAULT_HEADLESS_TIMEOUT_MS);
    v.clamp(5_000, MAX_HEADLESS_TIMEOUT_MS)
}

fn truncate_text(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let clipped: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{clipped}…")
}

/// Build headless argv (without binary path). Pure for tests.
pub fn batch_headless_args(prompt: &str, parent_policy: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".into(),
        prompt.to_string(),
        "--no-subagents".into(),
        "--disallowed-tools".into(),
        "run_terminal_cmd,run_terminal_command,search_replace,write,Agent,spawn_subagent,bash,bash_tool".into(),
        "--max-turns".into(),
        "8".into(),
        "--output-format".into(),
        "plain".into(),
    ];
    let is_yolo = parent_policy.map_or(false, |pol| {
        matches!(
            crate::permission::PermissionPolicy::parse(pol),
            crate::permission::PermissionPolicy::AlwaysApprove
        )
    });
    if is_yolo {
        args.push("--always-approve".into());
    }
    args
}

enum ThreadWait<T> {
    Done(T),
    TimedOut,
    JoinErr,
}

fn wait_thread<T: Send + 'static>(
    handle: std::thread::JoinHandle<T>,
    timeout: Duration,
) -> ThreadWait<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if handle.is_finished() {
            return match handle.join() {
                Ok(v) => ThreadWait::Done(v),
                Err(_) => ThreadWait::JoinErr,
            };
        }
        if Instant::now() >= deadline {
            // Soft-timeout: worker may still hold a child; process dies with app.
            return ThreadWait::TimedOut;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

/// Run headless batch turn following parent session policy.
/// If no session is present, refuses with `no_session`.
pub fn batch_agents_headless(
    project_path: &str,
    prompt: &str,
    timeout_ms: Option<u64>,
    parent_policy: Option<&str>,
) -> BatchHeadlessResult {
    let started = Instant::now();
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return soft_fail(
            "empty_prompt",
            None,
            None,
            None,
            started.elapsed().as_millis() as u64,
        );
    }
    let cwd = project_path.trim();
    if cwd.is_empty() {
        return soft_fail(
            "empty_path",
            None,
            None,
            None,
            started.elapsed().as_millis() as u64,
        );
    }
    let cwd_path = PathBuf::from(cwd);
    if !cwd_path.is_dir() {
        return soft_fail(
            "path_missing",
            None,
            None,
            None,
            started.elapsed().as_millis() as u64,
        );
    }
    let Some(policy) = parent_policy.filter(|p| !p.trim().is_empty()) else {
        return soft_fail(
            "no_session",
            None,
            None,
            None,
            started.elapsed().as_millis() as u64,
        );
    };

    run_batch_headless_inner(prompt, &cwd_path, timeout_ms, started, Some(policy))
}

/// One-shot headless turn for a project cwd. Soft-fails; never panics.
/// Resolves parent session policy; if no session exists, refuses.
pub fn run_batch_headless(
    project_path: &str,
    prompt: &str,
    timeout_ms: Option<u64>,
) -> BatchHeadlessResult {
    let sessions = store::load_sessions_index();
    let first = sessions.first();
    let parent_policy = first.map(|s| {
        s.permission_policy
            .as_deref()
            .unwrap_or("ask")
    });
    batch_agents_headless(project_path, prompt, timeout_ms, parent_policy)
}

fn run_batch_headless_inner(
    prompt: &str,
    cwd_path: &std::path::Path,
    timeout_ms: Option<u64>,
    started: Instant,
    parent_policy: Option<&str>,
) -> BatchHeadlessResult {
    let settings = store::load_settings();
    let probe = cli_probe::probe_cli(settings.manual_cli_path.as_deref());
    if !probe.found {
        return soft_fail(
            "cli_missing",
            None,
            probe.version.clone(),
            None,
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
                started.elapsed().as_millis() as u64,
            );
        }
    };
    let cli_version = probe.version.clone();
    let timeout = Duration::from_millis(clamp_timeout_ms(timeout_ms));
    let args = batch_headless_args(prompt, parent_policy);

    let cli_path_clone = cli_path.clone();
    let cwd_clone = cwd_path.to_path_buf();
    let mode = settings.session_data_mode.clone();
    let handle = std::thread::spawn(move || {
        let mut cmd = Command::new(&cli_path_clone);
        cmd.args(&args)
            .current_dir(&cwd_clone)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        process_util::apply_cli_env_std(&mut cmd);
        let grok_home = crate::paths::resolve_agent_grok_home(&mode);
        let _ = std::fs::create_dir_all(&grok_home);
        cmd.env("GROK_HOME", &grok_home);
        if mode != "shared" {
            crate::providers::prepare_route_auth_for_agent();
        }
        proxy::apply_to_std_command(&mut cmd);
        cmd.output()
    });

    let joined = wait_thread(handle, timeout);
    let duration_ms = started.elapsed().as_millis() as u64;

    match joined {
        ThreadWait::TimedOut => {
            soft_fail("timeout", Some(cli_path), cli_version, None, duration_ms)
        }
        ThreadWait::JoinErr => soft_fail(
            "spawn_failed",
            Some(cli_path),
            cli_version,
            None,
            duration_ms,
        ),
        ThreadWait::Done(Err(e)) => {
            tracing::warn!(
                target: "batch_agents",
                error = %e,
                "batch headless output failed"
            );
            soft_fail(
                "spawn_failed",
                Some(cli_path),
                cli_version,
                None,
                duration_ms,
            )
        }
        ThreadWait::Done(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let text = truncate_text(&stdout, MAX_TEXT_CHARS);
            let status_ok = output.status.success();
            if !text.is_empty() {
                return BatchHeadlessResult {
                    ok: true,
                    reason: if status_ok {
                        None
                    } else {
                        Some("nonzero_exit".into())
                    },
                    text: Some(text),
                    duration_ms,
                    cli_path: Some(cli_path),
                    cli_version,
                };
            }
            let err_snip = truncate_text(&stderr, 200);
            soft_fail(
                if status_ok { "empty" } else { "spawn_failed" },
                Some(cli_path),
                cli_version,
                if err_snip.is_empty() {
                    None
                } else {
                    Some(err_snip)
                },
                duration_ms,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_restricted_in_ask_and_always_approve_in_yolo() {
        let ask_args = batch_headless_args("hello", Some("ask"));
        assert!(ask_args.contains(&"-p".into()));
        assert!(ask_args.contains(&"hello".into()));
        assert!(ask_args.contains(&"--output-format".into()));
        assert!(ask_args.contains(&"plain".into()));
        assert!(ask_args.contains(&"--no-subagents".into()));
        assert!(ask_args.contains(&"--disallowed-tools".into()));
        assert!(!ask_args.contains(&"--always-approve".into()));
        let dt_idx = ask_args.iter().position(|x| x == "--disallowed-tools").unwrap();
        let dt_val = &ask_args[dt_idx + 1];
        assert!(dt_val.contains("run_terminal_cmd"));
        assert!(dt_val.contains("write"));
        assert!(dt_val.contains("Agent"));

        let yolo_args = batch_headless_args("hello", Some("always_approve"));
        assert!(yolo_args.contains(&"--no-subagents".into()));
        assert!(yolo_args.contains(&"--disallowed-tools".into()));
        assert!(yolo_args.contains(&"--always-approve".into()));

        let default_args = batch_headless_args("hello", None);
        assert!(default_args.contains(&"--no-subagents".into()));
        assert!(default_args.contains(&"--disallowed-tools".into()));
        assert!(!default_args.contains(&"--always-approve".into()));
    }

    #[test]
    fn no_session_refuses() {
        let r = batch_agents_headless("/tmp", "hello", None, None);
        assert!(!r.ok);
        assert_eq!(r.reason.as_deref(), Some("no_session"));
    }

    #[test]
    fn empty_prompt_soft_fails() {
        let r = run_batch_headless("/tmp", "  ", None);
        assert!(!r.ok);
        assert_eq!(r.reason.as_deref(), Some("empty_prompt"));
    }

    #[test]
    fn empty_path_soft_fails() {
        let r = run_batch_headless("  ", "hi", None);
        assert!(!r.ok);
        assert_eq!(r.reason.as_deref(), Some("empty_path"));
    }

    #[test]
    fn missing_dir_soft_fails() {
        let r = run_batch_headless("/no/such/batch/path/xyz", "hi", Some(5_000));
        assert!(!r.ok);
        assert_eq!(r.reason.as_deref(), Some("path_missing"));
    }

    #[test]
    fn clamp_timeout_bounds() {
        assert_eq!(clamp_timeout_ms(None), DEFAULT_HEADLESS_TIMEOUT_MS);
        assert_eq!(clamp_timeout_ms(Some(100)), 5_000);
        assert_eq!(clamp_timeout_ms(Some(999_999)), MAX_HEADLESS_TIMEOUT_MS);
    }

    #[test]
    fn truncate_text_works() {
        assert_eq!(truncate_text("  ab  ", 10), "ab");
        let long = "x".repeat(20);
        let t = truncate_text(&long, 8);
        assert!(t.ends_with('…'));
        assert!(t.chars().count() <= 8);
    }
}
