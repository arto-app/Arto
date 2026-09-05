//! Event propagation system for multi-window coordination.
//!
//! This module provides broadcast channels for cross-window communication:
//! - Tab transfers (drag-and-drop, context menu "Move to Window")
//! - Drag state updates (visual feedback across windows)
//! - Cross-window file/directory opening (context menu "Open in Window")

use crate::state::Tab;
use dioxus::desktop::tao::window::WindowId;
use std::path::PathBuf;
use tokio::sync::broadcast;

// ============================================================================
// Tab Transfer Events (for Drag-and-Drop and Context Menu)
// ============================================================================

/// Transfer a tab to a specific window (used by drag-and-drop and context menu "Move to Window")
///
/// Tuple: (target_window_id, target_index, tab)
/// - target_window_id: The window that will receive the tab
/// - target_index: Position in the tab bar (None = append at end)
/// - tab: The tab to transfer (with full history preserved)
pub static TRANSFER_TAB_TO_WINDOW: std::sync::LazyLock<
    broadcast::Sender<(WindowId, Option<usize>, Tab)>,
> = std::sync::LazyLock::new(|| broadcast::channel(10).0);

// ============================================================================
// Unified Drag State Updates (for UI re-render)
// ============================================================================

/// Notification that drag state has changed.
///
/// Sent from DeviceEvent handlers in App when drag state is updated.
/// All windows subscribe to trigger re-render for visual feedback.
/// Windows read the actual state from `drag::get_active_drag()` to determine
/// if they are the current target.
#[derive(Debug, Clone, Copy)]
pub struct ActiveDragUpdate;

/// Global broadcast sender for drag updates.
///
/// Used to notify all windows to re-render when drag state changes.
/// Each window checks if it's the target and shows floating tab + shift indicators.
/// This bridges the gap between DeviceEvent handlers (global) and Dioxus reactivity.
pub static ACTIVE_DRAG_UPDATE: std::sync::LazyLock<broadcast::Sender<ActiveDragUpdate>> =
    std::sync::LazyLock::new(|| broadcast::channel(100).0);

// ============================================================================
// Cross-Window File/Directory Open Events (via Context Menu)
// ============================================================================

/// Open a file in a specific window (used by sidebar context menu "Open in Window")
///
/// Unlike FILE_OPEN_BROADCAST which is handled by the focused window,
/// this event targets a specific window by its WindowId.
pub static OPEN_FILE_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<(WindowId, PathBuf)>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

/// Open a directory in a specific window (used by sidebar context menu "Open in Window")
///
/// Unlike DIRECTORY_OPEN_BROADCAST which affects all windows,
/// this event targets a specific window by its WindowId.
pub static OPEN_DIRECTORY_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<(WindowId, PathBuf)>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);
