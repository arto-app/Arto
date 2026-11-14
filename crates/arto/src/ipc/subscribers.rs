//! Clients that asked to be told when a window changes.
//!
//! The one place the instance speaks first. A client says so in
//! `initialize`; until one does, [`publish`] costs a lock and a length
//! check, which matters because it is called from every scroll.
//!
//! Nothing is written to a socket from the thread that calls [`publish`].
//! That thread is the main one, and a client that has stopped reading can
//! fill its socket's buffer — so a write there would stall the window the
//! reader is scrolling. `publish` only records what should go out, and the
//! publisher thread does the writing.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use arto_lsp::{DidChangeStateParams, WindowKey, WindowState, Writer, DID_CHANGE_STATE};

/// The fastest a window that is only being scrolled is reported.
///
/// Scrolling changes the state on every frame, and a client watching where
/// the reader is wants to follow, not to be flooded. Everything else — a
/// document, a theme, a window closing — goes out at once.
const SCROLL_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Default)]
struct Registry {
    /// Numbered so a failed write can be dropped without asking whether
    /// two clones of a `Writer` are the same connection.
    clients: Vec<(u64, Writer)>,
    /// What each window was last reported as, and when.
    sent: HashMap<WindowKey, (Option<WindowState>, Instant)>,
    /// A scroll waiting out [`SCROLL_INTERVAL`].
    held: HashMap<WindowKey, Option<WindowState>>,
    /// Ready to go out, in the order it happened.
    queue: Vec<DidChangeStateParams>,
}

fn registry() -> &'static (Mutex<Registry>, Condvar) {
    static REGISTRY: OnceLock<(Mutex<Registry>, Condvar)> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Take on a client that asked to be told of changes.
pub fn subscribe(writer: Writer) {
    let (lock, changed) = registry();
    let Ok(mut registry) = lock.lock() else {
        return;
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    registry.clients.retain(|(_, client)| client.is_open());
    registry
        .clients
        .push((NEXT.fetch_add(1, Ordering::Relaxed), writer));

    // What was last sent describes clients that may be gone and never this
    // one, so keeping it would suppress this client's first change on the
    // grounds that somebody else had already heard it. A new subscriber is
    // told nothing until something moves; `arto/state` is how it learns
    // where things stand now.
    registry.sent.clear();

    start_publisher();
    changed.notify_one();
}

/// Report how a window reads now, or that it has closed.
///
/// Safe to call for a state that has not changed: an unchanged window is
/// not reported, so a caller may hand over everything it has whenever
/// anything at all moved.
pub fn publish(window: WindowKey, state: Option<WindowState>) {
    let (lock, changed) = registry();
    let Ok(mut registry) = lock.lock() else {
        return;
    };
    if registry.clients.is_empty() {
        return;
    }

    match registry.sent.get(&window) {
        Some((last, at)) => {
            if last == &state {
                return;
            }
            let scrolling =
                matches!((last, &state), (Some(last), Some(new)) if scroll_only(last, new));
            if scrolling && at.elapsed() < SCROLL_INTERVAL {
                registry.held.insert(window, state);
                changed.notify_one();
                return;
            }
        }
        // A close for a window no client was ever told about. Reporting it
        // would name a window the client has never seen.
        None if state.is_none() => return,
        None => {}
    }

    enqueue(&mut registry, window, state);
    changed.notify_one();
}

fn scroll_only(last: &WindowState, new: &WindowState) -> bool {
    last.document == new.document && last.theme == new.theme && last.headings == new.headings
}

fn enqueue(registry: &mut Registry, window: WindowKey, state: Option<WindowState>) {
    registry.held.remove(&window);
    match state {
        // A key is never handed out twice, so a closed window has nothing
        // left to compare against.
        None => {
            registry.sent.remove(&window);
        }
        Some(ref state) => {
            registry
                .sent
                .insert(window, (Some(state.clone()), Instant::now()));
        }
    }
    registry.queue.push(DidChangeStateParams { window, state });
}

/// Let go of the scrolls that have waited out [`SCROLL_INTERVAL`].
///
/// Without this the last move of a scroll would be the one never reported,
/// which is the only one a client actually wants.
fn release_due(registry: &mut Registry) {
    let due: Vec<WindowKey> = registry
        .held
        .keys()
        .copied()
        .filter(|window| {
            registry
                .sent
                .get(window)
                .is_none_or(|(_, at)| at.elapsed() >= SCROLL_INTERVAL)
        })
        .collect();
    for window in due {
        let Some(state) = registry.held.remove(&window) else {
            continue;
        };
        enqueue(registry, window, state);
    }
}

fn start_publisher() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("ipc-state-publisher".into())
            .spawn(publish_forever);
        if let Err(error) = spawned {
            tracing::warn!(%error, "Could not start the state publisher; no client will be told of changes");
        }
    });
}

fn publish_forever() {
    let (lock, changed) = registry();
    loop {
        let outgoing = {
            let Ok(mut registry) = lock.lock() else {
                return;
            };
            // A held scroll is the only thing that comes due on its own;
            // with none, there is nothing to wake up for.
            if registry.queue.is_empty() && registry.held.is_empty() {
                let Ok(guard) = changed.wait(registry) else {
                    return;
                };
                registry = guard;
            } else if registry.queue.is_empty() {
                let Ok((guard, _)) = changed.wait_timeout(registry, SCROLL_INTERVAL) else {
                    return;
                };
                registry = guard;
            }
            release_due(&mut registry);
            let outgoing = std::mem::take(&mut registry.queue);
            let clients = registry.clients.clone();
            (outgoing, clients)
        };

        let (outgoing, clients) = outgoing;
        if outgoing.is_empty() || clients.is_empty() {
            continue;
        }

        let mut gone = Vec::new();
        for (id, client) in &clients {
            let delivered = outgoing
                .iter()
                .all(|params| client.notify(DID_CHANGE_STATE, params));
            if !delivered {
                gone.push(*id);
            }
        }
        if gone.is_empty() {
            continue;
        }
        let Ok(mut registry) = lock.lock() else {
            return;
        };
        registry.clients.retain(|(id, _)| !gone.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arto_config::Theme;
    use arto_lsp::{Heading, ScrollPosition};

    fn window_state(scroll: u32, document: &str) -> WindowState {
        WindowState {
            window: WindowKey(1),
            document: Some(document.into()),
            theme: Theme::Dark,
            scroll: ScrollPosition {
                line: scroll,
                fraction: 0.0,
            },
            headings: vec![Heading {
                level: 1,
                text: "Install".into(),
                id: "install".into(),
            }],
        }
    }

    #[test]
    fn a_move_within_the_same_document_is_only_a_scroll() {
        assert!(scroll_only(
            &window_state(1, "/a.md"),
            &window_state(9, "/a.md")
        ));
    }

    #[test]
    fn opening_another_document_is_more_than_a_scroll() {
        assert!(!scroll_only(
            &window_state(1, "/a.md"),
            &window_state(1, "/b.md")
        ));
    }

    #[test]
    fn a_scroll_is_held_back_and_released_when_its_turn_comes() {
        let mut registry = Registry::default();
        enqueue(&mut registry, WindowKey(1), Some(window_state(1, "/a.md")));
        registry.queue.clear();
        registry
            .held
            .insert(WindowKey(1), Some(window_state(9, "/a.md")));

        release_due(&mut registry);
        assert!(registry.queue.is_empty(), "not due yet");

        registry
            .sent
            .insert(WindowKey(1), (Some(window_state(1, "/a.md")), earlier()));
        release_due(&mut registry);
        assert_eq!(registry.queue.len(), 1);
        assert!(registry.held.is_empty());
    }

    fn earlier() -> Instant {
        Instant::now() - SCROLL_INTERVAL - Duration::from_millis(1)
    }
}
