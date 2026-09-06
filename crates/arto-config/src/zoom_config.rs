use serde::{Deserialize, Serialize};

use super::behavior::{NewWindowBehavior, StartupBehavior};

/// Neutral zoom level (100%), used as the default and as the fallback for
/// non-finite input.
pub const DEFAULT_ZOOM_LEVEL: f64 = 1.0;

/// Zoom range for the content area. Content tolerates far more magnification
/// than the sidebars, whose fixed-width chrome breaks down past 2x.
pub const MIN_CONTENT_ZOOM: f64 = 0.5;
pub const MAX_CONTENT_ZOOM: f64 = 5.0;

/// Zoom range for the left and right sidebar panels.
pub const MIN_SIDEBAR_ZOOM: f64 = 0.5;
pub const MAX_SIDEBAR_ZOOM: f64 = 2.0;

/// Increment applied by a single zoom in/out action. Normalization snaps to
/// this same grid.
pub const ZOOM_STEP: f64 = 0.1;

fn default_zoom_level() -> f64 {
    DEFAULT_ZOOM_LEVEL
}

/// Snap to the nearest 0.1 step and clamp into `[min, max]`.
///
/// Snapping prevents precision drift from repeated zoom in/out steps, so the
/// level stays on the same grid the menu and keybinding actions move along.
fn normalize_zoom(zoom: f64, min: f64, max: f64) -> f64 {
    if !zoom.is_finite() {
        return DEFAULT_ZOOM_LEVEL;
    }
    ((zoom * 10.0).round() / 10.0).clamp(min, max)
}

/// Normalize a content-area zoom level.
pub fn normalize_content_zoom(zoom: f64) -> f64 {
    normalize_zoom(zoom, MIN_CONTENT_ZOOM, MAX_CONTENT_ZOOM)
}

/// Normalize a sidebar panel zoom level.
pub fn normalize_sidebar_zoom(zoom: f64) -> f64 {
    normalize_zoom(zoom, MIN_SIDEBAR_ZOOM, MAX_SIDEBAR_ZOOM)
}

/// Configuration for zoom-related settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomConfig {
    /// Default zoom level (1.0 = 100%)
    #[serde(default = "default_zoom_level")]
    pub default_zoom_level: f64,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

// Manual Default because f64's default is 0.0, but zoom default should be 1.0
impl Default for ZoomConfig {
    fn default() -> Self {
        Self {
            default_zoom_level: DEFAULT_ZOOM_LEVEL,
            on_startup: StartupBehavior::default(),
            on_new_window: NewWindowBehavior::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_zoom_rounds_to_nearest_tenth() {
        assert_eq!(normalize_content_zoom(1.05), 1.1);
        assert_eq!(normalize_content_zoom(1.04), 1.0);
        assert_eq!(normalize_content_zoom(1.95), 2.0);
        assert_eq!(normalize_content_zoom(0.99), 1.0);
    }

    #[test]
    fn content_zoom_clamps_to_range() {
        assert_eq!(normalize_content_zoom(0.3), MIN_CONTENT_ZOOM);
        assert_eq!(normalize_content_zoom(10.0), MAX_CONTENT_ZOOM);
        assert_eq!(normalize_content_zoom(-1.0), MIN_CONTENT_ZOOM);
    }

    #[test]
    fn sidebar_zoom_clamps_tighter_than_content() {
        // 3.0 is a valid content zoom but past the sidebar ceiling.
        assert_eq!(normalize_content_zoom(3.0), 3.0);
        assert_eq!(normalize_sidebar_zoom(3.0), MAX_SIDEBAR_ZOOM);
    }

    #[test]
    fn non_finite_zoom_falls_back_to_default() {
        assert_eq!(normalize_content_zoom(f64::NAN), DEFAULT_ZOOM_LEVEL);
        assert_eq!(normalize_content_zoom(f64::INFINITY), DEFAULT_ZOOM_LEVEL);
        assert_eq!(normalize_sidebar_zoom(f64::NAN), DEFAULT_ZOOM_LEVEL);
    }

    #[test]
    fn aligned_values_are_preserved() {
        assert_eq!(normalize_content_zoom(1.0), 1.0);
        assert_eq!(normalize_content_zoom(1.5), 1.5);
        assert_eq!(normalize_sidebar_zoom(2.0), 2.0);
    }
}
