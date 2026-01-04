use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::window::Window;

use crate::state::{Position, Size};

use super::types::WindowMetrics;

/// Convert outer window size to inner (content) size using cached chrome inset.
///
/// Uses CHROME_INSET which is set once at first window mount.
/// Falls back to outer size if chrome inset is not yet initialized.
pub fn outer_to_inner_size(outer: LogicalSize<u32>) -> LogicalSize<u32> {
    let chrome = super::get_chrome_inset();
    // chrome.x = left border (usually 0 on macOS)
    // chrome.y = title bar height
    LogicalSize::new(
        outer.width.saturating_sub(chrome.x as u32),
        outer.height.saturating_sub(chrome.y as u32),
    )
}

pub fn capture_window_metrics(window: &Window) -> WindowMetrics {
    let scale = window.scale_factor();
    let position = window
        .outer_position()
        .map(|pos| pos.to_logical::<i32>(scale))
        .unwrap_or_else(|_| LogicalPosition::new(0, 0));
    let outer_size = window.outer_size().to_logical::<u32>(scale);

    // Use cached chrome inset for conversion; fall back to direct inner_size query
    let chrome = super::get_chrome_inset();
    let inner_size = if chrome.y > 0.0 {
        outer_to_inner_size(outer_size)
    } else {
        // Chrome inset not yet initialized; query directly
        window.inner_size().to_logical::<u32>(scale)
    };

    WindowMetrics {
        position: Position {
            x: position.x,
            y: position.y,
        },
        size: Size {
            width: inner_size.width,
            height: inner_size.height,
        },
    }
}
