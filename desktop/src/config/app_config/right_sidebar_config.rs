use super::behavior::{NewWindowBehavior, StartupBehavior};
use crate::components::right_sidebar::RightSidebarTab;
use serde::{Deserialize, Serialize};

/// Default right sidebar panel width in pixels
pub const DEFAULT_RIGHT_SIDEBAR_WIDTH: f64 = 220.0;

fn default_right_sidebar_width() -> f64 {
    DEFAULT_RIGHT_SIDEBAR_WIDTH
}

/// Configuration for right sidebar panel settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightSidebarConfig {
    /// Whether right sidebar panel is open by default
    pub default_open: bool,
    /// Default right sidebar panel width in pixels
    #[serde(default = "default_right_sidebar_width")]
    pub default_width: f64,
    /// Default active tab
    #[serde(default)]
    pub default_tab: RightSidebarTab,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for RightSidebarConfig {
    fn default() -> Self {
        Self {
            default_open: false,
            default_width: default_right_sidebar_width(),
            default_tab: RightSidebarTab::default(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
