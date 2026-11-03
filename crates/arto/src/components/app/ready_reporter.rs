use std::path::{Path, PathBuf};

use arto_lsp::AppliedResult;
use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::window;
use dioxus::document;
use dioxus::prelude::*;
use tokio::sync::broadcast::error::RecvError;

use crate::events::REPORT_READY_IN_WINDOW;
use crate::ipc::ready;
use crate::state::AppState;

/// How long this window will wait for the page to say it has drawn.
///
/// Only a backstop. The waiting launch has a longer bound of its own, so
/// giving up here answers it a little early rather than leaving it to time
/// out — which is the better of the two, because the request was applied
/// either way and the launch should hear so.
const DRAW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Answer the launches that are waiting for this window to draw.
///
/// Two ways a window comes to owe an answer, and both are handled here. A
/// window created to satisfy a request claims what was parked for it as it
/// mounts, because nothing else could have been meant. A window that was
/// already open is told over [`REPORT_READY_IN_WINDOW`], since a request
/// that only moved or repainted it leaves nothing for it to notice.
pub(super) fn setup_ready_reporter() {
    let state = use_context::<AppState>();
    use_future(move || async move {
        let window_id = window().id();

        // Subscribed before anything is awaited, because a broadcast reaches
        // only the receivers that already exist: a request arriving while
        // this window is drawing would otherwise be announced to nobody and
        // sit in the registry until the window closed.
        let mut rx = REPORT_READY_IN_WINDOW.subscribe();

        // What a new window shows is what the request that created it asked
        // for, so its claimed responders are all waiting for that file.
        let claimed = ready::claim_new_window();
        if !claimed.is_empty() {
            let file = shown_file(state);
            let claimed = claimed
                .into_iter()
                .map(|responder| (file.clone(), responder))
                .collect();
            report_once_drawn(state, window_id, claimed).await;
        }

        loop {
            match rx.recv().await {
                Ok(target) if target == window_id => report_pending(state, window_id).await,
                Ok(_) => {}
                // Falling behind loses messages, not requests: what is
                // waiting lives in the registry, and the message is only a
                // nudge to go and look. So look, and keep listening — the
                // alternative is a window that never answers again.
                Err(RecvError::Lagged(missed)) => {
                    tracing::debug!(
                        ?window_id,
                        missed,
                        "Missed ready notifications; checking the registry instead"
                    );
                    report_pending(state, window_id).await;
                }
                Err(RecvError::Closed) => break,
            }
        }
    });

    // A window that closes before it draws still owes an answer, and the
    // honest one is "no window" rather than silence until the launch's own
    // wait runs out.
    use_drop(move || ready::forget_window(window().id()));
}

async fn report_pending(state: AppState, window_id: WindowId) {
    let pending = ready::take_pending(window_id);
    if !pending.is_empty() {
        report_once_drawn(state, window_id, pending).await;
    }
}

/// Wait for the page to finish drawing, then release what is `waiting`.
///
/// The responders are held here rather than left in the registry so that a
/// request arriving mid-draw is not answered by a draw it never asked for.
/// Holding them also means a window torn down while this runs drops them,
/// which releases the launches rather than stranding them.
async fn report_once_drawn(state: AppState, window_id: WindowId, waiting: Vec<ready::Waiting>) {
    let file = shown_file(state);
    let (responders, superseded) = ready::split_by_shown_file(waiting, file.as_deref());
    if !superseded.is_empty() {
        tracing::debug!(
            ?window_id,
            superseded = superseded.len(),
            "Requests whose file was replaced before the window drew it"
        );
    }
    for responder in superseded {
        responder.ok(AppliedResult { ready: false });
    }
    if responders.is_empty() {
        return;
    }

    // Awaited, not left running: an eval counts as finished once its own
    // body returns, and a finished one takes nothing more from `send`.
    let mut drawn = document::eval(indoc::indoc! {r#"
        await (async () => {
            const file = await dioxus.recv();
            // The renderer bundle arrives after the first frame, and a cold
            // start can be a second or two behind this.
            for (let attempt = 0; attempt < 100; attempt++) {
                if (window.Arto?.render?.onComplete) break;
                await new Promise(r => setTimeout(r, 50));
            }
            if (!window.Arto?.render?.onComplete) {
                dioxus.send(false);
                return;
            }
            // A file is read and rendered by a task of its own, so a window
            // that has just opened is still showing an empty page here, and
            // a render of that would answer for a document not yet in it.
            // `FileViewer` marks the page with the file once its HTML is in,
            // and the error page marks itself with the file it stands for:
            // an error page left over from the previous file is not this
            // one arriving. Only those marks count: every document is drawn
            // inside a `.markdown-body`, and raw HTML in the one still on
            // screen may carry the same attribute.
            const arrived = () =>
                [...document.querySelectorAll("[data-arto-loaded]")].some(
                    (element) =>
                        element.dataset.artoLoaded === file &&
                        !element.parentElement?.closest(".markdown-body"),
                );
            // Bounded like the wait on the Rust side, which gives up on this
            // script without stopping it.
            for (let attempt = 0; file !== null && !arrived() && attempt < 400; attempt++) {
                await new Promise(r => setTimeout(r, 50));
            }
            // A callback fires on the next batch render, and a batch render
            // happens when the document changes. Asking for one is what
            // makes this answer a document that is already settled — a
            // window that was only moved, or one showing the welcome page.
            await new Promise(resolve => {
                window.Arto.render.onComplete(resolve);
                window.Arto.render.schedule();
            });
            dioxus.send(true);
        })();
    "#});

    let marked = file
        .as_ref()
        .map(|file| file.to_string_lossy().into_owned());
    let answered = match drawn.send(marked) {
        Ok(()) => {
            tokio::time::timeout(DRAW_TIMEOUT, async {
                tokio::select! {
                    drew = drawn.recv::<bool>() => Some(drew),
                    () = moved_on(state, file) => None,
                }
            })
            .await
        }
        Err(error) => Ok(Some(Err(error))),
    };

    // Only the page saying so counts as drawn. Every other outcome releases
    // the waiting launches — leaving them to time out helps nobody — but
    // releases them with `ready: false`, because the one thing `--wait-ready`
    // rules out is reporting success for a window that never drew.
    let drew = match answered {
        Ok(Some(Ok(true))) => {
            tracing::debug!(?window_id, "Window drew; releasing the waiting launches");
            true
        }
        Ok(Some(Ok(false))) => {
            tracing::warn!(?window_id, "Renderer never appeared");
            false
        }
        // The file asked for was replaced before it arrived, so it will
        // never be drawn; waiting on would only hold up the request that
        // replaced it, which is answered next.
        Ok(None) => {
            tracing::debug!(?window_id, "Window moved on before drawing the request");
            false
        }
        Ok(Some(Err(error))) => {
            tracing::warn!(?window_id, %error, "Could not ask the page whether it had drawn");
            false
        }
        Err(_) => {
            tracing::warn!(?window_id, "Window did not report a draw in time");
            false
        }
    };
    for responder in responders {
        responder.ok(AppliedResult { ready: drew });
    }
}

fn shown_file(state: AppState) -> Option<PathBuf> {
    state.document.peek().file().map(Path::to_path_buf)
}

/// Resolves once the window shows another file than `file`.
async fn moved_on(state: AppState, file: Option<PathBuf>) {
    // Polled: the document is a signal, which offers nothing to await.
    while shown_file(state) == file {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
