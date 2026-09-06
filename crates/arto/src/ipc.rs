//! Single-instance IPC module
//!
//! This module provides IPC functionality to ensure only one instance of Arto runs at a time.
//! When a second instance is launched with paths, it sends those paths to the existing instance
//! via a local socket and exits.
//!
//! # Architecture
//!
//! ```text
//! 1st Instance (Primary):
//!   main() → try_send_to_existing() fails → start_ipc_server()
//!                                                 ↓
//!                                           accept connections
//!                                                 ↓
//!                                           recv JSON Lines → IPC_EVENT_QUEUE
//!                                                 ↓
//!                                           GCD wake → process_pending_events()
//!
//! 2nd Instance (Secondary):
//!   main() → try_send_to_existing() succeeds → exit(0)
//! ```
//!
//! # Protocol
//!
//! Messages are sent as JSON Lines (one JSON object per line):
//!
//! ```json
//! {"type":"open","files":["/path/to/file.md"],"directory":null,"behavior":"last_focused"}
//! {"type":"open","files":[],"directory":"/path/to/dir","behavior":"new_window"}
//! {"type":"reopen","behavior":"last_focused"}
//! ```

mod client;
mod protocol;
mod queue;
mod server;
pub(crate) mod socket;
mod window_selection;

pub use client::*;
pub use protocol::*;
pub use queue::*;
pub use server::*;
pub use socket::cleanup_socket;

use queue::{drain_events, SHUTDOWN_REQUESTED, SHUTDOWN_SIGNAL, SHUTDOWN_STARTED};
use std::sync::atomic::Ordering;
use window_selection::{select_target_window, select_target_window_with_behavior};

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
    use indoc::indoc;
    use std::path::{Path, PathBuf};
    use window_selection::{
        choose_open_target, is_window_on_display, OpenTarget, WindowBounds, WindowSelectionInput,
    };

    // Re-import protocol types used by tests
    use protocol::IpcMessage;
    use socket::is_address_in_use;
    #[cfg(unix)]
    use socket::{get_socket_path, SOCKET_NAME};

    #[test]
    fn test_ipc_message_open_serialization() {
        let msg = IpcMessage::Open {
            files: vec![PathBuf::from("/path/to/file.md")],
            directory: Some(PathBuf::from("/path/to/dir")),
            behavior: Some(FileOpenBehavior::LastFocused),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"type":"open","files":["/path/to/file.md"],"directory":"/path/to/dir","behavior":"last_focused"}"#
        );
    }

    #[test]
    fn test_ipc_message_reopen_serialization() {
        let msg = IpcMessage::Reopen {
            behavior: Some(FileOpenBehavior::LastFocused),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"reopen","behavior":"last_focused"}"#);
    }

    #[test]
    fn test_ipc_message_open_deserialization() {
        let json = r#"{"type":"open","files":["/path/to/file.md"],"directory":"/path/to/dir","behavior":"last_focused"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            IpcMessage::Open {
                files,
                directory,
                behavior: Some(FileOpenBehavior::LastFocused)
            } if files == vec![PathBuf::from("/path/to/file.md")] && directory == Some(PathBuf::from("/path/to/dir"))
        ));
    }

    #[test]
    fn test_ipc_message_reopen_deserialization() {
        let json = r#"{"type":"reopen","behavior":"last_focused"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            IpcMessage::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused)
            }
        ));
    }

    #[test]
    fn test_ipc_message_reopen_deserialization_legacy_without_behavior() {
        let json = r#"{"type":"reopen"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, IpcMessage::Reopen { behavior: None }));
    }

    #[test]
    fn test_ipc_message_into_open_event_open() {
        let msg = IpcMessage::Open {
            files: vec![PathBuf::from("/test.md")],
            directory: Some(PathBuf::from("/test/dir")),
            behavior: Some(FileOpenBehavior::CurrentScreen),
        };
        let event = msg.into_open_event();
        assert!(matches!(
            event,
            OpenEvent::Open(OpenRequest {
                files,
                directory: Some(directory),
                behavior: Some(FileOpenBehavior::CurrentScreen)
            }) if files == vec![PathBuf::from("/test.md")] && directory == Path::new("/test/dir")
        ));
    }

    #[test]
    fn test_ipc_message_into_open_event_reopen() {
        let msg = IpcMessage::Reopen {
            behavior: Some(FileOpenBehavior::LastFocused),
        };
        let event = msg.into_open_event();
        assert!(matches!(
            event,
            OpenEvent::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused)
            }
        ));
    }

    #[test]
    fn test_json_lines_protocol() {
        // Test that multiple messages can be parsed from newline-separated JSON
        let input = indoc! {r#"
            {"type":"open","files":["/file1.md"],"directory":null,"behavior":"last_focused"}
            {"type":"open","files":[],"directory":"/dir","behavior":"new_window"}
            {"type":"reopen","behavior":"current_screen"}
        "#};

        let messages: Vec<IpcMessage> = input
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(messages.len(), 3);
        assert!(matches!(
            &messages[0],
            IpcMessage::Open { files, directory: None, behavior: Some(FileOpenBehavior::LastFocused) }
            if files == &vec![PathBuf::from("/file1.md")]
        ));
        assert!(matches!(
            &messages[1],
            IpcMessage::Open { files, directory: Some(directory), behavior: Some(FileOpenBehavior::NewWindow) }
            if files.is_empty() && directory == Path::new("/dir")
        ));
        assert!(matches!(
            &messages[2],
            IpcMessage::Reopen {
                behavior: Some(FileOpenBehavior::CurrentScreen)
            }
        ));
    }

    #[test]
    #[cfg(unix)]
    fn test_socket_path_contains_user_id() {
        let path = get_socket_path();

        // Ensure the socket file name is exactly SOCKET_NAME
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("Socket path should have a valid UTF-8 file name");
        assert_eq!(
            file_name, SOCKET_NAME,
            "Socket file name should be '{}', got '{}'",
            SOCKET_NAME, file_name
        );

        // Either XDG_RUNTIME_DIR or /tmp/arto-{uid}/
        let parent = path
            .parent()
            .expect("Socket path should have a parent directory");

        let xdg_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let parent_matches_xdg = xdg_runtime_dir.as_deref().is_some_and(|xdg| parent == xdg);

        let parent_str = parent.to_string_lossy();
        let parent_matches_tmp = parent_str.starts_with("/tmp/arto-");

        assert!(
            parent_matches_xdg || parent_matches_tmp,
            "Socket directory should be XDG_RUNTIME_DIR ({:?}) or start with '/tmp/arto-'; got {}",
            xdg_runtime_dir,
            parent_str
        );
    }

    #[test]
    fn test_is_address_in_use_for_non_matching_error() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        assert!(!is_address_in_use(&err));
    }

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
