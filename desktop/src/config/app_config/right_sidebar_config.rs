use super::behavior::{NewWindowBehavior, StartupBehavior};
use crate::state::SidebarPanel;
use serde::{Deserialize, Serialize};

/// Default right sidebar panel width in pixels
pub const DEFAULT_RIGHT_SIDEBAR_WIDTH: f64 = 220.0;

/// Default right sidebar zoom level
pub const DEFAULT_RIGHT_SIDEBAR_ZOOM_LEVEL: f64 = 1.0;

fn default_right_sidebar_width() -> f64 {
    DEFAULT_RIGHT_SIDEBAR_WIDTH
}

fn default_right_sidebar_zoom_level() -> f64 {
    DEFAULT_RIGHT_SIDEBAR_ZOOM_LEVEL
}

fn default_right_sidebar_panel() -> SidebarPanel {
    SidebarPanel::Contents
}

/// Configuration for right sidebar panel settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightSidebarConfig {
    /// Whether right sidebar panel is pinned to the layout by default
    pub default_pinned: bool,
    /// Default right sidebar panel width in pixels
    #[serde(default = "default_right_sidebar_width")]
    pub default_width: f64,
    /// Default panel content
    #[serde(default = "default_right_sidebar_panel", alias = "defaultTab")]
    pub default_panel: SidebarPanel,
    /// Default zoom level for right sidebar content
    #[serde(default = "default_right_sidebar_zoom_level")]
    pub default_zoom_level: f64,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for RightSidebarConfig {
    fn default() -> Self {
        Self {
            default_pinned: false,
            default_width: default_right_sidebar_width(),
            default_panel: default_right_sidebar_panel(),
            default_zoom_level: default_right_sidebar_zoom_level(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
