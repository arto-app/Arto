use dioxus::desktop::window;
use dioxus::document;
use dioxus::prelude::*;

use crate::events::READ_SELECTION_IN_WINDOW;
use arto_lsp::SelectionResult;

/// Answer `arto/selection` for this window.
///
/// The selection is the one thing a client can ask for that is not already
/// on the main thread, so this is a round trip into the WebView rather than
/// a read — and `document::eval` only reaches it from the window's own
/// scope.
pub(super) fn setup_selection_listener() {
    use_future(move || async move {
        let window_id = window().id();
        let mut rx = READ_SELECTION_IN_WINDOW.subscribe();

        loop {
            match rx.recv().await {
                Ok((target_window, token)) if target_window == window_id => {
                    // Bounded, because this loop cannot read the next
                    // question until this one is answered: a page that
                    // never replies would take every later request with
                    // it, not only its own.
                    match tokio::time::timeout(SELECTION_TIMEOUT, read_selection()).await {
                        Ok(text) => crate::ipc::answer_parked(token, |window| SelectionResult {
                            window,
                            text,
                        }),
                        Err(_) => {
                            crate::ipc::abandon_parked(token, "the window did not answer");
                        }
                    }
                }
                Ok(_) => {}
                // The client is released by the server's own deadline, so a
                // dropped question costs a wait rather than an answer.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::debug!(?window_id, missed, "Missed selection requests");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// What the reader has selected, or `None` when the selection is empty.
///
/// An empty string and no selection at all are the same thing to a client,
/// and collapsing them here is what lets the answer say "nothing" once.
async fn read_selection() -> Option<String> {
    let mut eval = document::eval(SELECTION_JS);
    let text = eval.recv::<String>().await.ok()?;
    (!text.is_empty()).then_some(text)
}

const SELECTION_JS: &str = "(() => { dioxus.send(window.getSelection()?.toString() ?? ''); })()";

/// Shorter than the server's own deadline, so a page that does not answer
/// is reported as such rather than left to look like the app hanging.
const SELECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
