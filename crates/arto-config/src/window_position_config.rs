use super::window_dimension::{WindowDimension, WindowDimensionUnit};
use super::{NewWindowBehavior, StartupBehavior};
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
    /// Resolve to absolute coordinates within an area of the given size.
    /// Percent values are resolved against the available width/height
    /// (e.g., 50% x 50% centers the window in the usable space).
    ///
    /// Returns plain numbers; the app wraps them in its window toolkit's
    /// position type.
    pub fn resolve(self, available_width: f64, available_height: f64) -> (f64, f64) {
        (
            self.x.clamp_percent().resolve(available_width),
            self.y.clamp_percent().resolve(available_height),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_position_resolves_against_available_area() {
        let position = WindowPosition {
            x: WindowDimension {
                value: 50.0,
                unit: WindowDimensionUnit::Percent,
            },
            y: WindowDimension {
                value: 25.0,
                unit: WindowDimensionUnit::Percent,
            },
        };
        assert_eq!(position.resolve(1000.0, 800.0), (500.0, 200.0));
    }

    #[test]
    fn pixel_position_is_kept_as_is() {
        let position = WindowPosition {
            x: WindowDimension {
                value: 120.0,
                unit: WindowDimensionUnit::Pixels,
            },
            y: WindowDimension {
                value: 40.0,
                unit: WindowDimensionUnit::Pixels,
            },
        };
        assert_eq!(position.resolve(1000.0, 800.0), (120.0, 40.0));
    }
}
