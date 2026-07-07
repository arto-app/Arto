use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::components::right_sidebar::RightSidebarTab;
use crate::config::DEFAULT_RIGHT_SIDEBAR_WIDTH;
use crate::state::AppState;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl From<LogicalPosition<i32>> for Position {
    fn from(from: LogicalPosition<i32>) -> Self {
        Self {
            x: from.x,
            y: from.y,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl From<LogicalSize<u32>> for Size {
    fn from(from: LogicalSize<u32>) -> Self {
        Self {
            width: from.width,
            height: from.height,
        }
    }
}

/// Persisted state from the last closed window
///
/// This is a subset of AppState that gets saved to session.json
/// when a window closes and loaded on app startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PersistedState {
    pub directory: Option<PathBuf>,
    pub theme: Theme,
    pub content_full_width: bool,
    pub sidebar_pinned: bool,
    pub sidebar_width: f64,
    pub sidebar_show_all_files: bool,
    #[serde(default = "default_zoom_level")]
    pub sidebar_zoom_level: f64,
    pub right_sidebar_pinned: bool,
    pub right_sidebar_width: f64,
    pub right_sidebar_tab: RightSidebarTab,
    #[serde(default = "default_zoom_level")]
    pub right_sidebar_zoom_level: f64,
    pub window_position: Position,
    pub window_size: Size,
    #[serde(default = "default_zoom_level")]
    pub zoom_level: f64,
}

fn default_zoom_level() -> f64 {
    1.0
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            directory: None,
            theme: Theme::default(),
            content_full_width: false,
            sidebar_pinned: false,
            sidebar_width: 280.0,
            sidebar_show_all_files: false,
            sidebar_zoom_level: 1.0,
            right_sidebar_pinned: false,
            right_sidebar_width: DEFAULT_RIGHT_SIDEBAR_WIDTH,
            right_sidebar_tab: RightSidebarTab::default(),
            right_sidebar_zoom_level: 1.0,
            window_position: Position::default(),
            window_size: Size::default(),
            zoom_level: 1.0,
        }
    }
}

impl From<&AppState> for PersistedState {
    fn from(state: &AppState) -> Self {
        let sidebar = state.sidebar.read();
        Self {
            directory: sidebar.root_directory.clone(),
            theme: *state.current_theme.read(),
            content_full_width: *state.content_full_width.read(),
            sidebar_pinned: sidebar.pinned,
            sidebar_width: sidebar.width,
            sidebar_show_all_files: sidebar.show_all_files,
            sidebar_zoom_level: sidebar.zoom_level,
            right_sidebar_pinned: *state.right_sidebar_pinned.read(),
            right_sidebar_width: *state.right_sidebar_width.read(),
            right_sidebar_tab: *state.right_sidebar_tab.read(),
            right_sidebar_zoom_level: *state.right_sidebar_zoom_level.read(),
            window_position: (*state.position.read()).into(),
            window_size: (*state.size.read()).into(),
            zoom_level: *state.zoom_level.read(),
        }
    }
}

impl PersistedState {
    /// Get the state file path (state.json in local data directory)
    pub fn path() -> PathBuf {
        const FILENAME: &str = "state.json";
        if let Some(mut path) = dirs::data_local_dir() {
            path.push("arto");
            path.push(FILENAME);
            return path;
        }

        // Fallback to home directory
        if let Some(mut path) = dirs::home_dir() {
            path.push(".arto");
            path.push(FILENAME);
            return path;
        }

        PathBuf::from(FILENAME)
    }

    /// Load persisted state from file or return default
    pub fn load() -> Self {
        let path = Self::path();

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save persisted state to file
    ///
    /// This function should be called when a window is closing to persist its state.
    pub fn save(&self) {
        let path = Self::path();

        tracing::debug!(
            path = %path.display(),
            theme = ?self.theme,
            content_full_width = self.content_full_width,
            sidebar_pinned = self.sidebar_pinned,
            sidebar_width = self.sidebar_width,
            sidebar_show_all_files = self.sidebar_show_all_files,
            sidebar_zoom_level = self.sidebar_zoom_level,
            right_sidebar_pinned = self.right_sidebar_pinned,
            right_sidebar_width = self.right_sidebar_width,
            right_sidebar_tab = ?self.right_sidebar_tab,
            right_sidebar_zoom_level = self.right_sidebar_zoom_level,
            zoom_level = self.zoom_level,
            "Saving persisted state"
        );

        // Save to file synchronously
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!(?e, "Failed to create session directory");
                return;
            }
        }

        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::error!(?e, "Failed to save persisted state");
                }
            }
            Err(e) => {
                tracing::error!(?e, "Failed to serialize persisted state");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_default_sidebar_state() {
        let state = PersistedState::default();

        // Sidebars default to unpinned (overlay/hover mode)
        assert!(!state.sidebar_pinned);
        assert!(!state.right_sidebar_pinned);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let state = PersistedState {
            sidebar_pinned: false,
            right_sidebar_pinned: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: PersistedState = serde_json::from_str(&json).unwrap();

        assert!(!parsed.sidebar_pinned);
        assert!(parsed.right_sidebar_pinned);
    }

    #[test]
    fn test_deserialize_missing_fields_uses_defaults() {
        // Simulates loading an old state.json that lacks sidebar pinned fields.
        // #[serde(default)] on the struct fills missing fields from PersistedState::default().
        let json = indoc! {r#"
            {
                "theme": "auto",
                "sidebarWidth": 300.0
            }
        "#};

        let parsed: PersistedState = serde_json::from_str(json).unwrap();

        assert!(!parsed.sidebar_pinned);
        assert!(!parsed.right_sidebar_pinned);
        assert_eq!(parsed.sidebar_width, 300.0);
    }

    #[test]
    fn test_deserialize_only_left_pinned() {
        // Only left sidebar pinned; right sidebar fields missing → defaults
        let json = indoc! {r#"
            {
                "sidebarPinned": true
            }
        "#};

        let parsed: PersistedState = serde_json::from_str(json).unwrap();

        assert!(parsed.sidebar_pinned);
        // Right sidebar: defaults (pinned=false)
        assert!(!parsed.right_sidebar_pinned);
    }

    #[test]
    fn test_serialization_roundtrip_pinned_true() {
        let state = PersistedState {
            sidebar_pinned: true,
            right_sidebar_pinned: true,
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: PersistedState = serde_json::from_str(&json).unwrap();

        assert!(parsed.sidebar_pinned);
        assert!(parsed.right_sidebar_pinned);
    }

    #[test]
    fn test_content_full_width_roundtrip() {
        let state = PersistedState {
            content_full_width: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: PersistedState = serde_json::from_str(&json).unwrap();

        assert!(parsed.content_full_width);
    }

    #[test]
    fn test_deserialize_explicit_false_preserved() {
        // User unpinned sidebar via Cmd+B → sidebarPinned: false persisted
        let json = indoc! {r#"
            {
                "sidebarPinned": false,
                "rightSidebarPinned": false
            }
        "#};

        let parsed: PersistedState = serde_json::from_str(json).unwrap();

        assert!(!parsed.sidebar_pinned);
        assert!(!parsed.right_sidebar_pinned);
    }
}
