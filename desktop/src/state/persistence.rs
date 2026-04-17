use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::config::DEFAULT_RIGHT_SIDEBAR_WIDTH;
use crate::state::{AppState, SidebarPanel, Tab};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct PersistedFileView {
    pub path: PathBuf,
    #[serde(default)]
    pub scroll_position: f64,
}

impl Default for PersistedFileView {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            scroll_position: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PersistedState {
    pub directory: Option<PathBuf>,
    pub recent_files: Vec<PathBuf>,
    pub recent_file_views: Vec<PersistedFileView>,
    pub open_files: Vec<PathBuf>,
    pub open_file_views: Vec<PersistedFileView>,
    pub active_open_file_index: Option<usize>,
    pub theme: Theme,
    pub sidebar_pinned: bool,
    pub sidebar_width: f64,
    pub sidebar_show_all_files: bool,
    #[serde(default = "default_zoom_level")]
    pub sidebar_zoom_level: f64,
    #[serde(default)]
    pub left_sidebar_panel: SidebarPanel,
    pub right_sidebar_pinned: bool,
    pub right_sidebar_width: f64,
    #[serde(default = "default_right_sidebar_panel", alias = "rightSidebarTab")]
    pub right_sidebar_panel: SidebarPanel,
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

fn default_right_sidebar_panel() -> SidebarPanel {
    SidebarPanel::Contents
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            directory: None,
            recent_files: Vec::new(),
            recent_file_views: Vec::new(),
            open_files: Vec::new(),
            open_file_views: Vec::new(),
            active_open_file_index: None,
            theme: Theme::default(),
            sidebar_pinned: false,
            sidebar_width: 280.0,
            sidebar_show_all_files: false,
            sidebar_zoom_level: 1.0,
            left_sidebar_panel: SidebarPanel::Directory,
            right_sidebar_pinned: false,
            right_sidebar_width: DEFAULT_RIGHT_SIDEBAR_WIDTH,
            right_sidebar_panel: default_right_sidebar_panel(),
            right_sidebar_zoom_level: 1.0,
            window_position: Position::default(),
            window_size: Size::default(),
            zoom_level: 1.0,
        }
    }
}

impl From<&AppState> for PersistedState {
    fn from(state: &AppState) -> Self {
        let previous = PersistedState::load();
        let sidebar = state.sidebar.read();
        let tabs = state.tabs.read();
        let active_tab = *state.active_tab.read();
        let open_file_views =
            dedupe_existing_file_views(tabs.iter().enumerate().filter_map(|(index, tab)| {
                let path = tab.file()?.to_path_buf();
                let scroll_position = if index == active_tab {
                    *state.current_scroll_position.read()
                } else {
                    tab.history
                        .current()
                        .map(|entry| entry.scroll_position)
                        .unwrap_or(0.0)
                };
                Some(PersistedFileView {
                    path,
                    scroll_position,
                })
            }));
        let open_files = open_file_views
            .iter()
            .map(|view| view.path.clone())
            .collect();
        let active_open_file_index =
            tabs.get(active_tab)
                .and_then(|tab| tab.file())
                .and_then(|active_file| {
                    open_file_views
                        .iter()
                        .position(|view| view.path.as_path() == active_file)
                });
        let recent_file_views = merge_recent_file_views(
            previous.recent_file_views,
            state.recent_files.read().iter().cloned(),
            &open_file_views,
        );
        let recent_files = recent_file_views
            .iter()
            .map(|view| view.path.clone())
            .collect();
        Self {
            directory: sidebar.root_directory.clone(),
            recent_files,
            recent_file_views,
            open_files,
            open_file_views,
            active_open_file_index,
            theme: *state.current_theme.read(),
            sidebar_pinned: sidebar.pinned,
            sidebar_width: sidebar.width,
            sidebar_show_all_files: sidebar.show_all_files,
            sidebar_zoom_level: sidebar.zoom_level,
            left_sidebar_panel: *state.left_sidebar_panel.read(),
            right_sidebar_pinned: *state.right_sidebar_pinned.read(),
            right_sidebar_width: *state.right_sidebar_width.read(),
            right_sidebar_panel: *state.right_sidebar_panel.read(),
            right_sidebar_zoom_level: *state.right_sidebar_zoom_level.read(),
            window_position: (*state.position.read()).into(),
            window_size: (*state.size.read()).into(),
            zoom_level: *state.zoom_level.read(),
        }
    }
}

fn dedupe_existing_files(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for path in paths {
        if !path.is_file() || !seen.insert(path.clone()) {
            continue;
        }
        result.push(path);
    }

    result
}

fn dedupe_existing_file_views(
    views: impl IntoIterator<Item = PersistedFileView>,
) -> Vec<PersistedFileView> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for view in views {
        if !view.path.is_file() || !seen.insert(view.path.clone()) {
            continue;
        }
        result.push(view);
    }

    result
}

fn merge_recent_file_views(
    previous_views: Vec<PersistedFileView>,
    recent_paths: impl IntoIterator<Item = PathBuf>,
    open_views: &[PersistedFileView],
) -> Vec<PersistedFileView> {
    let previous_map = previous_views
        .into_iter()
        .map(|view| (view.path.clone(), view))
        .collect::<std::collections::HashMap<_, _>>();
    let open_map = open_views
        .iter()
        .cloned()
        .map(|view| (view.path.clone(), view))
        .collect::<std::collections::HashMap<_, _>>();

    dedupe_existing_file_views(
        recent_paths
            .into_iter()
            .chain(open_views.iter().map(|view| view.path.clone()))
            .filter_map(|path| {
                open_map
                    .get(&path)
                    .cloned()
                    .or_else(|| previous_map.get(&path).cloned())
                    .or_else(|| {
                        path.is_file().then_some(PersistedFileView {
                            path,
                            scroll_position: 0.0,
                        })
                    })
            }),
    )
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
            Ok(content) => serde_json::from_str::<Self>(&content)
                .map(Self::normalized)
                .unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn normalized(mut self) -> Self {
        self.open_file_views = if self.open_file_views.is_empty() {
            self.open_files
                .iter()
                .cloned()
                .map(|path| PersistedFileView {
                    path,
                    scroll_position: 0.0,
                })
                .collect()
        } else {
            self.open_file_views
        };
        self.recent_file_views = if self.recent_file_views.is_empty() {
            self.recent_files
                .iter()
                .cloned()
                .map(|path| PersistedFileView {
                    path,
                    scroll_position: 0.0,
                })
                .collect()
        } else {
            self.recent_file_views
        };

        self.open_file_views = dedupe_existing_file_views(self.open_file_views);
        self.recent_file_views = dedupe_existing_file_views(
            self.recent_file_views
                .into_iter()
                .chain(self.open_file_views.iter().cloned()),
        );
        self.open_files = dedupe_existing_files(self.open_files);
        self.open_files = self
            .open_file_views
            .iter()
            .map(|view| view.path.clone())
            .collect();
        self.recent_files = self
            .recent_file_views
            .iter()
            .map(|view| view.path.clone())
            .collect();
        self.active_open_file_index = self
            .active_open_file_index
            .filter(|index| *index < self.open_file_views.len());
        self
    }

    pub fn restored_open_tabs(&self) -> Vec<Tab> {
        self.open_file_views
            .iter()
            .cloned()
            .map(|view| {
                let mut tab = Tab::new(view.path);
                tab.history.save_scroll_position(view.scroll_position);
                tab
            })
            .collect()
    }

    pub fn restored_active_tab(&self) -> usize {
        self.active_open_file_index.unwrap_or(0)
    }

    pub fn recent_file_view(&self, index: usize) -> Option<&PersistedFileView> {
        self.recent_file_views.get(index)
    }

    /// Save persisted state to file
    ///
    /// This function should be called when a window is closing to persist its state.
    pub fn save(&self) {
        let normalized = self.clone().normalized();
        let path = Self::path();

        tracing::debug!(
            path = %path.display(),
            theme = ?normalized.theme,
            sidebar_pinned = normalized.sidebar_pinned,
            sidebar_width = normalized.sidebar_width,
            sidebar_show_all_files = normalized.sidebar_show_all_files,
            sidebar_zoom_level = normalized.sidebar_zoom_level,
            left_sidebar_panel = ?normalized.left_sidebar_panel,
            right_sidebar_pinned = normalized.right_sidebar_pinned,
            right_sidebar_width = normalized.right_sidebar_width,
            right_sidebar_panel = ?normalized.right_sidebar_panel,
            right_sidebar_zoom_level = normalized.right_sidebar_zoom_level,
            recent_files = normalized.recent_files.len(),
            open_files = normalized.open_files.len(),
            zoom_level = normalized.zoom_level,
            "Saving persisted state"
        );

        // Save to file synchronously
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!(?e, "Failed to create session directory");
                return;
            }
        }

        match serde_json::to_string_pretty(&normalized) {
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
            recent_files: vec![PathBuf::from("/tmp/recent.md")],
            recent_file_views: vec![PersistedFileView {
                path: PathBuf::from("/tmp/recent.md"),
                scroll_position: 120.0,
            }],
            open_files: vec![PathBuf::from("/tmp/open.md")],
            open_file_views: vec![PersistedFileView {
                path: PathBuf::from("/tmp/open.md"),
                scroll_position: 240.0,
            }],
            active_open_file_index: Some(0),
            sidebar_pinned: false,
            right_sidebar_pinned: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: PersistedState = serde_json::from_str(&json).unwrap();

        assert!(!parsed.sidebar_pinned);
        assert!(parsed.right_sidebar_pinned);
        assert_eq!(parsed.recent_files, vec![PathBuf::from("/tmp/recent.md")]);
        assert_eq!(parsed.open_files, vec![PathBuf::from("/tmp/open.md")]);
        assert_eq!(parsed.recent_file_views[0].scroll_position, 120.0);
        assert_eq!(parsed.open_file_views[0].scroll_position, 240.0);
        assert_eq!(parsed.active_open_file_index, Some(0));
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

    #[test]
    fn test_normalized_filters_missing_and_duplicate_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let existing = temp_dir.path().join("existing.md");
        std::fs::write(&existing, "# test").unwrap();
        let missing = temp_dir.path().join("missing.md");

        let normalized = PersistedState {
            recent_files: vec![existing.clone(), missing.clone(), existing.clone()],
            recent_file_views: vec![
                PersistedFileView {
                    path: existing.clone(),
                    scroll_position: 10.0,
                },
                PersistedFileView {
                    path: missing.clone(),
                    scroll_position: 20.0,
                },
            ],
            open_files: vec![missing, existing.clone(), existing.clone()],
            open_file_views: vec![
                PersistedFileView {
                    path: existing.clone(),
                    scroll_position: 30.0,
                },
                PersistedFileView {
                    path: existing.clone(),
                    scroll_position: 40.0,
                },
            ],
            active_open_file_index: Some(5),
            ..Default::default()
        }
        .normalized();

        assert_eq!(normalized.recent_files, vec![existing.clone()]);
        assert_eq!(normalized.open_files, vec![existing]);
        assert_eq!(normalized.recent_file_views[0].scroll_position, 10.0);
        assert_eq!(normalized.open_file_views[0].scroll_position, 30.0);
        assert_eq!(normalized.active_open_file_index, None);
    }
}
