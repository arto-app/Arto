//! The bridge between the IPC thread and the main thread: a queue of
//! events waiting to be applied, and the shutdown flags a termination
//! signal sets for the main thread to act on.

use super::OpenEvent;
use arto_ipc::ReadySignal;
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
static IPC_EVENT_QUEUE: OnceLock<Mutex<VecDeque<QueuedEvent>>> = OnceLock::new();
pub(super) static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(super) static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);
pub(super) static SHUTDOWN_SIGNAL: AtomicI32 = AtomicI32::new(0);

fn get_event_queue() -> &'static Mutex<VecDeque<QueuedEvent>> {
    IPC_EVENT_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// An event on its way to the main thread, and the launch waiting on it.
///
/// The signal rides beside the event rather than inside it because waiting
/// is the sending connection's business: the same event means the same thing
/// whether or not anyone is holding a socket open for it.
pub struct QueuedEvent {
    pub event: OpenEvent,
    pub ready: Option<ReadySignal>,
}

/// Register signal handlers for clean socket cleanup.
///
/// This complements the stale socket detection on startup by handling
/// graceful shutdown cases like SIGINT and SIGTERM.
///
/// Uses signal-hook to allow multiple independent signal handlers to coexist,
/// avoiding conflicts with other parts of the application that may need to
/// handle signals.
#[cfg(unix)]
pub(super) fn register_cleanup_handler() {
    use signal_hook::{consts::signal::*, iterator::Signals};
    use std::sync::Once;
    use std::thread;

    static REGISTER_ONCE: Once = Once::new();

    REGISTER_ONCE.call_once(|| {
        match Signals::new([SIGINT, SIGTERM]) {
            Ok(mut signals) => {
                // Spawn a dedicated thread to listen for termination signals and
                // request graceful shutdown when they are received. This approach
                // allows multiple independent signal handlers to coexist.
                thread::spawn(move || {
                    for signal in &mut signals {
                        match signal {
                            SIGINT | SIGTERM => {
                                request_shutdown(signal);
                            }
                            _ => {}
                        }
                    }
                });
                tracing::debug!("IPC cleanup signal handler registered");
            }
            Err(e) => {
                tracing::warn!(?e, "Failed to register IPC cleanup signal handler");
            }
        }
    });
}

#[cfg(not(unix))]
pub(super) fn register_cleanup_handler() {
    // No-op on Windows
}

#[cfg(unix)]
fn request_shutdown(signal: i32) {
    let was_requested = SHUTDOWN_REQUESTED.swap(true, Ordering::SeqCst);
    if was_requested {
        tracing::warn!(
            signal,
            "Second termination signal received during shutdown; forcing immediate exit"
        );
        super::cleanup_socket();
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
                super::cleanup_socket();
                std::process::exit(128 + signal);
            }
        });
    }
}

/// Push an event to the IPC queue. Thread-safe.
pub fn push_event(event: OpenEvent) {
    push_queued_event(QueuedEvent { event, ready: None });
}

/// Push an event together with the launch waiting for its window.
pub fn push_queued_event(queued: QueuedEvent) {
    get_event_queue().lock().push_back(queued);
}

/// Pop the first event from the queue (for initial event in MainApp).
///
/// Normally this is what the launch itself asked for and nothing waits on
/// it. But the IPC server starts listening before the launch queues its own
/// event, so a second launch can slip in ahead of it and have its request
/// become the first window's. That launch is waiting, so its signal is
/// parked for the window this event is about to build rather than dropped —
/// dropping it would answer a request that had in fact been carried out.
pub fn try_pop_first_event() -> Option<OpenEvent> {
    let queued = get_event_queue().lock().pop_front()?;
    if let Some(signal) = queued.ready {
        super::ready::await_new_window(signal);
    }
    Some(queued.event)
}

/// Drain all pending events from the IPC queue.
pub(super) fn drain_events() -> VecDeque<QueuedEvent> {
    std::mem::take(&mut *get_event_queue().lock())
}
