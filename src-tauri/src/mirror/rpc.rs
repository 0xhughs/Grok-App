//! Mirror WebSocket RPC method dispatch (DESIGN §7.2).

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::AppHandle;

use super::MirrorHost;
use crate::session_manager::SessionManager;
use crate::store;

/// Protocol version advertised in `mirror://hello`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Read-only RPC methods permitted when the host is in read-only mode.
pub const READ_METHODS: &[&str] = &[
    "projects.list",
    "sessions.list",
    "session.messages",
    "session.getState",
    "account.status",
    "settings.get",
    "models.list",
    "composer.prefsResolve",
    "voice.status",
];

/// Write RPC methods blocked when the host is in read-only mode.
/// Keep in sync with `src/lib/mirrorWriteSurface.ts` (UI category list).
#[allow(dead_code)]
pub const WRITE_METHODS: &[&str] = &[
    "session.send",
    "session.stop",
    "session.create",
    "session.rename",
    "session.autoTitle",
    "session.connect",
    "session.resolvePermission",
    "session.resolvePlan",
    "session.resolveAskUser",
    "voice.transcribe",
];

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: &'static str,
    pub message: String,
}

impl RpcError {
    pub fn unsupported(method: &str) -> Self {
        Self {
            code: "UNSUPPORTED",
            message: format!("unsupported method: {method}"),
        }
    }

    pub fn bad_params(msg: impl Into<String>) -> Self {
        Self {
            code: "BAD_PARAMS",
            message: msg.into(),
        }
    }

    pub fn host(msg: impl Into<String>) -> Self {
        Self {
            code: "HOST_ERROR",
            message: msg.into(),
        }
    }

    pub fn no_ctx() -> Self {
        Self {
            code: "NOT_READY",
            message: "mirror host context not attached (app/session manager)".into(),
        }
    }
}

/// Dispatch one RPC method. Unknown methods → `UNSUPPORTED` (WS stays open).
pub async fn dispatch(
    method: &str,
    params: Value,
    host: &MirrorHost,
    app: Option<&AppHandle>,
    mgr: Option<&Arc<SessionManager>>,
) -> Result<Value, RpcError> {
    // Read-only sessions can observe but not drive the agent.
    // Explicit allowlist of read methods: reject everything else when read-only.
    if host.is_read_only() && !READ_METHODS.contains(&method) {
        return Err(RpcError::unsupported("mirror is in read-only mode"));
    }

    match method {
        // ── Read path (Slice 3) ───────────────────────────────────────────
        "projects.list" => {
            let list = store::load_projects();
            Ok(serde_json::to_value(list).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "sessions.list" => {
            let list = store::load_sessions_index();
            Ok(serde_json::to_value(list).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.messages" => {
            let id = param_string(&params, &["sessionId", "session_id", "id"])
                .ok_or_else(|| RpcError::bad_params("sessionId required"))?;
            // Same as desktop `session_messages`: recover missing assistant bodies
            // and backfill user attachment cards from agent chat_history.
            let _ = crate::cli_sessions::try_reconcile_linked_session(&id);
            let msgs = store::load_messages(&id);
            Ok(serde_json::to_value(msgs).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.getState" => {
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?;
            let snap = mgr.snapshot();
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "account.status" => {
            // Light: skip heavy billing refresh unless client asks.
            let refresh = params
                .get("refreshBilling")
                .or_else(|| params.get("refresh_billing"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let include_usage = params
                .get("includeLocalUsage")
                .or_else(|| params.get("include_local_usage"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let status = crate::account::account_status_opts(None, refresh, include_usage).await;
            Ok(serde_json::to_value(status).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "settings.get" => {
            let s = store::load_settings();
            // Subset safe for phone display (no secret paths beyond what desktop already exposes).
            Ok(json!({
                "theme": s.theme,
                "locale": s.locale,
                "sessionDataMode": s.session_data_mode,
                "permissionPolicy": s.permission_policy,
                "modelId": s.model_id,
                "effort": s.effort,
                "mode": s.mode,
                "composerPrefsScope": s.composer_prefs_scope,
                "maxConcurrentAgents": s.max_concurrent_agents,
                "agentIdleMinutes": s.agent_idle_minutes,
                "streamStallSeconds": s.stream_stall_seconds,
                "onboardingDone": true,
                "setupSkipped": true,
                "setupWizardCompleted": true,
                "authSetupDeferred": true,
                "defaultOpenTarget": s.default_open_target,
                "manualCliPath": null,
                "acpServerAddr": s.acp_server_addr,
            }))
        }
        "models.list" => {
            let models = crate::models_catalog::list_available_models();
            Ok(serde_json::to_value(models).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "composer.prefsResolve" => {
            let project_id = param_string(&params, &["projectId", "project_id"]);
            let session_id = param_string(&params, &["sessionId", "session_id"]);
            let prefs = store::resolve_composer_prefs(project_id.as_deref(), session_id.as_deref());
            Ok(serde_json::to_value(prefs).map_err(|e| RpcError::host(e.to_string()))?)
        }

        // ── Composer voice dictation (phone STT) ─────────────────────────────
        // Minimal disclosure: `voice.status` returns only a boolean availability
        // (+ opaque reason/authSource class), never any secret material.
        "voice.status" => {
            let status = crate::voice_stt::voice_status();
            Ok(serde_json::to_value(status).map_err(|e| RpcError::host(e.to_string()))?)
        }
        // Stateless STT: decode base64 audio on the host and POST to xAI STT.
        // The same call the desktop `voice_transcribe` command uses, unchanged.
        "voice.transcribe" => {
            // audioBase64 is required and must be a raw base64 string (no data: URL).
            let audio_base64 = params
                .get("audioBase64")
                .or_else(|| params.get("audio_base64"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| RpcError::bad_params("audioBase64 required"))?;
            // Hygiene cap far above real dictation size (~24 MB of base64 text,
            // i.e. ~18 MB of audio). Rejects abusive frames before we decode.
            // The mirror WS frame cap (axum 64 MiB default) is the outer bound.
            const MAX_AUDIO_BASE64_BYTES: usize = 24 * 1024 * 1024;
            if audio_base64.len() > MAX_AUDIO_BASE64_BYTES {
                return Err(RpcError::bad_params(format!(
                    "audioBase64 too large: {} bytes (max {})",
                    audio_base64.len(),
                    MAX_AUDIO_BASE64_BYTES
                )));
            }
            let filename = param_string(&params, &["filename"]);
            let mime = param_string(&params, &["mime"]);
            let locale = param_string(&params, &["locale"]);
            let result = crate::voice_stt::voice_transcribe(
                audio_base64.to_string(),
                filename,
                mime,
                locale,
            )
            .await;
            Ok(serde_json::to_value(result).map_err(|e| RpcError::host(e.to_string()))?)
        }

        // ── Focus / connect (Slice 4) ─────────────────────────────────────
        "session.connect" => {
            let project_path = param_string(&params, &["projectPath", "project_path"]);
            if let Some(ref path) = project_path {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    let projects = store::load_projects();
                    if !store::is_trusted_project_path(&projects, trimmed) {
                        return Err(RpcError::bad_params(
                            "projectPath must be a registered, trusted project",
                        ));
                    }
                }
            }
            let app = app.ok_or_else(RpcError::no_ctx)?.clone();
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?.clone();
            let session_id = param_string(&params, &["sessionId", "session_id"]);
            let mode = param_string(&params, &["mode"]);
            let snap = mgr
                .connect(app, project_path, session_id, mode, None)
                .await
                .map_err(RpcError::host)?;
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.create" => {
            let project_id = param_string(&params, &["projectId", "project_id"]);
            let title = param_string(&params, &["title"]);
            let scheduled = params
                .get("scheduled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let meta =
                store::create_session(project_id, title, scheduled).map_err(RpcError::host)?;
            // Desktop + every other mirror client must see the new row without a
            // manual refresh (the index was mutated behind their back).
            notify_sessions_changed(app, "create", &meta.id);
            Ok(serde_json::to_value(meta).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.rename" => {
            let id = param_string(&params, &["id", "sessionId", "session_id"])
                .ok_or_else(|| RpcError::bad_params("id required"))?;
            let title = param_string(&params, &["title"])
                .ok_or_else(|| RpcError::bad_params("title required"))?;
            let meta = store::rename_session(&id, &title).map_err(RpcError::host)?;
            // Same as `commands::session_rename`: keep live meta aligned so a
            // mid-stream session://state does not revive the old title.
            if let (Some(app), Some(mgr)) = (app, mgr) {
                let _ = mgr.apply_title(app, &meta.id, &meta.title);
            }
            notify_sessions_changed(app, "rename", &meta.id);
            Ok(serde_json::to_value(meta).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.autoTitle" => {
            let id = param_string(&params, &["id", "sessionId", "session_id"])
                .ok_or_else(|| RpcError::bad_params("id required"))?;
            let first_message = param_string(&params, &["firstMessage", "first_message"])
                .ok_or_else(|| RpcError::bad_params("firstMessage required"))?;
            let meta = crate::session_title::auto_title_session_fast(&id, &first_message)
                .map_err(RpcError::host)?;
            if let (Some(app), Some(mgr)) = (app, mgr) {
                let _ = mgr.apply_title(app, &meta.id, &meta.title);
                crate::session_title::refine_title_in_background(
                    app.clone(),
                    Arc::clone(mgr),
                    id,
                    first_message,
                );
            }
            notify_sessions_changed(app, "autoTitle", &meta.id);
            Ok(serde_json::to_value(meta).map_err(|e| RpcError::host(e.to_string()))?)
        }

        // ── Write path (Slice 5 / AC5) ────────────────────────────────────
        "session.send" => {
            let text = param_string(&params, &["text"])
                .ok_or_else(|| RpcError::bad_params("text required"))?;
            let display_text = param_string(&params, &["displayText", "display_text"]);
            let attachments = param_attachments(&params);
            let target = param_string(&params, &["sessionId", "session_id"]);

            let focused = mgr.and_then(|m| m.snapshot().session_id);
            let effective_sid = target.as_deref().or(focused.as_deref());

            let Some(sid) = effective_sid else {
                if mgr.is_none() && app.is_none() && target.is_none() {
                    return Err(RpcError::no_ctx());
                }
                return Err(RpcError::host(
                    "no active session — call session.connect or pass sessionId",
                ));
            };

            // R2: When mirror.allow_remote_yolo is false, refuse turns directed to
            // sessions whose effective permission policy is relaxed.
            if !host.allow_remote_yolo() {
                let prefs = store::resolve_composer_prefs(None, Some(sid));
                let policy = crate::permission::PermissionPolicy::parse(&prefs.permission_policy);
                if matches!(
                    policy,
                    crate::permission::PermissionPolicy::AlwaysApprove
                        | crate::permission::PermissionPolicy::DontAsk
                        | crate::permission::PermissionPolicy::Auto
                        | crate::permission::PermissionPolicy::AcceptEdits
                ) {
                    return Err(RpcError::host(format!(
                        "refusing remote send: session effective permission policy is '{}' and mirror remote YOLO is disabled",
                        policy.as_str()
                    )));
                }
            }

            let app = app.ok_or_else(RpcError::no_ctx)?.clone();
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?.clone();

            // Connect-before-send if client names a session that is not focused.
            if let Some(target_id) = target.as_deref() {
                let live_focused = mgr.snapshot().session_id;
                if live_focused.as_deref() != Some(target_id) {
                    mgr.connect(app.clone(), None, Some(target_id.to_string()), None, None)
                        .await
                        .map_err(RpcError::host)?;
                }
            }
            // Pass the id through so Host re-focuses if another chat stole the
            // live slot between connect and send. Attachments land on the user
            // journal row so history reloads image/file cards.
            let snap = mgr
                .send_message(app, text, display_text, attachments, target)
                .await
                .map_err(RpcError::host)?;
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.stop" => {
            let app = app.ok_or_else(RpcError::no_ctx)?.clone();
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?.clone();
            let session_id = param_string(&params, &["sessionId", "session_id"]);
            let snap = mgr.stop(app, session_id).await.map_err(RpcError::host)?;
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }

        // ── Interactive gates (Slice 6) ──────────────────────────────────
        "session.resolvePermission" => {
            let app = app.ok_or_else(RpcError::no_ctx)?.clone();
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?.clone();
            let rpc_id = param_u64(&params, &["rpcId", "rpc_id"])
                .ok_or_else(|| RpcError::bad_params("rpcId required"))?;
            let decision = param_string(&params, &["decision"])
                .ok_or_else(|| RpcError::bad_params("decision required"))?;
            let option_id = param_string(&params, &["optionId", "option_id"]);
            let scope_key = param_string(&params, &["scopeKey", "scope_key", "scope"]);
            let client_options = params.get("options").cloned();
            let client_tool = param_string(&params, &["toolName", "tool_name"]);
            let snap = mgr
                .resolve_permission(
                    app,
                    rpc_id,
                    decision,
                    option_id,
                    scope_key,
                    param_string(&params, &["sessionId", "session_id"]),
                    client_options,
                    client_tool,
                )
                .await
                .map_err(RpcError::host)?;
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.resolvePlan" => {
            let app = app.ok_or_else(RpcError::no_ctx)?.clone();
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?.clone();
            let decision = param_string(&params, &["decision"])
                .ok_or_else(|| RpcError::bad_params("decision required"))?;
            let feedback = param_string(&params, &["feedback"]);
            let rpc_id = param_u64(&params, &["rpcId", "rpc_id"]);
            let snap = mgr
                .resolve_plan(
                    app,
                    decision,
                    feedback,
                    rpc_id,
                    param_string(&params, &["sessionId", "session_id"]),
                )
                .await
                .map_err(RpcError::host)?;
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }
        "session.resolveAskUser" => {
            let app = app.ok_or_else(RpcError::no_ctx)?.clone();
            let mgr = mgr.ok_or_else(RpcError::no_ctx)?.clone();
            let decision = param_string(&params, &["decision"])
                .ok_or_else(|| RpcError::bad_params("decision required"))?;
            let answers = params.get("answers").cloned().filter(|v| !v.is_null());
            let rpc_id = param_u64(&params, &["rpcId", "rpc_id"]);
            let snap = mgr
                .resolve_ask_user(
                    app,
                    decision,
                    answers,
                    rpc_id,
                    param_string(&params, &["sessionId", "session_id"]),
                )
                .await
                .map_err(RpcError::host)?;
            Ok(serde_json::to_value(snap).map_err(|e| RpcError::host(e.to_string()))?)
        }

        // Explicitly unsupported desktop-only (never crash UI)
        "pick_directory" | "pick_attach_files" | "pick_cli_binary" | "pick_agent_profile"
        | "path_open" | "path_reveal" | "open_in_editor" | "cli_install_latest"
        | "account_login" | "account.login" | "reset_app_data" | "fs_list_dir" | "fs_read_file" => {
            Err(RpcError::unsupported(method))
        }

        _ => Err(RpcError::unsupported(method)),
    }
}

/// Light account blob for hello (no billing refresh).
pub async fn account_summary_light(_app: &AppHandle) -> Value {
    let status = crate::account::account_status(None, false).await;
    json!({
        "signedIn": status.profile.signed_in,
        "displayName": status.profile.display_name.clone()
            .or(status.profile.email.clone()),
        "email": status.profile.email,
        "channel": status.channel,
    })
}

/// Tell every attached surface (desktop WebView + all mirror clients) that the
/// sessions index changed, so each can re-run `sessions.list`. Fired only from
/// the mirror RPC write path — desktop commands already refresh in-process.
fn notify_sessions_changed(app: Option<&AppHandle>, reason: &str, session_id: &str) {
    if let Some(app) = app {
        super::fanout_event(
            app,
            "sessions://changed",
            json!({ "reason": reason, "sessionId": session_id }),
        );
    }
}

fn param_string(params: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = params.get(*k) {
            if v.is_null() {
                continue;
            }
            if let Some(s) = v.as_str() {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// Optional file/image cards on `session.send` (camelCase wire shape).
fn param_attachments(params: &Value) -> Option<Vec<store::MessageAttachmentStored>> {
    let raw = params
        .get("attachments")
        .or_else(|| params.get("Attachments"))?;
    if raw.is_null() {
        return None;
    }
    let arr = raw.as_array()?;
    let mut out = Vec::new();
    for item in arr {
        let Some(path) = item
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                std::path::Path::new(path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string())
            });
        let is_dir = item
            .get("isDir")
            .or_else(|| item.get("is_dir"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        out.push(store::MessageAttachmentStored {
            path: path.to_string(),
            name,
            is_dir,
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn param_u64(params: &Value, keys: &[&str]) -> Option<u64> {
    for k in keys {
        if let Some(v) = params.get(*k) {
            if v.is_null() {
                continue;
            }
            if let Some(n) = v.as_u64() {
                return Some(n);
            }
            if let Some(n) = v.as_i64() {
                if n >= 0 {
                    return Some(n as u64);
                }
            }
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.trim().parse::<u64>() {
                    return Some(n);
                }
            }
            if let Some(f) = v.as_f64() {
                if f.is_finite() && f >= 0.0 && f == f.trunc() {
                    return Some(f as u64);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_host(read_only: bool, allow_remote_yolo: bool) -> Arc<MirrorHost> {
        let host = Arc::new(MirrorHost::from_env());
        host.set_read_only(read_only);
        host.set_allow_remote_yolo(allow_remote_yolo);
        host
    }

    #[tokio::test]
    async fn read_only_allowlist_permits_reads() {
        let host = make_test_host(true, false);
        let mgr = Arc::new(SessionManager::new());

        for &method in READ_METHODS {
            let params = match method {
                "session.messages" => json!({ "sessionId": "dummy-session-id" }),
                _ => json!({}),
            };
            let res = dispatch(method, params, &host, None, Some(&mgr)).await;
            // None of the read methods should fail with "unsupported method: mirror is in read-only mode"
            if let Err(e) = res {
                assert_ne!(
                    e.message, "unsupported method: mirror is in read-only mode",
                    "read method {method} was blocked by read-only gate"
                );
            }
        }
    }

    #[tokio::test]
    async fn read_only_blocks_all_writes() {
        let host = make_test_host(true, false);
        let mgr = Arc::new(SessionManager::new());

        for &method in WRITE_METHODS {
            let res = dispatch(method, json!({}), &host, None, Some(&mgr)).await;
            let err = res.expect_err(&format!("write method {method} should be blocked in read-only"));
            assert_eq!(err.code, "UNSUPPORTED");
            assert_eq!(
                err.message, "unsupported method: mirror is in read-only mode",
                "method {method} should return read-only unsupported error"
            );
        }

        // Also test arbitrary unknown and desktop-only methods in read-only mode
        for arbitrary in &["unknown.method", "pick_directory", "account.login", "fs_read_file"] {
            let res = dispatch(arbitrary, json!({}), &host, None, Some(&mgr)).await;
            let err = res.expect_err(&format!("arbitrary method {arbitrary} should be blocked"));
            assert_eq!(err.code, "UNSUPPORTED");
            assert_eq!(err.message, "unsupported method: mirror is in read-only mode");
        }
    }

    #[tokio::test]
    async fn all_dispatch_methods_classified() {
        // 1. Verify READ_METHODS and WRITE_METHODS are disjoint
        for r in READ_METHODS {
            assert!(
                !WRITE_METHODS.contains(r),
                "method {r} is in both READ_METHODS and WRITE_METHODS"
            );
        }
        for w in WRITE_METHODS {
            assert!(
                !READ_METHODS.contains(w),
                "method {w} is in both WRITE_METHODS and READ_METHODS"
            );
        }

        // 2. Verify all known active functional dispatch methods are classified in either READ or WRITE
        let all_active_methods = [
            "projects.list",
            "sessions.list",
            "session.messages",
            "session.getState",
            "account.status",
            "settings.get",
            "models.list",
            "composer.prefsResolve",
            "voice.status",
            "voice.transcribe",
            "session.connect",
            "session.create",
            "session.rename",
            "session.autoTitle",
            "session.send",
            "session.stop",
            "session.resolvePermission",
            "session.resolvePlan",
            "session.resolveAskUser",
        ];

        for m in all_active_methods {
            let classified = READ_METHODS.contains(&m) || WRITE_METHODS.contains(&m);
            assert!(classified, "active dispatch method {m} must be classified");
        }
        assert_eq!(READ_METHODS.len() + WRITE_METHODS.len(), all_active_methods.len());

        // 3. When read_only is false, write methods must not fail with read-only unsupported error
        let host_rw = make_test_host(false, false);
        for &w in WRITE_METHODS {
            let res = dispatch(w, json!({}), &host_rw, None, None).await;
            if let Err(e) = res {
                assert_ne!(
                    e.message, "unsupported method: mirror is in read-only mode",
                    "write method {w} should not be blocked by read-only gate when read_only=false"
                );
            }
        }
    }

    #[tokio::test]
    async fn allow_remote_yolo_enforcement() {
        let _ = crate::paths::ensure_app_dirs();

        // Create a test session
        let mut session = store::create_session(None, Some("test-remote-yolo".into()), false)
            .expect("create test session");

        // 1. When session policy is ask and allow_remote_yolo is false: permitted past policy check
        session.permission_policy = Some("ask".into());
        store::update_session_meta(&session).expect("update meta");

        let host_no_yolo = make_test_host(false, false);
        let res = dispatch(
            "session.send",
            json!({ "sessionId": session.id, "text": "hello" }),
            &host_no_yolo,
            None,
            None,
        )
        .await;
        // Permitted past policy check, then fails on missing app/mgr context (NOT_READY)
        let err = res.expect_err("should fail on missing ctx, not policy");
        assert_eq!(err.code, "NOT_READY");

        // 2. When session policy is relaxed and allow_remote_yolo is false: refused
        for relaxed in &["always_approve", "dont_ask", "auto", "accept_edits"] {
            session.permission_policy = Some((*relaxed).into());
            store::update_session_meta(&session).expect("update meta");

            let res = dispatch(
                "session.send",
                json!({ "sessionId": session.id, "text": "hello" }),
                &host_no_yolo,
                None,
                None,
            )
            .await;
            let err = res.expect_err(&format!("policy {relaxed} must be refused when allow_remote_yolo is false"));
            assert_eq!(err.code, "HOST_ERROR");
            assert!(
                err.message.contains("refusing remote send"),
                "error message should explain refusal: {}",
                err.message
            );
            assert!(
                err.message.contains(relaxed),
                "error message should cite policy {}: {}",
                relaxed,
                err.message
            );
            assert!(
                err.message.contains("mirror remote YOLO is disabled"),
                "error message should cite remote YOLO disabled: {}",
                err.message
            );
        }

        // 3. When session policy is relaxed and allow_remote_yolo is true: permitted past policy check
        let host_yolo = make_test_host(false, true);
        for relaxed in &["always_approve", "dont_ask", "auto", "accept_edits"] {
            session.permission_policy = Some((*relaxed).into());
            store::update_session_meta(&session).expect("update meta");

            let res = dispatch(
                "session.send",
                json!({ "sessionId": session.id, "text": "hello" }),
                &host_yolo,
                None,
                None,
            )
            .await;
            let err = res.expect_err("should pass policy check and fail on missing ctx");
            assert_eq!(
                err.code, "NOT_READY",
                "relaxed policy {relaxed} must be permitted past policy check when allow_remote_yolo is true"
            );
        }

        // Clean up
        let _ = store::delete_session(&session.id);
    }

    #[tokio::test]
    async fn session_connect_rejects_untrusted_and_unregistered_project_path() {
        let host = make_test_host(false, false);
        let mgr = Arc::new(SessionManager::new());

        // 1. Non-registered path
        let res = dispatch(
            "session.connect",
            json!({ "projectPath": "/unregistered/arbitrary/path/xyz" }),
            &host,
            None,
            Some(&mgr),
        )
        .await;
        let err = res.expect_err("non-registered project path should be rejected");
        assert_eq!(err.code, "BAD_PARAMS");
        assert_eq!(err.message, "projectPath must be a registered, trusted project");

        // 2. Registered untrusted project path
        let mut list = store::load_projects();
        let untrusted_path = "/tmp/test-untrusted-proj-xyz";
        let untrusted_proj = store::Project {
            id: "test-untrusted-id-123".into(),
            name: "untrusted-test".into(),
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
        };
        list.push(untrusted_proj);
        let _ = store::save_projects(&list);

        let res2 = dispatch(
            "session.connect",
            json!({ "projectPath": untrusted_path }),
            &host,
            None,
            Some(&mgr),
        )
        .await;
        let err2 = res2.expect_err("registered untrusted project path should be rejected");
        assert_eq!(err2.code, "BAD_PARAMS");
        assert_eq!(err2.message, "projectPath must be a registered, trusted project");

        // Clean up project
        let mut clean_list = store::load_projects();
        clean_list.retain(|p| p.id != "test-untrusted-id-123");
        let _ = store::save_projects(&clean_list);
    }
}


