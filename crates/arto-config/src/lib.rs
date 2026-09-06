//! Arto's user configuration.
//!
//! The types behind `config.json` (preferences) and `mappings.json`
//! (keybindings), plus where those files live and how to load and save
//! them. Sections that belong to another crate are borrowed from it rather
//! than redefined: rendering options come from `arto-markdown`, the binding
//! set from `arto-keybindings`, so every consumer of the configuration
//! (the desktop app, `arto page`, Quick Look) reads the same definitions.
//!
//! What stays out of this crate is the runtime side: the desktop app holds
//! the live `Config` behind a lock and broadcasts changes to its windows.

use serde::{Deserialize, Serialize};

mod behavior;
mod directory_config;
mod file_open_behavior;
mod persistence;
mod right_sidebar_config;
mod sidebar_config;
mod theme;
mod theme_config;
mod window_dimension;
mod window_position_config;
mod window_size_config;
mod zoom_config;

pub use arto_keybindings::{BindingSet, KeyAction};
pub use arto_markdown::RenderOptions;
pub use behavior::*;
pub use directory_config::*;
pub use file_open_behavior::*;
pub use persistence::*;
pub use right_sidebar_config::*;
pub use sidebar_config::*;
pub use theme::*;
pub use theme_config::*;
pub use window_dimension::*;
pub use window_position_config::*;
pub use window_size_config::*;
pub use zoom_config::*;

/// Global application configuration
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub directory: DirectoryConfig,
    pub file_open: FileOpenBehavior,
    /// Rendering choices, owned by the crate that renders (arto-markdown) so
    /// every consumer of the pipeline shares one definition.
    pub markdown: RenderOptions,
    pub theme: ThemeConfig,
    pub sidebar: SidebarConfig,
    pub right_sidebar: RightSidebarConfig,
    pub window_position: WindowPositionConfig,
    pub window_size: WindowSizeConfig,
    pub zoom: ZoomConfig,
    /// Keybindings live in their own file (`mappings.json`), so they are
    /// neither read from nor written to `config.json`.
    #[serde(skip_serializing, skip_deserializing, default)]
    pub keybindings: BindingSet,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assert_keybindings_empty(set: &BindingSet) {
        assert!(set.global.is_empty());
        assert!(set.content.is_empty());
        assert!(set.sidebar.is_empty());
        assert!(set.quick_access.is_empty());
        assert!(set.right_sidebar.is_empty());
        assert!(set.search.is_empty());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();

        // Theme defaults
        assert_eq!(config.theme.default_theme, Theme::Auto);
        assert_eq!(config.theme.on_startup, StartupBehavior::Default);
        assert_eq!(config.theme.on_new_window, NewWindowBehavior::Default);
        assert_eq!(config.file_open, FileOpenBehavior::LastFocused);

        // Directory defaults
        assert_eq!(config.directory.default_directory, None);
        assert_eq!(config.directory.on_startup, StartupBehavior::Default);
        assert_eq!(config.directory.on_new_window, NewWindowBehavior::Default);

        // Markdown defaults
        assert!(config.markdown.auto_link_urls); // Default is true

        // Sidebar defaults
        assert!(!config.sidebar.default_pinned); // Default is false (unpinned, overlay on hover)
        assert_eq!(config.sidebar.default_width, 280.0);
        assert!(!config.sidebar.default_show_all_files);
        assert_eq!(config.sidebar.default_zoom_level, 1.0);
        assert_eq!(config.sidebar.on_startup, StartupBehavior::Default);
        assert_eq!(config.sidebar.on_new_window, NewWindowBehavior::Default);

        // Right sidebar defaults
        assert!(!config.right_sidebar.default_pinned);
        assert_eq!(config.right_sidebar.default_width, 220.0);
        assert_eq!(config.right_sidebar.default_zoom_level, 1.0);
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

        // Keybindings defaults
        assert_keybindings_empty(&config.keybindings);

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
            file_open: FileOpenBehavior::CurrentScreen,
            markdown: RenderOptions {
                auto_link_urls: true,
            },
            directory: DirectoryConfig {
                default_directory: Some(PathBuf::from("/home/user")),
                on_startup: StartupBehavior::Default,
                on_new_window: NewWindowBehavior::Default,
            },
            sidebar: SidebarConfig {
                default_pinned: false,
                default_width: 320.0,
                default_show_all_files: true,
                default_zoom_level: 1.2,
                on_startup: StartupBehavior::LastClosed,
                on_new_window: NewWindowBehavior::LastFocused,
            },
            right_sidebar: RightSidebarConfig {
                default_pinned: true,
                default_width: 250.0,
                default_tab: Default::default(),
                default_zoom_level: 0.8,
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
            keybindings: BindingSet {
                global: vec![KeyAction {
                    key: "Ctrl+k".to_string(),
                    action: "tab.close".to_string(),
                }],
                ..Default::default()
            },
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert!(!json.contains("\"keybindings\""));

        assert_eq!(parsed.theme.default_theme, Theme::Dark);
        assert_eq!(parsed.theme.on_startup, StartupBehavior::LastClosed);
        assert_eq!(parsed.file_open, FileOpenBehavior::CurrentScreen);
        assert_eq!(
            parsed.directory.default_directory,
            Some(PathBuf::from("/home/user"))
        );
        assert!(!parsed.sidebar.default_pinned);
        assert_eq!(parsed.sidebar.default_width, 320.0);
        assert_eq!(parsed.sidebar.default_zoom_level, 1.2);
        assert!(parsed.right_sidebar.default_pinned);
        assert_eq!(parsed.right_sidebar.default_width, 250.0);
        assert_eq!(parsed.right_sidebar.default_zoom_level, 0.8);
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
        assert_keybindings_empty(&parsed.keybindings);
    }

    #[test]
    fn test_config_without_zoom_section_uses_defaults() {
        // Empty JSON should deserialize to all defaults (including zoom)
        let json = r#"{}"#;
        let parsed: Config = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.zoom.default_zoom_level, 1.0);
        assert_eq!(parsed.zoom.on_startup, StartupBehavior::Default);
        assert_eq!(parsed.zoom.on_new_window, NewWindowBehavior::Default);

        // Markdown defaults (auto_link_urls defaults to true even when section is missing)
        assert!(parsed.markdown.auto_link_urls);

        // Sidebar zoom defaults
        assert_eq!(parsed.sidebar.default_zoom_level, 1.0);
        assert_eq!(parsed.right_sidebar.default_zoom_level, 1.0);
        assert_eq!(parsed.file_open, FileOpenBehavior::LastFocused);
    }

    #[test]
    fn test_config_file_open_deserialization() {
        let last_focused: Config = serde_json::from_str(r#"{"fileOpen":"last_focused"}"#).unwrap();
        assert_eq!(last_focused.file_open, FileOpenBehavior::LastFocused);

        let current_screen: Config =
            serde_json::from_str(r#"{"fileOpen":"current_screen"}"#).unwrap();
        assert_eq!(current_screen.file_open, FileOpenBehavior::CurrentScreen);
    }
}
