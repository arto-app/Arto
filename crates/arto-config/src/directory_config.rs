use super::behavior::{NewWindowBehavior, StartupBehavior};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for directory-related settings
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryConfig {
    /// Default directory to open
    pub default_directory: Option<PathBuf>,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}
