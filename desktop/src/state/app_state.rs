use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;
use std::path::PathBuf;

use super::persistence::LAST_FOCUSED_STATE;
use crate::theme::Theme;

mod sidebar;
mod tabs;

pub use sidebar::Sidebar;
pub use tabs::{Tab, TabContent};

/// Per-window application state.
///
/// # Copy Semantics
///
/// This struct implements `Copy` because all fields are `Signal<T>`, which are cheap to copy
/// (they contain only Arc pointers internally). This allows passing `AppState` to closures
/// and async blocks without explicit `.clone()` calls, making the code cleaner.
///
/// **This aligns with Dioxus design philosophy**: `Signal<T>` is intentionally `Copy` to enable
/// ergonomic state passing in reactive UIs. Wrapping `Signal` fields in a `Copy` struct is the
/// recommended pattern in Dioxus applications.
///
/// # Why Per-field Signals?
///
/// We use per-field `Signal<T>` instead of `Signal<AppState>` for fine-grained reactivity:
/// - Changing `current_theme` doesn't trigger re-renders in components that only watch `tabs`
/// - Different components can update different fields concurrently without conflicts
/// - Components subscribe only to the fields they need (e.g., Header watches theme, TabBar watches tabs)
///
/// If we used `Signal<AppState>`, any field change would trigger re-renders in ALL components
/// that access the state, causing unnecessary performance overhead.
/// Drag state for tab reordering
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabDragState {
    pub dragging_tab_index: Signal<Option<usize>>,
    pub drop_target_index: Signal<Option<usize>>,
    pub animating: Signal<bool>,
}

impl TabDragState {
    pub fn new() -> Self {
        Self {
            dragging_tab_index: Signal::new(None),
            drop_target_index: Signal::new(None),
            animating: Signal::new(false),
        }
    }

    pub fn start_drag(&mut self, index: usize) {
        self.dragging_tab_index.set(Some(index));
        self.drop_target_index.set(None);
        self.animating.set(false);
    }

    pub fn update_drop_target(&mut self, index: Option<usize>) {
        self.drop_target_index.set(index);
    }

    pub fn end_drag(&mut self) {
        self.dragging_tab_index.set(None);
        self.drop_target_index.set(None);
    }

    pub fn trigger_animation(&mut self) {
        self.animating.set(true);
        // Reset after animation duration
        let mut animating = self.animating;
        dioxus::prelude::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            animating.set(false);
        });
    }
}

impl Default for TabDragState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppState {
    pub tabs: Signal<Vec<Tab>>,
    pub active_tab: Signal<usize>,
    pub current_theme: Signal<Theme>,
    pub zoom_level: Signal<f64>,
    pub directory: Signal<Option<PathBuf>>,
    pub sidebar: Signal<Sidebar>,
    pub position: Signal<LogicalPosition<i32>>,
    pub size: Signal<LogicalSize<u32>>,
    pub tab_drag_state: Signal<TabDragState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tabs: Signal::new(vec![Tab::default()]),
            active_tab: Signal::new(0),
            current_theme: Signal::new(LAST_FOCUSED_STATE.read().theme),
            zoom_level: Signal::new(1.0),
            directory: Signal::new(None),
            sidebar: Signal::new(Sidebar::default()),
            position: Signal::new(Default::default()),
            size: Signal::new(Default::default()),
            tab_drag_state: Signal::new(TabDragState::default()),
        }
    }
}

impl AppState {
    /// Set the root directory
    /// Note: The directory is persisted to state file when window closes
    pub fn set_root_directory(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        *self.directory.write() = Some(path.clone());
        self.sidebar.write().expanded_dirs.clear();
        LAST_FOCUSED_STATE.write().directory = Some(path);
    }
}
