//! Event propagation system for multi-window coordination.
//!
//! This module provides broadcast channels for cross-window communication:
//! - Preferences acting on the window that opened it
//! - A launch waiting to be told the window it reached has drawn

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

/// Apply a zoom level to one window's document, from the preferences window.
///
/// The counterpart of [`SET_SIDEBAR_ZOOM_IN_WINDOW`] for the page itself.
pub static SET_CONTENT_ZOOM_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<(WindowId, f64)>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

// ============================================================================
// Readiness
// ============================================================================

/// Ask one window to report once it has next finished drawing.
///
/// Sent when a request from `arto --wait-ready` lands in a window that was
/// already open, because such a window has no reason to notice on its own
/// that anything is now waiting on it. A window created *for* the request
/// needs no event: it claims what is waiting as it mounts (see
/// [`crate::ipc::ready`]).
pub static REPORT_READY_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<WindowId>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

/// Move the reader within one window's document.
///
/// Travels as an event because the move itself is `document::eval`, which
/// only reaches a WebView from inside that window's own scope.
pub static GOTO_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<(WindowId, GotoTarget)>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

/// Ask a window what the reader has selected.
///
/// The `u64` names the request that is waiting; the window answers it
/// through [`crate::ipc::answer_selection`]. A selection lives in the
/// WebView, so unlike everything else a client can ask for, this one cannot
/// be read off the main thread.
pub static READ_SELECTION_IN_WINDOW: std::sync::LazyLock<broadcast::Sender<(WindowId, u64)>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

pub use arto_lsp::GotoTarget;
