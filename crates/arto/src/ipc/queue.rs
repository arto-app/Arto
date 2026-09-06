use super::protocol::OpenEvent;
use parking_lot::Mutex;
use std::collections::VecDeque;
#[cfg(unix)]
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::OnceLock;

// ============================================================================
// IPC Event Queue — thread-safe queue for IPC → main thread communication
// ============================================================================

/// Global event queue for IPC messages.
///
/// IPC threads push events here via `push_event()`.
/// The main thread drains them via `process_pending_events()`, which is called
/// from the GCD wake callback or the custom_event_handler.
///
/// Uses `parking_lot::Mutex` instead of `std::sync::Mutex` because IPC queue
/// operations are infallible — there is no recovery strategy for a poisoned
/// mutex, and panicking while holding the lock indicates a bug, not a state
/// that other threads should reason about.
static IPC_EVENT_QUEUE: OnceLock<Mutex<VecDeque<OpenEvent>>> = OnceLock::new();
pub(super) static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(super) static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);
pub(super) static SHUTDOWN_SIGNAL: AtomicI32 = AtomicI32::new(0);

fn get_event_queue() -> &'static Mutex<VecDeque<OpenEvent>> {
    IPC_EVENT_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(unix)]
pub(super) fn request_shutdown(signal: i32) {
    let was_requested = SHUTDOWN_REQUESTED.swap(true, Ordering::SeqCst);
    if was_requested {
        tracing::warn!(
            signal,
            "Second termination signal received during shutdown; forcing immediate exit"
        );
        super::socket::cleanup_socket();
        std::process::exit(128 + signal);
    }
    SHUTDOWN_SIGNAL.store(signal, Ordering::SeqCst);

    tracing::info!(
        signal,
        "Termination signal received; requesting graceful shutdown"
    );
    super::wake_main_thread();

    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS fallback: if the event loop does not begin shutdown for a
        // long time, force exit as a last resort.
        const SHUTDOWN_START_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        const SHUTDOWN_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < SHUTDOWN_START_TIMEOUT {
                if SHUTDOWN_STARTED.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
            }

            if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) && !SHUTDOWN_STARTED.load(Ordering::SeqCst)
            {
                tracing::warn!(
                    signal,
                    "Main thread did not start graceful shutdown in time; forcing exit"
                );
                super::socket::cleanup_socket();
                std::process::exit(128 + signal);
            }
        });
    }
}

/// Push an event to the IPC queue. Thread-safe.
pub fn push_event(event: OpenEvent) {
    get_event_queue().lock().push_back(event);
}

/// Pop the first event from the queue (for initial event in MainApp).
pub fn try_pop_first_event() -> Option<OpenEvent> {
    get_event_queue().lock().pop_front()
}

/// Drain all pending events from the IPC queue.
pub(super) fn drain_events() -> VecDeque<OpenEvent> {
    std::mem::take(&mut *get_event_queue().lock())
}
