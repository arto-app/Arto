pub mod child;
pub mod index;
pub mod main;
pub mod metrics;
pub mod preview;
pub mod settings;
mod types;

use std::sync::OnceLock;

// ============================================================================
// Offset type
// ============================================================================

/// A 2D offset representing x and y coordinates
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
}

impl Offset {
    pub const ZERO: Offset = Offset { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// ============================================================================
// Chrome Inset (window frame offset)
// ============================================================================

/// Window chrome inset (title bar height, window borders)
///
/// This is the offset from outer window position to inner (client) position.
/// Determined by the OS and set once on first window mount.
static CHROME_INSET: OnceLock<Offset> = OnceLock::new();

/// Set the chrome inset (only first call takes effect)
pub fn set_chrome_inset(x: f64, y: f64) {
    let _ = CHROME_INSET.set(Offset::new(x, y));
}

/// Get the chrome inset (returns Offset::ZERO if not yet set)
pub fn get_chrome_inset() -> Offset {
    CHROME_INSET.get().copied().unwrap_or(Offset::ZERO)
}

pub use child::{
    close_child_windows_for_last_focused, close_child_windows_for_parent,
    open_or_focus_mermaid_window,
};
pub use main::{
    clear_last_focused_if_matches, close_all_main_windows, create_main_window_config,
    create_new_main_window_with_empty, create_new_main_window_with_file,
    focus_last_focused_main_window, has_any_main_windows, register_main_window,
    update_last_focused_window, CreateMainWindowConfigParams,
};
pub use preview::{
    close_preview_window, commit_preview_window, create_preview_window, discard_preview_window,
    get_preview_window_id, has_preview_window, hide_preview_window, show_preview_window,
    update_preview_position,
};
