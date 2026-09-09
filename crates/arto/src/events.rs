//! Event propagation system for multi-window coordination.
//!
//! This module provides broadcast channels for cross-window communication:
//! - Preferences acting on the window that opened it

use dioxus::desktop::tao::window::WindowId;
use tokio::sync::broadcast;

// ============================================================================
// Preferences Window Events
// ============================================================================

/// Apply a zoom level to one window's panel, from the preferences window.
///
/// The "Current Settings" sliders act on the window that opened preferences.
/// Preferences is its own window now and holds no `AppState`, so the value
/// travels as an event to that window rather than being written directly.
pub static SET_SIDEBAR_ZOOM_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<(WindowId, f64)>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);
