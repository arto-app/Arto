use super::window_dimension::{WindowDimension, WindowDimensionUnit};
use super::{NewWindowBehavior, StartupBehavior};
use dioxus::desktop::tao::dpi::LogicalSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub width: WindowDimension,
    pub height: WindowDimension,
}

impl WindowSize {
    pub fn to_logical_size(self, screen_size: &LogicalSize<f64>) -> LogicalSize<f64> {
        LogicalSize::new(
            self.width.clamp_percent().resolve(screen_size.width),
            self.height.clamp_percent().resolve(screen_size.height),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowSizeConfig {
    pub default_size: WindowSize,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for WindowSizeConfig {
    fn default() -> Self {
        Self {
            default_size: WindowSize {
                width: WindowDimension {
                    value: 1000.0,
                    unit: WindowDimensionUnit::Pixels,
                },
                height: WindowDimension {
                    value: 800.0,
                    unit: WindowDimensionUnit::Pixels,
                },
            },
            on_startup: StartupBehavior::Default,
            on_new_window: NewWindowBehavior::Default,
        }
    }
}
