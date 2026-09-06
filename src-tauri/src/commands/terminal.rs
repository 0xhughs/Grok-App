// Side Workbench: interactive PTY + path probe + embedded browser automation.

use std::path::Path;
use tauri::AppHandle;

use crate::pty_host;
use crate::side_browser_host::{self, SideBrowserInfo};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathExistsManyResult {
    pub existing: Vec<String>,
}

/// Soft path existence probe (optional diagnostics).
#[tauri::command]
pub async fn path_exists_many(paths: Vec<String>) -> Result<PathExistsManyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let existing: Vec<String> = paths
            .into_iter()
            .filter(|p| {
                let t = p.trim();
                !t.is_empty() && Path::new(t).exists()
            })
            .collect();
        PathExistsManyResult { existing }
    })
    .await
    .map_err(|e| format!("path_exists_many join: {e}"))
}

/// Validate project_path for terminal PTY spawn against registered, trusted projects.
pub fn validate_terminal_project_path(project_path: Option<&str>) -> Result<(), String> {
    if let Some(path) = project_path {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            let projects = crate::store::load_projects();
            if !crate::store::is_trusted_project_path(&projects, trimmed) {
                return Err("project_path must be a registered, trusted project".to_string());
            }
        }
    }
    Ok(())
}

/// Spawn interactive login shell PTY (`$SHELL -l -i`). Streams on `terminal://data`.
/// When `ssh_alias` is set, the PTY runs `ssh -tt` on that host instead.
#[tauri::command]
pub async fn terminal_pty_spawn(
    app: AppHandle,
    session_id: Option<String>,
    project_path: Option<String>,
    ssh_alias: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<pty_host::PtySpawnResult, String> {
    validate_terminal_project_path(project_path.as_deref())?;
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);
    tauri::async_runtime::spawn_blocking(move || {
        pty_host::spawn(app, session_id, project_path, ssh_alias, cols, rows)
    })
    .await
    .map_err(|e| format!("terminal_pty_spawn join: {e}"))?
}

/// Write UTF-8 input to a PTY session (keystrokes from xterm).
#[tauri::command]
pub async fn terminal_pty_write(session_id: String, data: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pty_host::write_bytes(&session_id, &data))
        .await
        .map_err(|e| format!("terminal_pty_write join: {e}"))?
}

/// Resize PTY when the terminal view changes.
#[tauri::command]
pub async fn terminal_pty_resize(session_id: String, cols: u16, rows: u16) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pty_host::resize(&session_id, cols, rows))
        .await
        .map_err(|e| format!("terminal_pty_resize join: {e}"))?
}

/// Tear down a PTY session.
#[tauri::command]
pub async fn terminal_pty_kill(session_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pty_host::kill(&session_id))
        .await
        .map_err(|e| format!("terminal_pty_kill join: {e}"))?
}

// ── Embedded side browser automation (in-app Webview only) ──────────────

/// Create/replace a side-browser child webview with download save-dialog wiring.
/// Prefer this over frontend `new Webview()` so WKWebView downloads can prompt.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn side_browser_create(
    app: AppHandle,
    label: String,
    url: String,
    window_label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    // `Window::add_child` dispatches work to the platform event loop and waits
    // for completion. Running it inside a synchronous IPC command can occupy
    // that same loop on Windows, leaving WebView2 half-created (renderer starts,
    // but the command never returns). Match Tauri's own async create_webview
    // command and keep the blocking wait off the UI/invoke thread.
    tauri::async_runtime::spawn_blocking(move || {
        side_browser_host::create(&app, label, url, window_label, x, y, width, height)
    })
    .await
    .map_err(|e| format!("side_browser_create join: {e}"))?
}

#[tauri::command]
pub fn side_browser_close(app: AppHandle, label: String) -> Result<(), String> {
    side_browser_host::close(&app, label)
}

#[tauri::command]
pub fn side_browser_list(app: AppHandle) -> Result<Vec<SideBrowserInfo>, String> {
    side_browser_host::list(&app)
}

#[tauri::command]
pub fn side_browser_navigate(app: AppHandle, label: String, url: String) -> Result<(), String> {
    side_browser_host::navigate(&app, label, url)
}

#[tauri::command]
pub fn side_browser_reload(app: AppHandle, label: String) -> Result<(), String> {
    side_browser_host::reload(&app, label)
}

#[tauri::command]
pub fn side_browser_url(app: AppHandle, label: String) -> Result<String, String> {
    side_browser_host::current_url(&app, label)
}

/// Eval waits on the webview callback (up to 15s). Keep that wait off the
/// UI/invoke thread — a sync command here freezes the whole app when the
/// child document is mid-navigation (WK/WebView2 will not answer).
#[tauri::command]
pub async fn side_browser_eval(
    app: AppHandle,
    label: String,
    script: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || side_browser_host::eval(&app, label, script))
        .await
        .map_err(|e| format!("side_browser_eval join: {e}"))?
}

#[tauri::command]
pub async fn side_browser_snapshot(app: AppHandle, label: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || side_browser_host::snapshot(&app, label))
        .await
        .map_err(|e| format!("side_browser_snapshot join: {e}"))?
}

/// Force-inject blob download polyfill into a side-browser webview (idempotent).
#[tauri::command]
pub fn side_browser_install_download_hook(app: AppHandle, label: String) -> Result<(), String> {
    crate::side_browser_blob::install_hook(&app, label)
}

#[cfg(test)]
mod terminal_tests {
    use super::*;

    #[test]
    fn terminal_pty_spawn_rejects_untrusted_project_path() {
        // 1. None or empty passes
        assert!(validate_terminal_project_path(None).is_ok());
        assert!(validate_terminal_project_path(Some("")).is_ok());
        assert!(validate_terminal_project_path(Some("   ")).is_ok());

        // 2. Unregistered path rejected
        let err_unregistered = validate_terminal_project_path(Some("/unregistered/arbitrary/path"));
        assert!(err_unregistered.is_err());
        assert_eq!(
            err_unregistered.unwrap_err(),
            "project_path must be a registered, trusted project"
        );

        // 3. Registered untrusted rejected
        let mut list = store::load_projects();
        let untrusted_path = "/tmp/test-untrusted-terminal-cwd";
        list.push(store::Project {
            id: "test-untrusted-terminal".into(),
            name: "untrusted".into(),
            path: untrusted_path.into(),
            trusted: false,
            last_opened_at: chrono::Utc::now(),
            path_ok: true,
            pinned: false,
            system: false,
            model_id: None,
            effort: None,
            mode: None,
            permission_policy: None,
            sandbox_profile: None,
            color: None,
            ssh_alias: None,
        });
        let _ = store::save_projects(&list);

        let err = validate_terminal_project_path(Some(untrusted_path)).unwrap_err();
        assert_eq!(err, "project_path must be a registered, trusted project");

        // 4. Registered trusted project passes
        let trusted_path = "/tmp/test-trusted-terminal-cwd";
        list.push(store::Project {
            id: "test-trusted-terminal".into(),
            name: "trusted".into(),
            path: trusted_path.into(),
            trusted: true,
            last_opened_at: chrono::Utc::now(),
            path_ok: true,
            pinned: false,
            system: false,
            model_id: None,
            effort: None,
            mode: None,
            permission_policy: None,
            sandbox_profile: None,
            color: None,
            ssh_alias: None,
        });
        let _ = store::save_projects(&list);

        assert!(validate_terminal_project_path(Some(trusted_path)).is_ok());

        // Clean up
        let mut clean = store::load_projects();
        clean.retain(|p| p.id != "test-untrusted-terminal" && p.id != "test-trusted-terminal");
        let _ = store::save_projects(&clean);
    }
}
