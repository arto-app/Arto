// Preview window management for tab detachment
//
// When a tab is dragged out of the tab bar, a preview window appears at the cursor.
// The preview window follows the cursor and is either committed (on drop) or
// closed (if the cursor returns to the tab bar or Escape is pressed).

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::desktop::tao::dpi::LogicalPosition;
use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::{window, DesktopService};

use super::main::{create_new_main_window, CreateMainWindowConfigParams};
use crate::state::Tab;

// Thread-local state for the preview window
//
// Using thread-local because:
// 1. Window operations must happen on the main thread
// 2. Only one preview window can exist at a time (per source window)
// 3. Need to track ownership for cleanup
thread_local! {
    static PREVIEW_STATE: RefCell<Option<PreviewState>> = const { RefCell::new(None) };
}

/// Preview window state
struct PreviewState {
    /// Handle to the preview window
    window_handle: Rc<DesktopService>,
    /// Whether the preview was created by promoting current window
    /// (true = single-tab window became preview, false = new window created)
    is_promoted: bool,
}

/// Create a preview window at the specified position
///
/// For single-tab windows, the current window becomes the preview.
/// For multi-tab windows, a new window is created.
///
/// Returns the WindowId of the preview window.
pub async fn create_preview_window(
    tab: Tab,
    position: LogicalPosition<i32>,
    params: CreateMainWindowConfigParams,
    is_single_tab: bool,
) -> WindowId {
    if is_single_tab {
        // Single-tab: promote current window to preview
        promote_current_window_to_preview(position)
    } else {
        // Multi-tab: create new window
        create_new_preview_window(tab, position, params).await
    }
}

/// Promote the current window to become the preview window
///
/// Used for single-tab windows: instead of creating a new window,
/// we move the existing window to follow the cursor.
fn promote_current_window_to_preview(position: LogicalPosition<i32>) -> WindowId {
    let ctx = window();
    let window_id = ctx.window.id();

    // Move window to cursor position
    ctx.window.set_outer_position(position);

    // Set always on top so preview stays visible during drag
    ctx.window.set_always_on_top(true);

    // Store state
    PREVIEW_STATE.with(|state| {
        *state.borrow_mut() = Some(PreviewState {
            window_handle: ctx,
            is_promoted: true,
        });
    });

    window_id
}

/// Create a new preview window
async fn create_new_preview_window(
    tab: Tab,
    position: LogicalPosition<i32>,
    mut params: CreateMainWindowConfigParams,
) -> WindowId {
    // Override position with cursor position
    params.position = position;
    // Skip position shifting - preview window must be at exact cursor-relative position
    params.skip_position_shift = true;

    // Create the window and get handle directly
    let window_handle = create_new_main_window(tab, params).await;
    let window_id = window_handle.window.id();

    // Set always on top so preview stays visible during drag
    window_handle.window.set_always_on_top(true);

    // Store in PREVIEW_STATE for position updates
    PREVIEW_STATE.with(|state| {
        *state.borrow_mut() = Some(PreviewState {
            window_handle,
            is_promoted: false,
        });
    });

    window_id
}

/// Update the preview window position
///
/// Called during drag to follow the cursor.
pub fn update_preview_position(position: LogicalPosition<i32>) {
    PREVIEW_STATE.with(|state| {
        if let Some(ref preview) = *state.borrow() {
            preview.window_handle.window.set_outer_position(position);
        }
    });
}

/// Close the preview window without committing
///
/// Called when:
/// - Cursor returns to valid zone (original tab bar)
/// - Escape key is pressed
/// - Drag is cancelled
pub fn close_preview_window() {
    PREVIEW_STATE.with(|state| {
        if let Some(preview) = state.borrow_mut().take() {
            if preview.is_promoted {
                // For promoted windows, restore normal state
                preview.window_handle.window.set_always_on_top(false);
                preview.window_handle.window.set_visible(true);
            } else {
                // For new windows, close them
                preview.window_handle.close();
            }
        }
    });
}

/// Discard the preview window without restoring visibility
///
/// Called when a promoted window's tab is being transferred to another window.
/// Unlike `close_preview_window()`, this does NOT restore visibility for promoted
/// windows, avoiding a visual flash before the window is closed.
///
/// For non-promoted windows, this behaves the same as `close_preview_window()`.
pub fn discard_preview_window() {
    PREVIEW_STATE.with(|state| {
        if let Some(preview) = state.borrow_mut().take() {
            if preview.is_promoted {
                // For promoted windows, just clear always_on_top
                // Do NOT set_visible(true) - the window will be closed by caller
                preview.window_handle.window.set_always_on_top(false);
            } else {
                // For new windows, close them
                preview.window_handle.close();
            }
        }
    });
}

/// Commit the preview window
///
/// Called when user releases mouse in a valid drop zone.
/// For promoted windows, this means keeping the window as-is.
/// For new windows, this means the window is now a permanent window.
///
/// Returns the WindowId if there was an active preview.
pub fn commit_preview_window() -> Option<WindowId> {
    PREVIEW_STATE.with(|state| {
        state.borrow_mut().take().map(|preview| {
            // Window is now permanent - clear always_on_top
            preview.window_handle.window.set_always_on_top(false);
            preview.window_handle.window.id()
        })
    })
}

/// Check if a preview window is currently active
pub fn has_preview_window() -> bool {
    PREVIEW_STATE.with(|state| state.borrow().is_some())
}

/// Hide the preview window temporarily (for cross-window drag)
///
/// The preview window is hidden but not closed. It can be shown again
/// by calling show_preview_window() or closed with close_preview_window().
pub fn hide_preview_window() {
    PREVIEW_STATE.with(|state| {
        if let Some(ref preview) = *state.borrow() {
            preview.window_handle.window.set_visible(false);
        }
    });
}

/// Show the preview window (after hiding for cross-window drag)
pub fn show_preview_window() {
    PREVIEW_STATE.with(|state| {
        if let Some(ref preview) = *state.borrow() {
            preview.window_handle.window.set_visible(true);
        }
    });
}

/// Get the preview window ID (for hit testing exclusion)
///
/// Returns the WindowId of the active preview window, if any.
/// This is used to exclude the preview window from `find_window_at_point`
/// during drag, since the preview window follows the cursor and would
/// otherwise block hit testing of the actual target window.
pub fn get_preview_window_id() -> Option<WindowId> {
    PREVIEW_STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|preview| preview.window_handle.window.id())
    })
}

/// Check if the preview window was created by promoting the current window
///
/// Returns true if the current window was promoted to become the preview,
/// false if a new window was created or there's no preview.
#[allow(dead_code)] // Reserved for future drag-drop preview behavior
pub fn is_preview_promoted() -> bool {
    PREVIEW_STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|preview| preview.is_promoted)
            .unwrap_or(false)
    })
}
