use dioxus::desktop::tao::window::WindowId;
use parking_lot::RwLock;
use std::sync::LazyLock;

/// Information about the tab currently being dragged
#[derive(Debug, Clone)]
pub struct DraggedTab {
    pub source_window_id: WindowId,
    pub source_tab_index: usize,
}

/// Global state: currently dragging tab
///
/// Note: Uses RwLock (same pattern as existing LAST_FOCUSED_STATE)
/// - High read frequency (App.rs ondragover frequently checks)
/// - Low write frequency (ondragstart/ondragend only)
pub static DRAGGED_TAB: LazyLock<RwLock<Option<DraggedTab>>> = LazyLock::new(|| RwLock::new(None));

pub fn start_tab_drag(window_id: WindowId, tab_index: usize) {
    *DRAGGED_TAB.write() = Some(DraggedTab {
        source_window_id: window_id,
        source_tab_index: tab_index,
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
