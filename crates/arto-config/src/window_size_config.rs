use super::window_dimension::{WindowDimension, WindowDimensionUnit};
use super::{NewWindowBehavior, StartupBehavior};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub width: WindowDimension,
    pub height: WindowDimension,
}

impl WindowSize {
    /// Resolve to a concrete size within a screen of the given size.
    ///
    /// Returns plain numbers; the app wraps them in its window toolkit's
    /// size type.
    pub fn resolve(self, screen_width: f64, screen_height: f64) -> (f64, f64) {
        (
            self.width.clamp_percent().resolve(screen_width),
            self.height.clamp_percent().resolve(screen_height),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_size_resolves_against_screen() {
        let size = WindowSize {
            width: WindowDimension {
                value: 50.0,
                unit: WindowDimensionUnit::Percent,
            },
            height: WindowDimension {
                value: 100.0,
                unit: WindowDimensionUnit::Percent,
            },
        };
        assert_eq!(size.resolve(2000.0, 1000.0), (1000.0, 1000.0));
    }
}
