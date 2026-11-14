//! Requests parked while a window is asked what the reader has selected.
//!
//! Everything else a client asks for is on the main thread already. A
//! selection is not — it lives in the WebView — so the answer arrives a
//! round trip later, from the window's own scope. The responder waits here
//! in the meantime, named by a number the window hands back.
//!
//! A responder dropped rather than answered releases the client too, so a
//! window that closes mid-question strands nothing.

use arto_lsp::{ErrorCode, Responder, ResponseError, SelectionResult, WindowKey, APPLY_TIMEOUT};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::Instant;

struct Asked {
    window: WindowKey,
    responder: Responder,
    at: Instant,
}

static ASKED: LazyLock<Mutex<HashMap<u64, Asked>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Park a responder and return the number the window answers it by.
pub fn ask(window: WindowKey, responder: Responder) -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let token = NEXT.fetch_add(1, Ordering::Relaxed);

    let mut asked = ASKED.lock();
    // A question the window never received — one lost to a lagging
    // broadcast — has nobody left to answer it, and the client it belongs
    // to gave up when the server's own deadline passed. Swept here rather
    // than on a timer, because entries only ever pile up as new ones come.
    asked.retain(|_, entry| entry.at.elapsed() < APPLY_TIMEOUT);
    asked.insert(
        token,
        Asked {
            window,
            responder,
            at: Instant::now(),
        },
    );
    token
}

/// Answer the request this number names, if it is still waiting.
///
/// The client may have given up — the server stops waiting after its own
/// deadline — so an unknown number is ordinary rather than an error.
pub fn answer(token: u64, text: Option<String>) {
    let Some(asked) = ASKED.lock().remove(&token) else {
        return;
    };
    asked.responder.ok(SelectionResult {
        window: asked.window,
        text,
    });
}

/// Give up on the request this number names, saying why.
///
/// A refusal beats silence: the client would otherwise wait out the
/// server's deadline to be told the same thing with less of a reason.
pub fn abandon(token: u64, reason: &str) {
    let Some(asked) = ASKED.lock().remove(&token) else {
        return;
    };
    asked
        .responder
        .err(ResponseError::new(ErrorCode::InternalError, reason));
}

/// Let go of every question put to a window that is closing. Dropping the
/// responder releases the client now rather than at the server's deadline.
pub fn forget_window(window: WindowKey) {
    ASKED.lock().retain(|_, asked| asked.window != window);
}
