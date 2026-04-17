use super::behavior::{NewWindowBehavior, StartupBehavior};
use crate::state::SidebarPanel;
use serde::{Deserialize, Serialize};

/// Default sidebar width in pixels
pub const DEFAULT_SIDEBAR_WIDTH: f64 = 280.0;

/// Default sidebar zoom level
pub const DEFAULT_SIDEBAR_ZOOM_LEVEL: f64 = 1.0;

/// Minimum zoom level for sidebar
pub const MIN_SIDEBAR_ZOOM: f64 = 0.5;

/// Maximum zoom level for sidebar
pub const MAX_SIDEBAR_ZOOM: f64 = 2.0;

/// Zoom step for sidebar (0.1 increments)
pub const ZOOM_STEP: f64 = 0.1;

fn default_sidebar_width() -> f64 {
    DEFAULT_SIDEBAR_WIDTH
}

fn default_sidebar_zoom_level() -> f64 {
    DEFAULT_SIDEBAR_ZOOM_LEVEL
}

/// Normalize and clamp zoom level to valid range with 0.1 step
pub fn normalize_zoom_level(zoom: f64) -> f64 {
    if zoom.is_nan() || zoom.is_infinite() {
        return DEFAULT_SIDEBAR_ZOOM_LEVEL;
    }
    // Clamp to valid range
    let clamped = zoom.clamp(MIN_SIDEBAR_ZOOM, MAX_SIDEBAR_ZOOM);
    // Snap to 0.1 step
    (clamped / ZOOM_STEP).round() * ZOOM_STEP
}

/// Configuration for sidebar-related settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarConfig {
    /// Whether sidebar is pinned to the layout by default
    pub default_pinned: bool,
    /// Default panel content
    #[serde(default)]
    pub default_panel: SidebarPanel,
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
            default_panel: SidebarPanel::Directory,
            default_width: default_sidebar_width(),
            default_show_all_files: false,
            default_zoom_level: default_sidebar_zoom_level(),
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
