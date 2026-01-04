//! Event propagation system for multi-window coordination.
//!
//! # Event Flow Architecture
//!
//! This module implements Layer 2 of a 3-layer event propagation system:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │ Layer 1: OS → Dioxus (Single Consumer)                             │
//! │ File: main.rs → components/entrypoint.rs                           │
//! │ Channel: mpsc (single producer, single consumer)                   │
//! │                                                                     │
//! │  OS Event (Finder/CLI)                                             │
//! │       │                                                             │
//! │       ├──→ Event::Opened { urls } ──→ OpenEvent::File(path)        │
//! │       ├──→ Event::Opened { urls } ──→ OpenEvent::Directory(path)   │
//! │       └──→ Event::Reopen          ──→ OpenEvent::Reopen            │
//! │                │                                                    │
//! │                v                                                    │
//! │       OPEN_EVENT_RECEIVER (mpsc::Receiver)                         │
//! │                │                                                    │
//! │                v                                                    │
//! │       Entrypoint component (consumes once)                         │
//! └─────────────────────────────────────────────────────────────────────┘
//!                          │
//!                          v
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │ Layer 2: Entrypoint → Apps (Multiple Consumers) ← THIS MODULE      │
//! │ File: events.rs                                                     │
//! │ Channel: broadcast (single producer, multiple consumers)           │
//! │                                                                     │
//! │  Entrypoint logic:                                                 │
//! │    - Check if windows exist                                        │
//! │    - Handle Reopen specially (focus or create)                     │
//! │    - Broadcast to all windows if needed                            │
//! │                │                                                    │
//! │                ├──→ FILE_OPEN_BROADCAST.send(path)                 │
//! │                └──→ DIRECTORY_OPEN_BROADCAST.send(path)            │
//! │                         │                                           │
//! │                         v                                           │
//! │            ┌────────────┴────────────┐                              │
//! │            │                         │                              │
//! │            v                         v                              │
//! │       Window 1 (App)            Window 2 (App)                     │
//! │       rx.subscribe()            rx.subscribe()                     │
//! └─────────────────────────────────────────────────────────────────────┘
//!                          │
//!                          v
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │ Layer 3: Focus-based Filtering (Per-Window)                        │
//! │ File: components/app.rs                                            │
//! │                                                                     │
//! │  Each App component:                                               │
//! │    while let Ok(path) = rx.recv().await {                          │
//! │        if window().is_focused() {  ← Only focused window handles   │
//! │            state.open_file(path);                                  │
//! │        }                                                            │
//! │    }                                                                │
//! │                                                                     │
//! │  Why focus check? Without it, ALL windows would open the file!     │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Why Broadcast Channels?
//!
//! We use `tokio::sync::broadcast` instead of `mpsc` because:
//! - **Multiple windows**: Each window needs to receive the same event
//! - **Dynamic subscribers**: Windows can be created/destroyed at runtime
//! - **Focus filtering**: All windows receive events, but only focused window acts
//!
//! # Why Not Direct OS → Apps?
//!
//! We cannot send directly from `main.rs` to `App` components because:
//! 1. **Synchronous initial event**: First event must be handled before any window exists
//! 2. **Window existence check**: Need to create window if none exist
//! 3. **Reopen special handling**: App icon click should focus existing window, not always open file
//!
//! The Entrypoint layer provides this coordination logic before broadcasting to App components.

use crate::state::Tab;
use dioxus::desktop::tao::window::WindowId;
use std::path::PathBuf;
use tokio::sync::broadcast;

/// Global broadcast sender for opening files in tabs.
///
/// Distributes file open events from Entrypoint to all App components.
/// Each window's App component subscribes via `FILE_OPEN_BROADCAST.subscribe()`.
/// Only the focused window should handle the event (checked via `window().is_focused()`).
pub static FILE_OPEN_BROADCAST: std::sync::LazyLock<broadcast::Sender<PathBuf>> =
    std::sync::LazyLock::new(|| broadcast::channel(100).0);

/// Global broadcast sender for opening directories in sidebar.
///
/// Distributes directory open events from Entrypoint to all App components.
/// Unlike file events, directory events are typically handled by ALL windows
/// (updating each window's sidebar root), but this can be changed to focused-only if needed.
pub static DIRECTORY_OPEN_BROADCAST: std::sync::LazyLock<broadcast::Sender<PathBuf>> =
    std::sync::LazyLock::new(|| broadcast::channel(100).0);

// ============================================================================
// Tab Transfer Events (Two-Phase Commit Pattern)
// ============================================================================

/// Tab transfer request (Prepare phase of Two-Phase Commit)
///
/// Sent from source window to target window to initiate tab transfer.
/// The target window validates the request and sends back Ack/Nack.
#[derive(Debug, Clone)]
pub struct TabTransferRequest {
    /// Source window that wants to transfer the tab
    pub source_window_id: WindowId,
    /// Target window that will receive the tab
    pub target_window_id: WindowId,
    /// The tab to transfer (with full history)
    pub tab: Tab,
    /// Target index in the tab bar (None = append at end)
    pub target_index: Option<usize>,
    /// Preserve source window's current directory for new window
    #[allow(dead_code)]
    pub source_directory: Option<PathBuf>,
    /// Unique ID to match request/response pairs
    pub request_id: uuid::Uuid,
}

/// Tab transfer response (Commit/Abort phase of Two-Phase Commit)
///
/// Sent from target window back to source window after validating the request.
#[derive(Debug, Clone)]
pub enum TabTransferResponse {
    /// Target accepts the tab transfer (Commit phase)
    Ack {
        /// Matches the request_id from TabTransferRequest
        request_id: uuid::Uuid,
        /// Source window that initiated the transfer
        #[allow(dead_code)]
        source_window_id: WindowId,
    },
    /// Target rejects the tab transfer (Abort phase)
    Nack {
        /// Matches the request_id from TabTransferRequest
        request_id: uuid::Uuid,
        /// Source window that initiated the transfer
        #[allow(dead_code)]
        source_window_id: WindowId,
        /// Human-readable reason for rejection
        reason: String,
    },
}

/// Global broadcast sender for tab transfer requests.
///
/// Used in Two-Phase Commit pattern:
/// 1. Source window sends TabTransferRequest
/// 2. Target window receives and validates
/// 3. Target responds via TAB_TRANSFER_RESPONSE
///
/// Capacity of 10 is sufficient for desktop use (most users won't have 10+ windows).
/// Smaller buffer makes lag issues more obvious during development.
pub static TAB_TRANSFER_REQUEST: std::sync::LazyLock<broadcast::Sender<TabTransferRequest>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

/// Global broadcast sender for tab transfer responses.
///
/// Used in Two-Phase Commit pattern:
/// 1. Target window sends Ack/Nack
/// 2. Source window receives response
/// 3. Source commits (close tab) or aborts (keep tab)
///
/// Capacity of 10 is sufficient for desktop use (most users won't have 10+ windows).
/// Smaller buffer makes lag issues more obvious during development.
pub static TAB_TRANSFER_RESPONSE: std::sync::LazyLock<broadcast::Sender<TabTransferResponse>> =
    std::sync::LazyLock::new(|| broadcast::channel(10).0);

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
