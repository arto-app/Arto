use super::behavior::{NewWindowBehavior, StartupBehavior};
use crate::theme::Theme;
use serde::{Deserialize, Serialize};

/// Configuration for theme-related settings
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    /// Default theme preference
    pub default_theme: Theme,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}
