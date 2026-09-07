//! Tauri commands — Host facade.
//!
//! Domain modules are `include!`d into this crate module so command symbols
//! stay at `commands::foo` for `generate_handler!` and cross-calls keep working
//! without path churn. Keep this facade thin (gate: ≤800 lines).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::cli_probe::{self, CliProbeResult};
use crate::session_manager::{SessionManager, SessionSnapshot};
use crate::store::{self, AppSettings, Project, SessionMeta};

include!("session_p1.rs");
include!("session_p2.rs");
include!("automation.rs");
include!("settings.rs");
include!("doctor_p1.rs");
include!("doctor_p2.rs");
include!("extensions_p1.rs");
include!("extensions_p2.rs");
include!("fs.rs");
include!("git_p1.rs");
include!("git_p2.rs");
include!("account.rs");
include!("providers.rs");
include!("worktree_agents_p1.rs");
include!("worktree_agents_p2.rs");
include!("hooks_setup_p1.rs");
include!("hooks_setup_p2.rs");
include!("misc_p1.rs");
include!("misc_p2.rs");
include!("terminal.rs");
include!("skin.rs");

pub(crate) const MAIN_ONLY_IPC_ERR: &str = "this command may only be invoked from the main window";

pub(crate) fn require_main_window_label(label: &str) -> Result<(), String> {
    if label.trim() == "main" {
        Ok(())
    } else {
        Err(MAIN_ONLY_IPC_ERR.into())
    }
}

#[cfg(test)]
mod ipc_gate_tests {
    use super::*;

    #[test]
    fn require_main_window_label_allows_only_main() {
        assert!(require_main_window_label("main").is_ok());
        assert!(require_main_window_label("  main  ").is_ok());
        for label in ["session-abc", "pet", "theme-editor", "", "Main"] {
            assert_eq!(
                require_main_window_label(label).unwrap_err(),
                MAIN_ONLY_IPC_ERR
            );
        }
    }
}
