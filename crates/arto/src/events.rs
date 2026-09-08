//! Event propagation system for multi-window coordination.
//!
//! This module provides broadcast channels for cross-window communication:
//! - Preferences acting on the window that opened it

use dioxus::desktop::tao::window::WindowId;
use tokio::sync::broadcast;

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
