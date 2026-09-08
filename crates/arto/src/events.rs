//! Event propagation system for multi-window coordination.
//!
//! This module provides broadcast channels for cross-window communication:
//! - Cross-window file/directory opening (context menu "Open in Window")
//! - Preferences acting on the window that opened it

use dioxus::desktop::tao::window::WindowId;
use std::path::PathBuf;
use tokio::sync::broadcast;

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

// ============================================================================
// Preferences Window Events
// ============================================================================

/// Which sidebar a preferences slider is talking about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSide {
    Left,
}

/// Apply a zoom level to one window's sidebar, from the preferences window.
///
/// The "Current Settings" sliders act on the window that opened preferences.
/// Preferences is its own window now and holds no `AppState`, so the value
/// travels as an event to that window rather than being written directly.
pub static SET_SIDEBAR_ZOOM_IN_WINDOW: std::sync::LazyLock<
    broadcast::Sender<(WindowId, SidebarSide, f64)>,
> = std::sync::LazyLock::new(|| broadcast::channel(10).0);
