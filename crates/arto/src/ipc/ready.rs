//! Telling a launch that asked to wait that its window has drawn.
//!
//! `arto --wait-ready` exists so a script does not have to guess how long a
//! document takes to appear. The launch holds its socket open and the
//! primary answers it here, once — and only once — the window that took the
//! request has finished drawing.
//!
//! Which window that is depends on what the request did. A request that
//! landed in a window already open names it, and [`await_window`] binds the
//! responder to that id. A request that created one cannot: window creation is
//! fire-and-forget, so the id does not exist yet when the request is
//! applied. [`await_new_window`] parks the responder instead and the first main
//! window to mount claims it.
//!
//! That is the window the request asked for, except in the one case where a
//! single launch asks for several — `arto --wait-ready a.md b.md`, which
//! gives each file a window of its own. The answer then comes from whichever
//! of them drew first rather than from all of them, which is the reading of
//! "ready" that a launch naming several files can be given without waiting
//! on the slowest.
//!
//! A window takes its responders *out* of here before it starts watching for a
//! draw, so what it finally answers is what was waiting when the draw was
//! asked for. A request that arrives while the window is already drawing is
//! left in the registry for the round after, because the draw in progress is
//! not the one it asked about.
//!
//! A responder that is dropped rather than fired releases the launch too, so a
//! window that closes — or is torn down mid-draw — strands nothing.

use arto_lsp::{AppliedResult, Responder};
use dioxus::desktop::tao::window::WindowId;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// A responder, and the file its request left the window showing (`None`
/// for the welcome page).
pub type Waiting = (Option<PathBuf>, Responder);

/// Signals whose window is already open.
static AWAITING_WINDOW: LazyLock<Mutex<HashMap<WindowId, Vec<Waiting>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Signals whose window has been asked for but does not exist yet.
static AWAITING_NEW_WINDOW: LazyLock<Mutex<Vec<Responder>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Answer this responder when `window_id` next finishes drawing `file`.
///
/// The file is recorded now rather than read when the window gets round to
/// answering, because by then a later request may already have replaced it,
/// and the draw of its file is not the one this request asked about.
pub fn await_window(window_id: WindowId, file: Option<PathBuf>, responder: Responder) {
    AWAITING_WINDOW
        .lock()
        .entry(window_id)
        .or_default()
        .push((file, responder));
}

/// Answer this responder when the window that is about to be created draws.
pub fn await_new_window(responder: Responder) {
    AWAITING_NEW_WINDOW.lock().push(responder);
}

/// Take over every responder parked for a window that did not exist yet.
///
/// Called by a main window as it mounts. What comes back is the window's to
/// answer — or to drop, which answers it too.
pub fn claim_new_window() -> Vec<Responder> {
    std::mem::take(&mut *AWAITING_NEW_WINDOW.lock())
}

/// Take every responder currently waiting on this window.
pub fn take_pending(window_id: WindowId) -> Vec<Waiting> {
    AWAITING_WINDOW
        .lock()
        .remove(&window_id)
        .unwrap_or_default()
}

/// Split `waiting` into those whose request left the window on `shown` and
/// those whose file has since been replaced, which no draw will answer.
pub fn split_by_shown_file<T>(
    waiting: Vec<(Option<PathBuf>, T)>,
    shown: Option<&Path>,
) -> (Vec<T>, Vec<T>) {
    let (drawn, superseded): (Vec<_>, Vec<_>) = waiting
        .into_iter()
        .partition(|(file, _)| file.as_deref() == shown);
    (
        drawn.into_iter().map(|(_, waiter)| waiter).collect(),
        superseded.into_iter().map(|(_, waiter)| waiter).collect(),
    )
}

/// Answer every request still bound to a window that is going away.
///
/// `ready: false` is the honest answer: the window will never draw, and a
/// launch told so now beats one held until its own timeout runs out.
pub fn forget_window(window_id: WindowId) {
    let Some(waiting) = AWAITING_WINDOW.lock().remove(&window_id) else {
        return;
    };
    tracing::debug!(?window_id, "Window closed before it reported ready");
    for (_, responder) in waiting {
        responder.ok(AppliedResult { ready: false });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_request_whose_file_was_replaced_is_not_answered_by_the_draw() {
        let waiting = vec![
            (Some(PathBuf::from("/a.md")), "a"),
            (Some(PathBuf::from("/b.md")), "b"),
            (None, "welcome"),
        ];

        let (drawn, superseded) = split_by_shown_file(waiting, Some(Path::new("/b.md")));

        assert_eq!(drawn, vec!["b"]);
        assert_eq!(superseded, vec!["a", "welcome"]);
    }

    #[test]
    fn test_a_request_for_the_welcome_page_is_answered_by_its_draw() {
        let waiting = vec![(None, "reopen"), (Some(PathBuf::from("/a.md")), "a")];

        let (drawn, superseded) = split_by_shown_file(waiting, None);

        assert_eq!(drawn, vec!["reopen"]);
        assert_eq!(superseded, vec!["a"]);
    }
}
