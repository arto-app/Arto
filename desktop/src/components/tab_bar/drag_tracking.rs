use dioxus::desktop::tao::window::WindowId;
use parking_lot::RwLock;
use std::sync::LazyLock;

/// Information about the tab currently being dragged
#[derive(Debug, Clone)]
pub struct DraggedTab {
    pub source_window_id: WindowId,
    pub source_tab_index: usize,
    pub offset_x: f64,                      // Mouse offset from tab's left edge
    pub offset_y: f64,                      // Mouse offset from tab's top edge
    pub target_window_id: Option<WindowId>, // Target window for cross-window transfer
}

/// Global state: currently dragging tab
///
/// Note: Uses RwLock (same pattern as existing LAST_FOCUSED_STATE)
/// - High read frequency (App.rs ondragover frequently checks)
/// - Low write frequency (ondragstart/ondragend only)
pub static DRAGGED_TAB: LazyLock<RwLock<Option<DraggedTab>>> = LazyLock::new(|| RwLock::new(None));

pub fn start_tab_drag(window_id: WindowId, tab_index: usize, offset_x: f64, offset_y: f64) {
    *DRAGGED_TAB.write() = Some(DraggedTab {
        source_window_id: window_id,
        source_tab_index: tab_index,
        offset_x,
        offset_y,
        target_window_id: None,
    });
}

pub fn end_tab_drag() {
    *DRAGGED_TAB.write() = None;
}

pub fn is_tab_dragging() -> bool {
    DRAGGED_TAB.read().is_some()
}

pub fn get_dragged_tab() -> Option<DraggedTab> {
    DRAGGED_TAB.read().clone()
}

/// Set the target window ID for cross-window tab transfer
pub fn set_target_window(window_id: WindowId) {
    if let Some(ref mut dragged) = *DRAGGED_TAB.write() {
        dragged.target_window_id = Some(window_id);
    }
}

/// Clear the target window ID (used when drag leaves a window)
pub fn clear_target_window() {
    if let Some(ref mut dragged) = *DRAGGED_TAB.write() {
        dragged.target_window_id = None;
    }
}
