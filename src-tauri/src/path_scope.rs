//! Unified path allowlist for absolute filesystem access.
//!
//! Used by loopback media HTTP / legacy `media://` (SEC-01) and
//! `fs_read_absolute` / `fs_write_absolute` (SEC-09).
//! Only trusted project roots, the App data root, system temp, and explicitly
//! granted one-off paths may be read/written.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::RwLock;

fn roots() -> &'static RwLock<Vec<PathBuf>> {
    static R: OnceLock<RwLock<Vec<PathBuf>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(Vec::new()))
}

fn extra_grants() -> &'static RwLock<Vec<PathBuf>> {
    static G: OnceLock<RwLock<Vec<PathBuf>>> = OnceLock::new();
    G.get_or_init(|| RwLock::new(Vec::new()))
}

/// Rebuild allowlisted roots from the project store + app data + temp.
/// Call on startup and whenever projects are added / removed / relocated / trusted.
pub fn refresh_from_store() {
    let mut next: Vec<PathBuf> = crate::store::load_projects()
        .into_iter()
        .filter(|p| p.trusted)
        .filter_map(|p| PathBuf::from(p.path).canonicalize().ok())
        .collect();

    if let Ok(app) = crate::paths::app_data_root().canonicalize() {
        next.push(app);
    } else {
        // Dir may not exist yet — still allow the logical root.
        next.push(crate::paths::app_data_root());
    }

    if let Ok(tmp) = std::env::temp_dir().canonicalize() {
        next.push(tmp);
    } else {
        next.push(std::env::temp_dir());
    }

    // Agent session media (`images/`, `videos/`) lives under GROK_HOME.
    // Independent mode is already under app_data; shared mode is `~/.grok` and
    // must be listed so chat image/video cards can load via media HTTP.
    let settings = crate::store::load_settings();
    let agent_home = crate::paths::resolve_agent_grok_home(&settings.session_data_mode);
    if let Ok(c) = agent_home.canonicalize() {
        next.push(c);
    } else {
        next.push(agent_home);
    }

    // CLI default home (`~/.grok`): marketplace-cache logos + installed-plugins
    // assets for Settings → Extensions cards (may differ from independent agent-home).
    let user_grok = crate::process_util::user_home().join(".grok");
    if let Ok(c) = user_grok.canonicalize() {
        next.push(c);
    } else {
        next.push(user_grok);
    }

    // Dedup while preserving order.
    let mut seen = std::collections::HashSet::new();
    next.retain(|p| seen.insert(p.clone()));

    *roots().write() = next;
}

/// Grant a one-off absolute path (e.g. user-picked file outside projects).
///
/// Files are granted **exactly** — never the parent directory. Granting the
/// parent used to let loopback media serve siblings (e.g. `~/.ssh/id_rsa`
/// classified from journal history also unlocked `id_rsa.pub`). Re-reads of
/// the same file still work because `is_allowed` treats the grant as a root
/// that matches that path.
pub fn grant_path(path: &Path) {
    let grant = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut g = extra_grants().write();
    if !g.iter().any(|x| x == &grant) {
        g.push(grant);
        // Cap grants so a long-running process cannot grow unbounded.
        const MAX_GRANTS: usize = 256;
        if g.len() > MAX_GRANTS {
            let drain = g.len() - MAX_GRANTS;
            g.drain(0..drain);
        }
    }
}

/// True when `path` matches sensitive files or directories that must never be accessed.
pub fn is_denied_target(path: &Path) -> bool {
    let comps: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // 1. Check for denied directory components (.ssh, .aws, .gnupg, .kube)
    for c in &comps {
        if *c == ".ssh" || *c == ".aws" || *c == ".gnupg" || *c == ".kube" {
            return true;
        }
    }

    // 2. Check for .config/gh
    for i in 0..comps.len() {
        if comps[i] == ".config" && i + 1 < comps.len() && comps[i + 1] == "gh" {
            return true;
        }
    }

    // 3. Check filename-specific denies
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        if file_name == "secrets.json"
            || file_name == "session-api.json"
            || file_name == ".netrc"
            || file_name == ".npmrc"
        {
            return true;
        }

        if file_name == "auth.json" {
            let in_sensitive_dir = comps.iter().any(|c| *c == ".grok" || *c == "agent-home");
            let in_app_data = path_under_root(path, &crate::paths::app_data_root());
            if in_sensitive_dir || in_app_data {
                return true;
            }
        }

        // agent-home/config.toml only (not every config.toml).
        if file_name == "config.toml" && comps.len() >= 2 && comps[comps.len() - 2] == "agent-home"
        {
            return true;
        }

        // .docker/config.json only (not all of .docker).
        if file_name == "config.json" && comps.len() >= 2 && comps[comps.len() - 2] == ".docker" {
            return true;
        }

        // 4. remote-im/*.json
        if file_name.ends_with(".json") && comps.contains(&"remote-im") {
            return true;
        }
    }

    false
}

/// True when `path` sits under an allowed root (after canonicalize when possible).
pub fn is_allowed(path: &Path) -> bool {
    if is_denied_target(path) {
        return false;
    }
    let candidate = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    is_allowed_canonical(&candidate)
}

fn is_allowed_canonical(path: &Path) -> bool {
    if is_denied_target(path) {
        return false;
    }
    if roots().read().is_empty() {
        // Lazy init on first check (tests / early calls before setup).
        refresh_from_store();
    }
    let under_root = roots().read().iter().any(|r| path_under_root(path, r));
    if under_root {
        return true;
    }
    extra_grants()
        .read()
        .iter()
        .any(|r| path_under_root(path, r))
}

fn path_under_root(path: &Path, root: &Path) -> bool {
    if path == root {
        return true;
    }
    // Use component-wise prefix so `/foo` does not match `/foobar`.
    let mut path_comps = path.components();
    for rc in root.components() {
        match path_comps.next() {
            Some(pc) if pc == rc => {}
            _ => return false,
        }
    }
    true
}

/// Canonicalize + allowlist gate. Returns the canonical path on success.
pub fn require_allowed(path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("path not found: {e}"))?;
    if !is_allowed_canonical(&canonical) {
        tracing::warn!(
            path = %canonical.display(),
            "path_scope: denied absolute path outside allowlisted roots"
        );
        return Err("path not allowed: outside trusted project or app data roots".into());
    }
    Ok(canonical)
}

/// Serializes tests that mutate the process-global roots/grants.
/// `tokio::sync` so async media_server tests can hold it across `.await`.
#[cfg(test)]
pub(crate) static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Grant `path` for the duration of `f` while holding [`TEST_LOCK`].
#[cfg(test)]
pub(crate) fn with_granted_path(path: &Path, f: impl FnOnce()) {
    let _g = TEST_LOCK.blocking_lock();
    extra_grants().write().clear();
    grant_path(path);
    f();
    extra_grants().write().clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct RestoreHome(Option<String>);
    impl Drop for RestoreHome {
        fn drop(&mut self) {
            match self.0.take() {
                Some(v) => std::env::set_var("GROK_APP_HOME", v),
                None => std::env::remove_var("GROK_APP_HOME"),
            }
        }
    }

    fn with_isolated_roots(project: &Path, app: &Path, include_temp: bool, f: impl FnOnce()) {
        let _g = TEST_LOCK.blocking_lock();
        let _home = crate::paths::APP_HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var("GROK_APP_HOME").ok();
        std::env::set_var("GROK_APP_HOME", app);
        let _restore = RestoreHome(prev);
        let _ = fs::create_dir_all(app);
        let _ = fs::create_dir_all(project);
        let projects_file = app.join("projects.json");
        let _ = fs::write(&projects_file, "[]");
        // Install a deterministic root set (optional temp) so tests do not depend on the
        // real machine project list or always-on temp allow.
        let mut next = Vec::new();
        if let Ok(c) = project.canonicalize() {
            next.push(c);
        }
        if let Ok(c) = app.canonicalize() {
            next.push(c);
        } else {
            next.push(app.to_path_buf());
        }
        if include_temp {
            if let Ok(c) = std::env::temp_dir().canonicalize() {
                next.push(c);
            }
        }
        *roots().write() = next;
        *extra_grants().write() = Vec::new();
        f();
        *extra_grants().write() = Vec::new();
        *roots().write() = Vec::new();
    }

    #[test]
    fn allows_path_under_project() {
        let tmp = std::env::temp_dir().join(format!("grok-scope-{}", std::process::id()));
        let project = tmp.join("proj");
        let app = tmp.join("app");
        let _ = fs::create_dir_all(&project);
        let file = project.join("readme.md");
        fs::write(&file, "hi").unwrap();
        with_isolated_roots(&project, &app, false, || {
            assert!(is_allowed(&file));
            assert!(require_allowed(&file).is_ok());
        });
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn denies_path_outside_roots() {
        let tmp = std::env::temp_dir().join(format!("grok-scope-out-{}", std::process::id()));
        let project = tmp.join("proj");
        let app = tmp.join("app");
        let _ = fs::create_dir_all(&project);
        let _ = fs::create_dir_all(tmp.join("other"));
        let outside = tmp.join("other").join("secret.txt");
        fs::write(&outside, "secret").unwrap();
        // No global temp root — sibling of project must be denied.
        with_isolated_roots(&project, &app, false, || {
            assert!(!is_allowed(&outside));
            assert!(require_allowed(&outside).is_err());
        });
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn grant_path_allows_one_off() {
        let tmp = std::env::temp_dir().join(format!("grok-scope-grant-{}", std::process::id()));
        let project = tmp.join("proj");
        let app = tmp.join("app");
        let other = tmp.join("picked");
        let _ = fs::create_dir_all(&project);
        let _ = fs::create_dir_all(&other);
        let file = other.join("picked.md");
        fs::write(&file, "x").unwrap();
        with_isolated_roots(&project, &app, false, || {
            assert!(!is_allowed(&file));
            grant_path(&file);
            assert!(is_allowed(&file));
            let sibling = other.join("secret.key");
            fs::write(&sibling, "no").unwrap();
            assert!(
                !is_allowed(&sibling),
                "granting a file must not unlock siblings in the parent dir"
            );
        });
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn prefix_does_not_match_sibling_name() {
        // /foo should not allow /foobar
        let foo = PathBuf::from("/foo");
        let foobar = PathBuf::from("/foobar/x");
        assert!(!path_under_root(&foobar, &foo));
        assert!(path_under_root(Path::new("/foo/bar"), &foo));
    }

    #[test]
    fn denies_sensitive_targets_even_under_allowed_roots() {
        let tmp = std::env::temp_dir().join(format!("grok-scope-deny-{}", std::process::id()));
        let project = tmp.join("proj");
        let app = tmp.join("app");
        let _ = fs::create_dir_all(&project);
        let _ = fs::create_dir_all(&app);

        let secrets = app.join("secrets.json");
        fs::write(&secrets, "{}").unwrap();

        let session_api = app.join("session-api.json");
        fs::write(&session_api, "{}").unwrap();

        let agent_home = app.join("agent-home");
        let _ = fs::create_dir_all(&agent_home);
        let auth_json = agent_home.join("auth.json");
        fs::write(&auth_json, "{}").unwrap();

        let remote_im_dir = app.join("remote-im");
        let _ = fs::create_dir_all(&remote_im_dir);
        let remote_im_cfg = remote_im_dir.join("config.json");
        fs::write(&remote_im_cfg, "{}").unwrap();

        let ssh_dir = project.join(".ssh");
        let _ = fs::create_dir_all(&ssh_dir);
        let id_rsa = ssh_dir.join("id_rsa");
        fs::write(&id_rsa, "key").unwrap();

        let aws_dir = project.join(".aws");
        let _ = fs::create_dir_all(&aws_dir);
        let aws_cred = aws_dir.join("credentials");
        fs::write(&aws_cred, "cred").unwrap();

        let gnupg_dir = project.join(".gnupg");
        let _ = fs::create_dir_all(&gnupg_dir);
        let gpg_key = gnupg_dir.join("secring.gpg");
        fs::write(&gpg_key, "gpg").unwrap();

        let gh_dir = project.join(".config").join("gh");
        let _ = fs::create_dir_all(&gh_dir);
        let gh_hosts = gh_dir.join("hosts.yml");
        fs::write(&gh_hosts, "oauth_token").unwrap();

        let normal_file = project.join("src").join("main.rs");
        let _ = fs::create_dir_all(project.join("src"));
        fs::write(&normal_file, "fn main() {}").unwrap();

        with_isolated_roots(&project, &app, false, || {
            assert!(!is_allowed(&secrets), "secrets.json should be denied");
            assert!(!is_allowed(&session_api), "session-api.json should be denied");
            assert!(!is_allowed(&auth_json), "agent-home/auth.json should be denied");
            assert!(!is_allowed(&remote_im_cfg), "remote-im/config.json should be denied");
            assert!(!is_allowed(&id_rsa), ".ssh/id_rsa should be denied");
            assert!(!is_allowed(&ssh_dir), ".ssh directory should be denied");
            assert!(!is_allowed(&aws_cred), ".aws/credentials should be denied");
            assert!(!is_allowed(&gpg_key), ".gnupg/secring.gpg should be denied");
            assert!(!is_allowed(&gh_hosts), ".config/gh/hosts.yml should be denied");

            assert!(require_allowed(&secrets).is_err());
            assert!(require_allowed(&id_rsa).is_err());

            assert!(is_allowed(&normal_file));
            assert!(require_allowed(&normal_file).is_ok());
        });

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn denies_s4_leftover_targets_even_under_allowed_roots() {
        let tmp =
            std::env::temp_dir().join(format!("grok-scope-s4-leftover-{}", std::process::id()));
        let project = tmp.join("proj");
        let app = tmp.join("app");
        let _ = fs::create_dir_all(&project);
        let _ = fs::create_dir_all(&app);

        let agent_home = app.join("agent-home");
        let _ = fs::create_dir_all(&agent_home);
        let agent_home_toml = agent_home.join("config.toml");
        fs::write(&agent_home_toml, "").unwrap();

        let netrc = project.join(".netrc");
        fs::write(&netrc, "").unwrap();

        let kube_dir = project.join(".kube");
        let _ = fs::create_dir_all(&kube_dir);
        let kube_config = kube_dir.join("config");
        fs::write(&kube_config, "").unwrap();

        let docker_dir = project.join(".docker");
        let _ = fs::create_dir_all(&docker_dir);
        let docker_cfg = docker_dir.join("config.json");
        fs::write(&docker_cfg, "{}").unwrap();
        let docker_daemon = docker_dir.join("daemon.json");
        fs::write(&docker_daemon, "{}").unwrap();

        let npmrc = project.join(".npmrc");
        fs::write(&npmrc, "").unwrap();

        let project_toml = project.join("config.toml");
        fs::write(&project_toml, "").unwrap();

        with_isolated_roots(&project, &app, false, || {
            assert!(is_denied_target(&agent_home_toml));
            assert!(!is_allowed(&agent_home_toml));
            assert!(is_denied_target(&netrc));
            assert!(!is_allowed(&netrc));
            assert!(is_denied_target(&kube_config));
            assert!(!is_allowed(&kube_config));
            assert!(is_denied_target(&kube_dir));
            assert!(!is_allowed(&kube_dir));
            assert!(is_denied_target(&docker_cfg));
            assert!(!is_allowed(&docker_cfg));
            assert!(is_denied_target(&npmrc));
            assert!(!is_allowed(&npmrc));

            assert!(!is_denied_target(&project_toml));
            assert!(is_allowed(&project_toml));
            assert!(!is_denied_target(&docker_daemon));

            grant_path(&agent_home_toml);
            assert!(
                !is_allowed(&agent_home_toml),
                "grant_path must not unlock agent-home/config.toml"
            );
        });

        let _ = fs::remove_dir_all(&tmp);
    }
}
