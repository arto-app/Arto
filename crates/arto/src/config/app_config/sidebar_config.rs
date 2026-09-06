use super::behavior::{NewWindowBehavior, StartupBehavior};
use super::zoom_config::DEFAULT_ZOOM_LEVEL;
use serde::{Deserialize, Serialize};

/// Default sidebar width in pixels
pub const DEFAULT_SIDEBAR_WIDTH: f64 = 280.0;

fn default_sidebar_width() -> f64 {
    DEFAULT_SIDEBAR_WIDTH
}

fn default_sidebar_zoom_level() -> f64 {
    DEFAULT_ZOOM_LEVEL
}

/// Configuration for sidebar-related settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarConfig {
    /// Whether sidebar is pinned to the layout by default
    pub default_pinned: bool,
    /// Default sidebar width in pixels
    #[serde(default = "default_sidebar_width")]
    pub default_width: f64,
    /// Whether to show all files (including non-markdown) by default
    pub default_show_all_files: bool,
    /// Default zoom level for sidebar content
    #[serde(default = "default_sidebar_zoom_level")]
    pub default_zoom_level: f64,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            default_pinned: false,
            default_width: default_sidebar_width(),
            default_show_all_files: false,
            default_zoom_level: default_sidebar_zoom_level(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
