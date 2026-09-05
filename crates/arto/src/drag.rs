// Drag module - manages global drag state to prevent conflicts
// between tab dragging and file/directory dragging
//
// Architecture: DeviceEvent-based global mouse tracking
// =====================================================
// Instead of using DOM pointer capture (which breaks when windows are hidden/shown),
// we use tao's DeviceEvent::MouseMotion and DeviceEvent::Button for global mouse tracking.
// This allows drag operations to work reliably across window boundaries.
//
// The global DRAG_STATE stores all drag information, and DeviceEvent handlers in the
// App component read/write this state to coordinate drag operations.

use std::time::Instant;

use dioxus::desktop::tao::window::WindowId;
use parking_lot::RwLock;

use crate::state::Tab;

// ============================================================================
// Constants
// ============================================================================

/// Fixed tab width in pixels (must match CSS .tab { width })
pub const TAB_WIDTH: f64 = 140.0;

/// Extra padding for tab bar hit detection (makes drag less finicky)
pub const TAB_BAR_HIT_PADDING: f64 = 20.0;

use crate::components::tab::get_tab_bar_bounds;
use crate::window::Offset;

// ============================================================================
// Window Hit Testing
// ============================================================================

/// Check if a screen point is inside a window's bounds
///
/// Handles coordinate conversion between logical (mouse) and physical (window) coordinates.
fn is_point_in_window(
    handle: &std::rc::Rc<dioxus::desktop::DesktopService>,
    screen_x: i32,
    screen_y: i32,
) -> bool {
    let Ok(pos) = handle.window.outer_position() else {
        return false;
    };
    let size = handle.window.outer_size();
    let scale = handle.window.scale_factor();

    // Convert physical pixels to logical points
    let logical_x = (pos.x as f64 / scale) as i32;
    let logical_y = (pos.y as f64 / scale) as i32;
    let logical_w = (size.width as f64 / scale) as i32;
    let logical_h = (size.height as f64 / scale) as i32;

    screen_x >= logical_x
        && screen_x < logical_x + logical_w
        && screen_y >= logical_y
        && screen_y < logical_y + logical_h
}

/// Find the window under the given screen coordinates
///
/// If multiple windows overlap, the `current_focus` window takes priority.
/// Use `is_point_in_window_tab_bar` to check if cursor is in the tab bar area.
pub fn find_window_at_point(
    screen_x: f64,
    screen_y: f64,
    current_focus: Option<WindowId>,
    exclude: Option<WindowId>,
) -> Option<WindowId> {
    let windows = crate::window::main::list_visible_main_windows();
    let px = screen_x as i32;
    let py = screen_y as i32;

    // Prioritize currently focused window in overlapping areas
    if let Some(focused_id) = current_focus {
        if exclude != Some(focused_id) {
            if let Some(handle) = windows.iter().find(|w| w.window.id() == focused_id) {
                if is_point_in_window(handle, px, py) {
                    return Some(focused_id);
                }
            }
        }
    }

    // Find any window containing the point
    windows
        .iter()
        .filter(|h| exclude != Some(h.window.id()))
        .find(|h| is_point_in_window(h, px, py))
        .map(|h| h.window.id())
}

/// Convert screen coordinates to client coordinates for a specific window
///
/// Returns `None` if the window cannot be found or position cannot be retrieved.
/// Logs debug information on failure to help diagnose coordinate conversion issues.
fn screen_to_client(window_id: WindowId, screen_x: f64, screen_y: f64) -> Option<(f64, f64)> {
    let windows = crate::window::main::list_visible_main_windows();
    let handle = windows.iter().find(|w| w.window.id() == window_id);

    let Some(handle) = handle else {
        tracing::debug!(?window_id, "screen_to_client: window not found");
        return None;
    };

    let outer_pos = match handle.window.outer_position() {
        Ok(pos) => pos,
        Err(e) => {
            tracing::debug!(
                ?window_id,
                ?e,
                "screen_to_client: failed to get outer position"
            );
            return None;
        }
    };

    let scale = handle.window.scale_factor();
    let chrome = crate::window::get_chrome_inset();

    // outer_pos and chrome are in physical pixels, convert to logical
    // client = screen - inner_logical = screen - (outer + chrome) / scale
    Some((
        screen_x - (outer_pos.x as f64 + chrome.x) / scale,
        screen_y - (outer_pos.y as f64 + chrome.y) / scale,
    ))
}

/// Check if a screen coordinate is within a specific window's tab bar
///
/// Used for two-phase hit testing:
/// 1. First, find which window the cursor is over (for focus)
/// 2. Then, check if cursor is in the focused window's tab bar (for drag target)
pub fn is_point_in_window_tab_bar(window_id: WindowId, screen_x: f64, screen_y: f64) -> bool {
    let Some((client_x, client_y)) = screen_to_client(window_id, screen_x, screen_y) else {
        return false;
    };
    let Some(tab_bar) = get_tab_bar_bounds(window_id) else {
        return false;
    };

    // Expand hit area by padding for easier targeting
    let padding = TAB_BAR_HIT_PADDING;
    client_x >= tab_bar.left - padding
        && client_x <= tab_bar.right + padding
        && client_y >= tab_bar.top - padding
        && client_y <= tab_bar.bottom + padding
}

// ============================================================================
// Tab Count and Target Index Calculation
// ============================================================================

/// Get tab count for a window (delegates to tab module)
pub fn get_tab_count(window_id: WindowId) -> usize {
    crate::components::tab::get_tab_count(window_id)
}

/// Calculate target index from screen coordinates
///
/// Uses the floating tab's visual center to determine where the tab should be inserted.
/// Tab positions are calculated from `padding + index * TAB_WIDTH` since tab width is fixed.
pub fn calculate_target_index_from_screen(window_id: WindowId, screen_x: f64) -> Option<usize> {
    let tab_count = get_tab_count(window_id);
    if tab_count == 0 {
        return Some(0);
    }

    let (client_x, _) = screen_to_client(window_id, screen_x, 0.0)?;
    let tab_bar = get_tab_bar_bounds(window_id)?;

    // Calculate floating tab's visual center (matches FloatingTab rendering)
    let grab_offset_x = get_active_drag().map(|d| d.grab_offset.x).unwrap_or(0.0);
    let logical_center = client_x - grab_offset_x + TAB_WIDTH / 2.0;

    // Convert from client coordinates to tab-bar-relative coordinates
    let relative_center = logical_center - tab_bar.left;

    // Calculate target index from position
    // Tab i is at position: i * TAB_WIDTH (relative to tab bar left)
    let raw_index = (relative_center / TAB_WIDTH).floor() as isize;
    Some(raw_index.clamp(0, tab_count as isize) as usize)
}

// ============================================================================
// Drag State
// ============================================================================

/// Data for a tab being dragged
#[derive(Debug, Clone, PartialEq)]
pub struct DraggedTab {
    /// The tab being dragged
    pub tab: Tab,
    /// Source window ID (for cleanup)
    pub source_window_id: WindowId,
    /// Original tab index in source window
    pub source_index: usize,
}

/// Global drag state (None = no drag, Some = tab drag in progress)
static DRAG_STATE: RwLock<Option<DraggedTab>> = RwLock::new(None);

/// Start a tab drag operation
pub fn start_tab_drag(tab: Tab, source_window_id: WindowId, source_index: usize) {
    *DRAG_STATE.write() = Some(DraggedTab {
        tab,
        source_window_id,
        source_index,
    });
}

/// End current drag operation
pub fn end_drag() {
    DRAG_STATE.write().take();
}

/// Get the currently dragged tab data (if any)
pub fn get_dragged_tab() -> Option<DraggedTab> {
    DRAG_STATE.read().clone()
}

/// Check if tab dragging is active
pub fn is_tab_dragging() -> bool {
    DRAG_STATE.read().is_some()
}

// ============================================================================
// Global Active Drag State (for DeviceEvent-based tracking)
// ============================================================================

/// Tab detachment state during drag
///
/// With the unified drag architecture, all windows are equal potential targets.
/// DetachState only tracks whether cursor is in a tab bar (None) or detached (preview window).
/// The specific target window is tracked via `target_window_id` in GlobalActiveDrag.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum DetachState {
    /// Cursor is in a window's tab bar (target tracked via target_window_id)
    #[default]
    None,
    /// Waiting for debounce before creating preview window
    Pending { entered_at: Instant },
    /// Creating preview window (async operation in progress)
    Creating,
    /// Preview window is visible and following cursor
    Detached { preview_window_id: WindowId },
}

/// Global active drag state
///
/// This contains all information needed to track a drag operation,
/// accessible by DeviceEvent handlers regardless of window focus.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalActiveDrag {
    /// Index of the tab being dragged in source window
    pub source_index: usize,
    /// Current target window (where cursor is in tab bar, None if detached)
    pub target_window_id: Option<WindowId>,
    /// Current target insertion position in target window's tab bar
    pub target_index: usize,
    /// Current screen X position
    pub screen_x: f64,
    /// Current screen Y position
    pub screen_y: f64,
    /// Mouse grab offset within the tab element (where user clicked)
    pub grab_offset: Offset,
    /// Detachment state (None = in tab bar, Detached = preview window visible)
    pub detach_state: DetachState,
    /// Tab count in source window (for single-tab detection)
    pub source_tab_count: usize,
}

/// Global active drag state
static ACTIVE_DRAG: RwLock<Option<GlobalActiveDrag>> = RwLock::new(None);

/// Start active drag (called when threshold is exceeded)
pub fn start_active_drag(drag: GlobalActiveDrag) {
    *ACTIVE_DRAG.write() = Some(drag);
}

/// Get the current active drag state
pub fn get_active_drag() -> Option<GlobalActiveDrag> {
    ACTIVE_DRAG.read().clone()
}

/// Update the active drag state (called during mouse move)
pub fn update_active_drag<F>(f: F)
where
    F: FnOnce(&mut GlobalActiveDrag),
{
    if let Some(ref mut drag) = *ACTIVE_DRAG.write() {
        f(drag);
    }
}

/// End active drag and return the final state
pub fn end_active_drag() -> Option<GlobalActiveDrag> {
    ACTIVE_DRAG.write().take()
}

/// Check if active drag is in progress
pub fn is_active_drag() -> bool {
    ACTIVE_DRAG.read().is_some()
}
