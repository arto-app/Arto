use serde::{Deserialize, Serialize};

mod behavior;
mod directory_config;
mod keybindings_config;
mod right_sidebar_config;
mod sidebar_config;
mod theme_config;
mod window_dimension;
mod window_position_config;
mod window_size_config;
mod zoom_config;

pub use behavior::{NewWindowBehavior, StartupBehavior};
pub use directory_config::DirectoryConfig;
pub use keybindings_config::*;
pub use right_sidebar_config::{RightSidebarConfig, DEFAULT_RIGHT_SIDEBAR_WIDTH};
pub use sidebar_config::SidebarConfig;
pub use theme_config::ThemeConfig;
pub use window_dimension::{WindowDimension, WindowDimensionUnit};
pub use window_position_config::{
    WindowPosition, WindowPositionConfig, WindowPositionMode, WindowPositionOffset,
};
pub use window_size_config::{WindowSize, WindowSizeConfig};
pub use zoom_config::ZoomConfig;

/// Global application configuration
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub directory: DirectoryConfig,
    pub theme: ThemeConfig,
    pub sidebar: SidebarConfig,
    pub right_sidebar: RightSidebarConfig,
    pub window_position: WindowPositionConfig,
    pub window_size: WindowSizeConfig,
    pub zoom: ZoomConfig,
    pub keybindings: KeybindingsConfig,
}

#[cfg(test)]
mod tests {
    use super::window_position_config::WindowPositionOffset;
    use super::*;
    use crate::theme::Theme;
    use std::path::PathBuf;

    #[test]
    fn test_config_default() {
        let config = Config::default();

        // Theme defaults
        assert_eq!(config.theme.default_theme, Theme::Auto);
        assert_eq!(config.theme.on_startup, StartupBehavior::Default);
        assert_eq!(config.theme.on_new_window, NewWindowBehavior::Default);

        // Directory defaults
        assert_eq!(config.directory.default_directory, None);
        assert_eq!(config.directory.on_startup, StartupBehavior::Default);
        assert_eq!(config.directory.on_new_window, NewWindowBehavior::Default);

        // Sidebar defaults
        assert!(!config.sidebar.default_open); // Default is false
        assert_eq!(config.sidebar.default_width, 280.0);
        assert!(!config.sidebar.default_show_all_files);
        assert_eq!(config.sidebar.on_startup, StartupBehavior::Default);
        assert_eq!(config.sidebar.on_new_window, NewWindowBehavior::Default);

        // Right sidebar defaults
        assert!(!config.right_sidebar.default_open);
        assert_eq!(config.right_sidebar.default_width, 220.0);
        assert_eq!(config.right_sidebar.on_startup, StartupBehavior::Default);
        assert_eq!(
            config.right_sidebar.on_new_window,
            NewWindowBehavior::Default
        );

        // Window size defaults
        assert_eq!(config.window_size.default_size.width.value, 1000.0);
        assert_eq!(
            config.window_size.default_size.width.unit,
            WindowDimensionUnit::Pixels
        );
        assert_eq!(config.window_size.default_size.height.value, 800.0);
        assert_eq!(
            config.window_size.default_size.height.unit,
            WindowDimensionUnit::Pixels
        );
        assert_eq!(config.window_size.on_startup, StartupBehavior::Default);
        assert_eq!(config.window_size.on_new_window, NewWindowBehavior::Default);

        // Zoom defaults
        assert_eq!(config.zoom.default_zoom_level, 1.0);
        assert_eq!(config.zoom.on_startup, StartupBehavior::Default);
        assert_eq!(config.zoom.on_new_window, NewWindowBehavior::Default);

        // Window position defaults
        assert_eq!(
            config.window_position.default_position_mode,
            WindowPositionMode::Coordinates
        );
        assert_eq!(config.window_position.position_offset.x, 20);
        assert_eq!(config.window_position.position_offset.y, 20);
        assert_eq!(config.window_position.default_position.x.value, 50.0);
        assert_eq!(
            config.window_position.default_position.x.unit,
            WindowDimensionUnit::Percent
        );
        assert_eq!(config.window_position.default_position.y.value, 50.0);
        assert_eq!(
            config.window_position.default_position.y.unit,
            WindowDimensionUnit::Percent
        );
        assert_eq!(config.window_position.on_startup, StartupBehavior::Default);
        assert_eq!(
            config.window_position.on_new_window,
            NewWindowBehavior::Default
        );
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = Config {
            theme: ThemeConfig {
                default_theme: Theme::Dark,
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            directory: DirectoryConfig {
                default_directory: Some(PathBuf::from("/home/user")),
                on_startup: StartupBehavior::Default,
                on_new_window: NewWindowBehavior::Default,
            },
            sidebar: SidebarConfig {
                default_open: false,
                default_width: 320.0,
                default_show_all_files: true,
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            right_sidebar: RightSidebarConfig {
                default_open: true,
                default_width: 250.0,
                default_tab: Default::default(),
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            window_position: WindowPositionConfig {
                default_position: WindowPosition {
                    x: WindowDimension {
                        value: 10.0,
                        unit: WindowDimensionUnit::Percent,
                    },
                    y: WindowDimension {
                        value: 15.0,
                        unit: WindowDimensionUnit::Percent,
                    },
                },
                default_position_mode: WindowPositionMode::Mouse,
                position_offset: WindowPositionOffset { x: 24, y: 12 },
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            window_size: WindowSizeConfig {
                default_size: WindowSize {
                    width: WindowDimension {
                        value: 1200.0,
                        unit: WindowDimensionUnit::Pixels,
                    },
                    height: WindowDimension {
                        value: 85.0,
                        unit: WindowDimensionUnit::Percent,
                    },
                },
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            zoom: ZoomConfig {
                default_zoom_level: 1.5,
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            keybindings: KeybindingsConfig::default(),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.theme.default_theme, Theme::Dark);
        assert_eq!(parsed.theme.on_startup, StartupBehavior::LastClosed);
        assert_eq!(
            parsed.directory.default_directory,
            Some(PathBuf::from("/home/user"))
        );
        assert!(!parsed.sidebar.default_open);
        assert_eq!(parsed.sidebar.default_width, 320.0);
        assert!(parsed.right_sidebar.default_open);
        assert_eq!(parsed.right_sidebar.default_width, 250.0);
        assert_eq!(parsed.window_position.default_position.x.value, 10.0);
        assert_eq!(
            parsed.window_position.default_position.x.unit,
            WindowDimensionUnit::Percent
        );
        assert_eq!(
            parsed.window_position.default_position_mode,
            WindowPositionMode::Mouse
        );
        assert_eq!(parsed.window_position.position_offset.x, 24);
        assert_eq!(parsed.window_size.default_size.width.value, 1200.0);
        assert_eq!(
            parsed.window_size.default_size.width.unit,
            WindowDimensionUnit::Pixels
        );
        assert_eq!(parsed.zoom.default_zoom_level, 1.5);
        assert_eq!(parsed.zoom.on_startup, StartupBehavior::LastClosed);
        assert_eq!(parsed.zoom.on_new_window, NewWindowBehavior::LastFocused);
    }

    #[test]
    fn test_config_without_zoom_section_uses_defaults() {
        // Empty JSON should deserialize to all defaults (including zoom)
        let json = r#"{}"#;
        let parsed: Config = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.zoom.default_zoom_level, 1.0);
        assert_eq!(parsed.zoom.on_startup, StartupBehavior::Default);
        assert_eq!(parsed.zoom.on_new_window, NewWindowBehavior::Default);
    }
}
