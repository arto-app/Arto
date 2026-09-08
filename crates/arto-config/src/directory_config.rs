use super::behavior::StartupBehavior;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for directory-related settings
///
/// There is no `onNewWindow` here: which folders a new window starts with is
/// decided by the command that opened it — Cmd+N carries the places alone,
/// Cmd+Shift+N duplicates this window's temporary roots as well — rather
/// than by a setting that has to guess between them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryConfig {
    /// Default directory to open
    pub default_directory: Option<PathBuf>,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
}
