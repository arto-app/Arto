use dioxus::desktop::tao::dpi::LogicalPosition;
use dioxus::desktop::window;
use dioxus::prelude::*;

use crate::drag;
use crate::events::{ActiveDragUpdate, ACTIVE_DRAG_UPDATE};
use crate::state::AppState;

/// Debounce delay before creating preview window (prevents flickering)
pub(super) const DETACH_DEBOUNCE_MS: u64 = 50;

/// Handle mouse motion during tab drag (called from DeviceEvent handler)
///
/// Unified architecture: all windows are equal potential targets.
/// DetachState tracks whether cursor is in a tab bar (None) or detached (preview window).
/// target_window_id in GlobalActiveDrag tracks which window the cursor is in.
pub(super) fn handle_drag_mouse_motion(state: AppState) {
    use crate::window;

    // Get current mouse position, normalized to logical coordinates.
    // The raw value is physical pixels on Windows - see cursor_position_to_logical.
    let scale_factor = dioxus::desktop::window().scale_factor();
    let Some((screen_x, screen_y)) =
        crate::utils::screen::get_cursor_logical_position(scale_factor)
    else {
        tracing::debug!("Failed to get mouse position during active drag");
        return;
    };

    let Some(active) = drag::get_active_drag() else {
        return;
    };

    // Two-phase hit testing for stable drag behavior:
    // Phase 1: Find which window cursor is over -> update focus
    // Phase 2: Check if cursor is in focused window's tab bar -> determine drag target

    let exclude_preview = window::get_preview_window_id();
    let is_single_tab = active.source_tab_count == 1;
    let source_window_id = drag::get_dragged_tab().map(|d| d.source_window_id);

    // Phase 1: Window hit test (ignore tab bar, just find window under cursor)
    // Prioritize currently focused window in overlapping areas for stable behavior
    let current_focus = window::main::get_last_focused_window();
    let window_under_cursor =
        drag::find_window_at_point(screen_x, screen_y, current_focus, exclude_preview);

    // Update focus if cursor moved to a different window
    // If no last focused window exists, focus the window under cursor
    if let Some(wid) = window_under_cursor {
        let last_focused = window::main::get_last_focused_window();
        if last_focused.is_none() || last_focused != Some(wid) {
            window::main::focus_window(wid);
        }
    }

    // Phase 2: Check if cursor is in the focused window's tab bar
    let focused_window = window::main::get_last_focused_window();
    let target_window = focused_window.and_then(|wid| {
        // For single-tab, skip source window (no local reordering possible)
        if is_single_tab && source_window_id == Some(wid) {
            return None;
        }
        // Exclude preview window from being a target
        if exclude_preview == Some(wid) {
            return None;
        }
        // Check if cursor is in this window's tab bar
        if drag::is_point_in_window_tab_bar(wid, screen_x, screen_y) {
            Some(wid)
        } else {
            None
        }
    });

    // Handle state transitions based on current state and target
    // Note: Focus is already handled in Phase 1, so no focus calls needed here
    let preview_position = calculate_window_position(&active, screen_x, screen_y);
    let transition = compute_detach_transition(&active, target_window, preview_position, &state);
    let new_detach_state = transition.new_state;
    let new_target_window_id = transition.target_window;

    // Calculate target_index if cursor is in a tab bar
    let new_target_index = new_target_window_id
        .and_then(|wid| drag::calculate_target_index_from_screen(wid, screen_x));

    // Optimization: Check if anything significant changed before broadcasting
    // This reduces unnecessary re-renders during drag
    let state_changed = active.target_window_id != new_target_window_id
        || active.detach_state != new_detach_state
        || new_target_index.is_some_and(|idx| idx != active.target_index)
        || (screen_x - active.screen_x).abs() > 1.0
        || (screen_y - active.screen_y).abs() > 1.0;

    // Update global state
    drag::update_active_drag(|d| {
        d.screen_x = screen_x;
        d.screen_y = screen_y;
        d.target_window_id = new_target_window_id;
        d.detach_state = new_detach_state;
        if let Some(idx) = new_target_index {
            d.target_index = idx;
        }
    });

    // Only notify windows when state actually changed (reduces re-renders)
    if state_changed {
        ACTIVE_DRAG_UPDATE.send(ActiveDragUpdate).ok();
    }
}

/// Handle mouse release during tab drag (called from DeviceEvent handler)
///
/// Unified architecture: drop inserts tab into target_window_id (if set) or commits preview.
///
/// State transitions:
/// - `DetachState::None` -> Tab is in a window's tab bar, insert there
/// - `DetachState::Pending` -> Drag cancelled during debounce, restore tab
/// - `DetachState::Creating` -> Window creation in progress, cancel and restore
/// - `DetachState::Detached` -> Preview visible, commit as new window
pub(crate) fn handle_drag_mouse_release(mut state: AppState) {
    use crate::drag::DetachState;

    let Some(active) = drag::get_active_drag() else {
        return;
    };

    let is_single_tab = active.source_tab_count == 1;

    match &active.detach_state {
        DetachState::None => {
            handle_drop_in_tab_bar(&mut state, &active, is_single_tab);
        }
        DetachState::Pending { .. } => {
            handle_drop_during_pending(&mut state, is_single_tab);
        }
        DetachState::Creating => {
            handle_drop_during_creating(&mut state, is_single_tab);
        }
        DetachState::Detached { .. } => {
            handle_drop_when_detached();
        }
    }

    // Notify all windows to clear drag UI
    ACTIVE_DRAG_UPDATE.send(ActiveDragUpdate).ok();

    // Clean up global drag state
    drag::end_active_drag();
    drag::end_drag();
}

/// Handle drop when cursor is in a window's tab bar (DetachState::None)
fn handle_drop_in_tab_bar(
    state: &mut AppState,
    active: &drag::GlobalActiveDrag,
    is_single_tab: bool,
) {
    let (Some(target_wid), Some(dragged)) = (active.target_window_id, drag::get_dragged_tab())
    else {
        return;
    };

    let current_wid = window().id();

    if target_wid == current_wid {
        // Drop in current window (source)
        if is_single_tab {
            // Single-tab: tab already there, just restore window state
            crate::window::close_preview_window();
        } else {
            // Multi-tab: insert and switch to the tab
            let insert_index = state.insert_tab(dragged.tab, active.target_index);
            state.switch_to_tab(insert_index);
            crate::window::close_preview_window();
        }
    } else {
        // Drop in another window - send transfer request
        crate::events::TRANSFER_TAB_TO_WINDOW
            .send((target_wid, Some(active.target_index), dragged.tab))
            .ok();
        crate::window::main::focus_window(target_wid);

        if is_single_tab {
            // Single-tab: discard preview and close source window
            // (tab was never removed, so closing window cleans up)
            crate::window::discard_preview_window();
            window().close();
        } else {
            // Multi-tab: just close preview window
            crate::window::close_preview_window();
        }
    }
}

/// Handle drop during debounce period (DetachState::Pending)
///
/// Restore tab to source window since no preview was created.
fn handle_drop_during_pending(state: &mut AppState, is_single_tab: bool) {
    if is_single_tab {
        // Single-tab: tab was never removed, nothing to restore
        return;
    }

    // Multi-tab: insert tab back to source position
    if let Some(dragged) = drag::get_dragged_tab() {
        state.insert_tab(dragged.tab, dragged.source_index);
    }
}

/// Handle drop during window creation (DetachState::Creating)
///
/// Cancel the creation and restore tab to source window.
fn handle_drop_during_creating(state: &mut AppState, is_single_tab: bool) {
    crate::window::close_preview_window();

    if !is_single_tab {
        // Multi-tab: insert tab back to source position
        if let Some(dragged) = drag::get_dragged_tab() {
            state.insert_tab(dragged.tab, dragged.source_index);
        }
    }
    // Single-tab: tab already in window, close_preview_window restored window state
}

/// Handle drop when preview window is visible (DetachState::Detached)
///
/// Commit the preview window as a permanent new window.
fn handle_drop_when_detached() {
    if let Some(preview_window_id) = crate::window::commit_preview_window() {
        crate::window::main::focus_window(preview_window_id);
    }
}

/// Calculate window position from screen coordinates and offsets
fn calculate_window_position(
    active: &drag::GlobalActiveDrag,
    screen_x: f64,
    screen_y: f64,
) -> LogicalPosition<i32> {
    let chrome = crate::window::get_chrome_inset();

    // Get tab bar position from source window
    let tab_bar = drag::get_dragged_tab()
        .and_then(|d| crate::components::tab::get_tab_bar_bounds(d.source_window_id))
        .map(|b| crate::window::Offset::new(b.left, b.top))
        .unwrap_or(crate::window::Offset::ZERO);

    // Position window so cursor stays at same relative position within tab
    let grab = &active.grab_offset;
    LogicalPosition::new(
        (screen_x - chrome.x - tab_bar.x - grab.x) as i32,
        (screen_y - chrome.y - tab_bar.y - grab.y) as i32,
    )
}

/// Result of a detach state transition
struct DetachTransition {
    new_state: drag::DetachState,
    target_window: Option<dioxus::desktop::tao::window::WindowId>,
}

/// Handle detach state transitions during drag
///
/// This function encapsulates the state machine logic for transitioning between:
/// - None (cursor in tab bar)
/// - Pending (waiting for debounce)
/// - Creating (preview window being created)
/// - Detached (preview window visible)
fn compute_detach_transition(
    active: &drag::GlobalActiveDrag,
    target_window: Option<dioxus::desktop::tao::window::WindowId>,
    preview_position: LogicalPosition<i32>,
    state: &AppState,
) -> DetachTransition {
    use crate::drag::DetachState;
    use crate::window;
    use std::time::{Duration, Instant};

    let (new_state, new_target) = match (&active.detach_state, target_window) {
        // Any state + cursor in tab bar -> transition to None (with hide/close if needed)
        (DetachState::None | DetachState::Pending { .. }, Some(wid)) => {
            (DetachState::None, Some(wid))
        }
        (DetachState::Creating, Some(wid)) => {
            window::close_preview_window();
            (DetachState::None, Some(wid))
        }
        (DetachState::Detached { .. }, Some(wid)) => {
            window::hide_preview_window();
            (DetachState::None, Some(wid))
        }

        // None + cursor outside -> start pending
        (DetachState::None, None) => (
            DetachState::Pending {
                entered_at: Instant::now(),
            },
            None,
        ),

        // Pending + debounce not elapsed -> stay pending
        (DetachState::Pending { entered_at }, None)
            if entered_at.elapsed() < Duration::from_millis(DETACH_DEBOUNCE_MS) =>
        {
            (active.detach_state.clone(), None)
        }

        // Pending + debounce elapsed -> create or show preview
        (DetachState::Pending { .. }, None) => {
            if window::has_preview_window() {
                window::show_preview_window();
                window::update_preview_position(preview_position);
                window::get_preview_window_id()
                    .map(|id| {
                        (
                            DetachState::Detached {
                                preview_window_id: id,
                            },
                            None,
                        )
                    })
                    .unwrap_or((active.detach_state.clone(), None))
            } else if let Some(dragged) = drag::get_dragged_tab() {
                let win = dioxus::desktop::window();
                spawn_preview_window_creation(PreviewWindowParams {
                    tab: dragged.tab.clone(),
                    position: preview_position,
                    is_single_tab: active.source_tab_count == 1,
                    directory: state.sidebar.read().root_directory.clone(),
                    sidebar: state.sidebar.read().clone(),
                    theme: *state.current_theme.read(),
                    zoom_level: *state.zoom_level.read(),
                    size: win.inner_size().to_logical::<u32>(win.scale_factor()),
                });
                (DetachState::Creating, None)
            } else {
                (active.detach_state.clone(), None)
            }
        }

        // Creating -> wait for preview, update position when ready
        (DetachState::Creating, None) => {
            if let Some(id) = window::get_preview_window_id() {
                window::update_preview_position(preview_position);
                (
                    DetachState::Detached {
                        preview_window_id: id,
                    },
                    None,
                )
            } else {
                (DetachState::Creating, None)
            }
        }

        // Detached -> update position
        (DetachState::Detached { preview_window_id }, None) => {
            window::update_preview_position(preview_position);
            (
                DetachState::Detached {
                    preview_window_id: *preview_window_id,
                },
                None,
            )
        }
    };

    DetachTransition {
        new_state,
        target_window: new_target,
    }
}

/// Parameters for spawning a preview window
struct PreviewWindowParams {
    tab: crate::state::Tab,
    position: LogicalPosition<i32>,
    is_single_tab: bool,
    directory: Option<std::path::PathBuf>,
    sidebar: crate::state::Sidebar,
    theme: crate::theme::Theme,
    zoom_level: f64,
    size: dioxus::desktop::tao::dpi::LogicalSize<u32>,
}

/// Spawn async task to create preview window
///
/// Note: This task is not tracked/cancellable. If drag is cancelled (e.g., via Escape)
/// before completion, the window may still be created. This is safe because:
/// - `cancel_active_drag_on_escape` calls `window::close_preview_window()` which closes
///   any existing preview window regardless of when it was created
/// - The preview window checks drag state on focus events
fn spawn_preview_window_creation(params: PreviewWindowParams) {
    use crate::window::{create_preview_window, CreateMainWindowConfigParams};

    spawn(async move {
        let config = CreateMainWindowConfigParams {
            directory: params.directory,
            sidebar_width: params.sidebar.width,
            sidebar_show_all_files: params.sidebar.show_all_files,
            theme: params.theme,
            zoom_level: params.zoom_level,
            size: params.size,
            ..Default::default()
        };
        create_preview_window(params.tab, params.position, config, params.is_single_tab).await;
    });
}
