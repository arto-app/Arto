use super::behavior::{NewWindowBehavior, StartupBehavior};
use crate::color_theme::ColorTheme;
use crate::theme::Theme;
use serde::{Deserialize, Serialize};

/// Configuration for theme-related settings
///
/// `lightTheme` and `darkTheme` carry their own defaults because a
/// configuration written before they existed omits them, and because the dark
/// slot defaults to something other than [`ColorTheme::default`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    /// Default theme preference
    pub default_theme: Theme,
    /// Which of GitHub's themes to paint in light mode
    #[serde(default)]
    pub light_theme: ColorTheme,
    /// Which of GitHub's themes to paint in dark mode
    #[serde(default = "dark_theme_default")]
    pub dark_theme: ColorTheme,
    /// Behavior on app startup: "default" or "last_closed"
    pub on_startup: StartupBehavior,
    /// Behavior when opening a new window: "default" or "last_focused"
    pub on_new_window: NewWindowBehavior,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            default_theme: Theme::default(),
            light_theme: ColorTheme::Light,
            dark_theme: dark_theme_default(),
            on_startup: StartupBehavior::default(),
            on_new_window: NewWindowBehavior::default(),
        }
    }
}

fn dark_theme_default() -> ColorTheme {
    ColorTheme::Dark
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_written_before_the_theme_choice_still_loads() {
        let config: ThemeConfig = serde_json::from_str(
            r#"{"defaultTheme":"dark","onStartup":"default","onNewWindow":"default"}"#,
        )
        .unwrap();
        assert_eq!(config.light_theme, ColorTheme::Light);
        assert_eq!(config.dark_theme, ColorTheme::Dark);
    }
}
