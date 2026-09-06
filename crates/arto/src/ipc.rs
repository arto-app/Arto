//! Single-instance IPC, the app's side.
//!
//! The protocol and the socket live in the `arto-ipc` crate. This module
//! connects them to the running app: it turns a CLI invocation into an
//! event, runs the server on a background thread, queues what arrives for
//! the main thread, wakes that thread, and finally opens files in the
//! right window.
//!
//! # Architecture
//!
//! ```text
//! 1st Instance (Primary):
//!   main() → try_send_to_existing_instance() → NoExistingInstance → start_ipc_server()
//!                                                                          ↓
//!                                                   IpcServer::serve() on a thread
//!                                                                          ↓
//!                                                   events → IPC_EVENT_QUEUE
//!                                                                          ↓
//!                                                   GCD wake → process_pending_events()
//!
//! 2nd Instance (Secondary):
//!   main() → try_send_to_existing_instance() → Sent → exit(0)
//! ```

mod queue;
mod request;
mod window_selection;

// Listed rather than glob-imported: this module wraps `cleanup_socket` and
// the server start-up with logging and shutdown handling, and the rest of
// the app should reach the transport only through those wrappers.
pub use arto_ipc::{OpenEvent, OpenRequest, SendResult};
pub use queue::*;
pub use request::*;

use crate::cli::CliInvocation;
use queue::{drain_events, SHUTDOWN_REQUESTED, SHUTDOWN_SIGNAL, SHUTDOWN_STARTED};
use std::sync::atomic::Ordering;
use window_selection::{select_target_window, select_target_window_with_behavior};

/// Try to hand this launch's request to an already running instance.
///
/// Returns only `Sent` or `NoExistingInstance`: a delivery failure is
/// logged and folded into `NoExistingInstance` here, so a primary that died
/// mid-handshake does not stop the user from opening anything and no call
/// site has to remember that rule.
pub fn try_send_to_existing_instance(invocation: &CliInvocation) -> SendResult {
    let event = open_event_for_invocation(invocation);
    match arto_ipc::send_to_existing_instance(&event) {
        SendResult::Failed(error) => {
            tracing::warn!(%error, "Failed to send to the primary instance; becoming primary");
            SendResult::NoExistingInstance
        }
        result => result,
    }
}

/// Start the IPC server to listen for connections from new instances.
///
/// This function spawns a background thread that accepts connections. Received
/// events are pushed to the global IPC event queue and the main thread is woken
/// via GCD to process them.
///
/// # Thread Lifecycle
///
/// The spawned thread runs indefinitely and is not explicitly joined on shutdown.
/// Socket cleanup relies on:
/// - Signal handlers (`register_cleanup_handler()`) to remove the socket on SIGTERM/SIGINT
/// - Stale socket detection on next startup to handle crashes
/// - OS-level cleanup when the process exits
///
/// This design trade-off avoids the complexity of coordinating graceful shutdown
/// with Dioxus's lifecycle. Any leftover socket file is harmless and cleaned up on next launch.
pub fn start_ipc_server() {
    // Register cleanup handler for graceful shutdown
    register_cleanup_handler();

    std::thread::spawn(move || {
        let server = match arto_ipc::IpcServer::bind() {
            Ok(server) => server,
            Err(error) => {
                tracing::error!(
                    %error,
                    "IPC server failed to start; single-instance enforcement is broken. \
                     Terminating to prevent duplicate instances."
                );
                // Fail fast: running without an IPC server breaks the single-instance guarantee,
                // so terminate the process rather than continuing in a degraded state.
                //
                // NOTE: process::exit() does not run destructors (e.g. use_drop / PersistedState::save).
                // This is acceptable because bind() fails during early startup, before any
                // user-visible windows or unsaved state exist.
                std::process::exit(1);
            }
        };
        tracing::info!(socket_path = ?server.socket_path(), "IPC server ready for connections");

        server.serve(|events| {
            for event in events {
                push_event(event);
            }
            // Wake main thread once per client connection.
            wake_main_thread();
        });
    });
}

/// Remove the IPC socket file on clean exit.
///
/// This prevents stale socket detection on next startup.
pub fn cleanup_socket() {
    match arto_ipc::cleanup_socket() {
        Ok(()) => tracing::debug!("IPC socket cleaned up"),
        Err(error) => tracing::warn!(%error, "Failed to remove IPC socket on cleanup"),
    }
}

// ============================================================================
// GCD wake mechanism — wake main thread from IPC background thread
// ============================================================================

/// Wake the main thread to process pending IPC events.
///
/// Uses macOS GCD to dispatch a callback to the main queue, which wakes
/// CFRunLoop even when App Nap has suspended the process. The callback
/// calls `process_main_thread_tasks()` on the main thread.
#[cfg(target_os = "macos")]
fn wake_main_thread() {
    extern "C" {
        // _dispatch_main_q is the GCD main dispatch queue (static symbol in libdispatch).
        // dispatch_get_main_queue() is a C macro that expands to &_dispatch_main_q,
        // so we reference the symbol directly for FFI.
        static _dispatch_main_q: u8;
        fn dispatch_async_f(
            queue: *const u8,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
    }

    extern "C" fn ipc_wake_callback(_context: *mut std::ffi::c_void) {
        // Runs on the main thread via GCD — safe to access MAIN_WINDOWS thread_local
        process_main_thread_tasks();
    }

    // SAFETY: _dispatch_main_q is a valid static symbol in libdispatch.
    // dispatch_async_f schedules the callback on the main thread.
    unsafe {
        let main_queue = std::ptr::addr_of!(_dispatch_main_q);
        dispatch_async_f(main_queue, std::ptr::null_mut(), ipc_wake_callback);
    }
}

#[cfg(not(target_os = "macos"))]
fn wake_main_thread() {
    // On non-macOS platforms, rely on custom_event_handler to run
    // process_main_thread_tasks() on the next event loop iteration.
}

// ============================================================================
// Main thread event processing — called from GCD callback or event handler
// ============================================================================

/// Process pending IPC events on the main thread.
///
/// MUST be called on the main thread (accesses MAIN_WINDOWS thread_local).
/// Called from:
/// - GCD wake callback (after IPC thread pushes events)
/// - custom_event_handler (defense in depth)
pub fn process_pending_events() {
    let Some(desktop) = crate::window::get_any_main_window() else {
        // No window available yet; events stay in queue for later processing
        return;
    };

    let events = drain_events();
    for event in events {
        handle_event_on_main_thread(&desktop, event);
    }
}

/// Process all main-thread IPC tasks.
///
/// This drains the open-event queue and handles pending graceful shutdown.
pub fn process_main_thread_tasks() {
    if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        process_shutdown_request();
        return;
    }

    process_pending_events();
    process_shutdown_request();
}

fn process_shutdown_request() {
    if !SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        return;
    }
    if SHUTDOWN_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tracing::info!("Starting graceful shutdown");
    cleanup_socket();
    let closed = crate::window::shutdown_all_windows();
    if closed == 0 {
        let signal = SHUTDOWN_SIGNAL.load(Ordering::SeqCst);
        tracing::warn!(
            signal,
            "No main windows found during shutdown; exiting process directly"
        );
        if signal > 0 {
            std::process::exit(128 + signal);
        }
        std::process::exit(0);
    }
}

/// Handle a single event by creating/showing windows. Runs on main thread.
fn handle_event_on_main_thread(
    desktop: &std::rc::Rc<dioxus::desktop::DesktopService>,
    event: OpenEvent,
) {
    match event {
        OpenEvent::Open(request) => {
            tracing::debug!(?request, "Processing open request event");
            open_request_with_behavior(desktop, request);
        }
        OpenEvent::Reopen { behavior } => {
            tracing::debug!(?behavior, "Processing reopen event");
            reopen_with_behavior(desktop, behavior);
        }
    }
}

fn open_request_with_behavior(
    desktop: &std::rc::Rc<dioxus::desktop::DesktopService>,
    request: OpenRequest,
) {
    let behavior = request
        .behavior
        .unwrap_or_else(|| crate::config::CONFIG.read().file_open);

    if let Some(window_id) = select_target_window_with_behavior(behavior) {
        if let Some(mut state) = crate::window::main::get_window_state(window_id) {
            apply_open_request_to_state(&mut state, &request);
            let _ = crate::window::main::focus_window(window_id);
            return;
        }
    }

    let params = crate::window::CreateMainWindowConfigParams {
        directory: request.directory,
        ..Default::default()
    };

    let tabs = if request.files.is_empty() {
        vec![crate::state::Tab::default()]
    } else {
        request
            .files
            .iter()
            .cloned()
            .map(crate::state::Tab::new)
            .collect()
    };
    crate::window::create_main_window_sync_with_tabs(desktop, tabs, params);
}

fn apply_open_request_to_state(state: &mut crate::state::AppState, request: &OpenRequest) {
    if let Some(directory) = request.directory.as_ref() {
        state.set_root_directory(directory.clone());
    }
    for path in &request.files {
        state.open_file(path);
    }
}

fn reopen_with_behavior(
    desktop: &std::rc::Rc<dioxus::desktop::DesktopService>,
    behavior: Option<crate::config::FileOpenBehavior>,
) {
    let target_window = match behavior {
        Some(behavior) => select_target_window_with_behavior(behavior),
        None => select_target_window(),
    };

    // First try to focus an existing visible window
    if let Some(window_id) = target_window {
        if crate::window::main::focus_window(window_id) {
            return;
        }
    }

    // If no visible windows, try to show and focus a hidden window (e.g., MainApp with WindowHides)
    if crate::window::main::show_and_focus_hidden_window() {
        return;
    }

    // If no windows at all, create a new one
    crate::window::create_main_window_sync(
        desktop,
        crate::state::Tab::default(),
        crate::window::CreateMainWindowConfigParams::default(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileOpenBehavior;
    use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
    use std::path::{Path, PathBuf};
    use window_selection::{
        choose_open_target, is_window_on_display, OpenTarget, WindowBounds, WindowSelectionInput,
    };

    #[test]
    fn test_event_queue_fifo_ordering() {
        // Drain any leftover events from other tests (global static is shared)
        drain_events();

        push_event(OpenEvent::Open(OpenRequest {
            files: vec![PathBuf::from("/first.md")],
            directory: None,
            behavior: None,
        }));
        push_event(OpenEvent::Open(OpenRequest {
            files: Vec::new(),
            directory: Some(PathBuf::from("/second")),
            behavior: None,
        }));
        push_event(OpenEvent::Reopen { behavior: None });

        // try_pop_first_event returns FIFO order
        let first = try_pop_first_event();
        assert!(matches!(
            first,
            Some(OpenEvent::Open(OpenRequest { files, directory: None, .. }))
            if files == vec![PathBuf::from("/first.md")]
        ));

        // drain_events returns remaining in FIFO order
        let remaining = drain_events();
        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            &remaining[0],
            OpenEvent::Open(OpenRequest { files, directory: Some(directory), .. })
            if files.is_empty() && directory == Path::new("/second")
        ));
        assert!(matches!(
            &remaining[1],
            OpenEvent::Reopen { behavior: None }
        ));

        // Queue is empty after drain
        assert!(try_pop_first_event().is_none());
        assert!(drain_events().is_empty());
    }

    #[test]
    fn test_choose_open_target_new_window_always_creates_new_window() {
        let windows = vec![WindowSelectionInput {
            window_id: 1_u8,
            is_on_current_screen: true,
        }];

        let target = choose_open_target(FileOpenBehavior::NewWindow, &windows, Some(1));
        assert_eq!(target, OpenTarget::NewWindow);
    }

    #[test]
    fn test_choose_open_target_last_focused_uses_visible_window() {
        let windows = vec![
            WindowSelectionInput {
                window_id: 1_u8,
                is_on_current_screen: false,
            },
            WindowSelectionInput {
                window_id: 2_u8,
                is_on_current_screen: true,
            },
        ];

        let target = choose_open_target(FileOpenBehavior::LastFocused, &windows, Some(2));
        assert_eq!(target, OpenTarget::ExistingWindow(2));
    }

    #[test]
    fn test_choose_open_target_last_focused_falls_back_to_new_window() {
        let windows = vec![WindowSelectionInput {
            window_id: 1_u8,
            is_on_current_screen: true,
        }];

        let target = choose_open_target(FileOpenBehavior::LastFocused, &windows, Some(2));
        assert_eq!(target, OpenTarget::NewWindow);
    }

    #[test]
    fn test_choose_open_target_current_screen_prefers_last_focused() {
        let windows = vec![
            WindowSelectionInput {
                window_id: 1_u8,
                is_on_current_screen: true,
            },
            WindowSelectionInput {
                window_id: 2_u8,
                is_on_current_screen: true,
            },
        ];

        let target = choose_open_target(FileOpenBehavior::CurrentScreen, &windows, Some(2));
        assert_eq!(target, OpenTarget::ExistingWindow(2));
    }

    #[test]
    fn test_choose_open_target_current_screen_uses_first_candidate_without_last_focus() {
        let windows = vec![
            WindowSelectionInput {
                window_id: 4_u8,
                is_on_current_screen: true,
            },
            WindowSelectionInput {
                window_id: 5_u8,
                is_on_current_screen: true,
            },
        ];

        let target = choose_open_target(FileOpenBehavior::CurrentScreen, &windows, None);
        assert_eq!(target, OpenTarget::ExistingWindow(4));
    }

    #[test]
    fn test_choose_open_target_current_screen_falls_back_to_new_window() {
        let windows = vec![WindowSelectionInput {
            window_id: 1_u8,
            is_on_current_screen: false,
        }];

        let target = choose_open_target(FileOpenBehavior::CurrentScreen, &windows, Some(1));
        assert_eq!(target, OpenTarget::NewWindow);
    }

    #[test]
    fn test_is_window_on_display_fully_inside() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        let bounds = WindowBounds {
            x: 300,
            y: 250,
            width: 400,
            height: 300,
        };
        assert!(is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_spanning_two_monitors() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        // Window straddles the right edge: 50% on left monitor
        let bounds = WindowBounds {
            x: 1720,
            y: 200,
            width: 400,
            height: 300,
        };
        // overlap = 200*300 = 60000, window = 400*300 = 120000 → 50% > 10%
        assert!(is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_minor_overlap_rejected() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        // Only 20px overlap on a 400px-wide window → 5% < 10%
        let bounds = WindowBounds {
            x: 1900,
            y: 200,
            width: 400,
            height: 300,
        };
        // overlap = 20*300 = 6000, window = 400*300 = 120000 → 5% < 10%
        assert!(!is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_completely_outside() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        let bounds = WindowBounds {
            x: 1921,
            y: 0,
            width: 400,
            height: 300,
        };
        assert!(!is_window_on_display(display, bounds));
    }

    #[test]
    fn test_is_window_on_display_hidden_in_corner() {
        let display = (LogicalPosition::new(0, 0), LogicalSize::new(1920, 1080));
        // Simulates Aerospace hideInCorner: window at bottom-right with 1px overlap
        // overlap = 1*1 = 1, window = 1265*2109 → ~0.00004% < 10%
        let bounds = WindowBounds {
            x: 5119,
            y: 2132,
            width: 1265,
            height: 2109,
        };
        assert!(!is_window_on_display(display, bounds));
    }
}
