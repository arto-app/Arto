use dioxus::desktop::tao::window::WindowId;
use parking_lot::RwLock;
use std::sync::LazyLock;

/// Information about the tab currently being dragged
#[derive(Debug, Clone)]
pub struct DraggedTab {
    pub source_window_id: WindowId,
    pub source_tab_index: usize,
    pub offset_x: f64, // Mouse offset from tab's left edge
    pub offset_y: f64, // Mouse offset from tab's top edge
}

/// Global state: currently dragging tab
///
/// Note: Uses RwLock (same pattern as existing LAST_FOCUSED_STATE)
/// - High read frequency (App.rs ondragover frequently checks)
/// - Low write frequency (ondragstart/ondragend only)
pub static DRAGGED_TAB: LazyLock<RwLock<Option<DraggedTab>>> = LazyLock::new(|| RwLock::new(None));

/// Global flag: true if the tab was dropped in-window (handled by app.rs ondrop)
/// This is set by ondrop and checked by ondragend
pub static DROPPED_IN_WINDOW: LazyLock<RwLock<bool>> = LazyLock::new(|| RwLock::new(false));

pub fn start_tab_drag(window_id: WindowId, tab_index: usize, offset_x: f64, offset_y: f64) {
    *DRAGGED_TAB.write() = Some(DraggedTab {
        source_window_id: window_id,
        source_tab_index: tab_index,
        offset_x,
        offset_y,
    });
}

pub fn end_tab_drag() {
    *DRAGGED_TAB.write() = None;
    *DROPPED_IN_WINDOW.write() = false;
}

pub fn mark_dropped_in_window() {
    *DROPPED_IN_WINDOW.write() = true;
}

pub fn was_dropped_in_window() -> bool {
    *DROPPED_IN_WINDOW.read()
}

pub fn is_tab_dragging() -> bool {
    DRAGGED_TAB.read().is_some()
}

pub fn get_dragged_tab() -> Option<DraggedTab> {
    DRAGGED_TAB.read().clone()
}
