use dioxus::prelude::*;

use super::calculations::{calculate_floating_tab_left, screen_to_client_x};
use crate::drag;

/// Floating tab component that follows the cursor during drag.
/// Y position is fixed to the tab bar for horizontal-only movement.
///
/// With the unified drag architecture, this component is rendered in the current
/// target window (whichever window's tab bar contains the cursor).
#[component]
pub fn FloatingTab(drag: crate::drag::GlobalActiveDrag, tab_name: String, fixed_y: f64) -> Element {
    use dioxus::desktop::window;

    // Convert screen coordinates to client coordinates for THIS window (target)
    let current_window_id = window().id();
    let win = window();
    let chrome = crate::window::get_chrome_inset();
    let scale = win.scale_factor();
    let client_x = if let Ok(outer) = win.outer_position() {
        screen_to_client_x(drag.screen_x, outer.x as f64, chrome.x, scale)
    } else {
        0.0
    };

    // Position the floating tab so the cursor stays at the same relative position
    let raw_left = client_x - drag.grab_offset.x;

    // Calculate clamped position (use tab bar left as minimum)
    let tab_count = super::tab_bar::get_tab_count(current_window_id);
    let min_left = super::tab_bar::get_tab_bar_bounds(current_window_id)
        .map(|bounds| bounds.left)
        .unwrap_or(0.0);
    let left = calculate_floating_tab_left(raw_left, min_left, tab_count, drag::TAB_WIDTH);

    // Y position is fixed to the tab bar (no vertical movement)
    let top = fixed_y;

    // Width is inherited from .tab CSS class
    let style = format!("left: {}px; top: {}px;", left, top);

    rsx! {
        div {
            class: "tab floating",
            style: "{style}",

            span {
                class: "tab-name",
                "{tab_name}"
            }
        }
    }
}
