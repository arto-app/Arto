use super::window_dimension::{WindowDimension, WindowDimensionUnit};
use super::{NewWindowBehavior, StartupBehavior};
use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowPositionMode {
    Coordinates,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: WindowDimension,
    pub y: WindowDimension,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPositionOffset {
    pub x: i32,
    pub y: i32,
}

impl WindowPosition {
    /// Convert to a logical position within the available screen area.
    /// Percent values are resolved against the available width/height
    /// (e.g., 50% x 50% centers the window in the usable space).
    pub fn to_logical_position(self, screen_size: LogicalSize<i32>) -> LogicalPosition<i32> {
        LogicalPosition::new(
            self.x.clamp_percent().resolve(screen_size.width as f64) as i32,
            self.y.clamp_percent().resolve(screen_size.height as f64) as i32,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowPositionConfig {
    pub default_position: WindowPosition,
    pub default_position_mode: WindowPositionMode,
    pub position_offset: WindowPositionOffset,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for WindowPositionConfig {
    fn default() -> Self {
        Self {
            default_position: WindowPosition {
                x: WindowDimension {
                    value: 50.0,
                    unit: WindowDimensionUnit::Percent,
                },
                y: WindowDimension {
                    value: 50.0,
                    unit: WindowDimensionUnit::Percent,
                },
            },
            default_position_mode: WindowPositionMode::Coordinates,
            position_offset: WindowPositionOffset { x: 20, y: 20 },
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
