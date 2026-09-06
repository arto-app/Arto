use super::behavior::{NewWindowBehavior, StartupBehavior};
use super::zoom_config::DEFAULT_ZOOM_LEVEL;
use serde::{Deserialize, Serialize};

/// Default right sidebar panel width in pixels
pub const DEFAULT_RIGHT_SIDEBAR_WIDTH: f64 = 220.0;

fn default_right_sidebar_width() -> f64 {
    DEFAULT_RIGHT_SIDEBAR_WIDTH
}

fn default_right_sidebar_zoom_level() -> f64 {
    DEFAULT_ZOOM_LEVEL
}

/// The panels the right sidebar can show.
///
/// Defined here, with the configuration that names a default, so the
/// setting does not depend on the UI that renders the tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RightSidebarTab {
    #[default]
    Contents,
    Search,
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
    /// Default active tab
    #[serde(default)]
    pub default_tab: RightSidebarTab,
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
            default_tab: RightSidebarTab::default(),
            default_zoom_level: default_right_sidebar_zoom_level(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
