use arto_lsp::ReadySignal;
use dioxus::desktop::tao::window::WindowId;
use dioxus::desktop::window;
use dioxus::document;
use dioxus::prelude::*;
use tokio::sync::broadcast::error::RecvError;

use crate::events::REPORT_READY_IN_WINDOW;
use crate::ipc::ready;

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
    use_future(move || async move {
        let window_id = window().id();

        // Subscribed before anything is awaited, because a broadcast reaches
        // only the receivers that already exist: a request arriving while
        // this window is drawing would otherwise be announced to nobody and
        // sit in the registry until the window closed.
        let mut rx = REPORT_READY_IN_WINDOW.subscribe();

        let claimed = ready::claim_new_window();
        if !claimed.is_empty() {
            report_once_drawn(window_id, claimed).await;
        }

        loop {
            match rx.recv().await {
                Ok(target) if target == window_id => report_pending(window_id).await,
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
                    report_pending(window_id).await;
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

async fn report_pending(window_id: WindowId) {
    let pending = ready::take_pending(window_id);
    if !pending.is_empty() {
        report_once_drawn(window_id, pending).await;
    }
}

/// Wait for the page to finish drawing, then release `signals`.
///
/// The signals are held here rather than left in the registry so that a
/// request arriving mid-draw is not answered by a draw it never asked for.
/// Holding them also means a window torn down while this runs drops them,
/// which releases the launches rather than stranding them.
async fn report_once_drawn(window_id: WindowId, signals: Vec<ReadySignal>) {
    let drawn = document::eval(indoc::indoc! {r#"
        (async () => {
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

    let answered = tokio::time::timeout(DRAW_TIMEOUT, async move {
        let mut drawn = drawn;
        drawn.recv::<bool>().await
    })
    .await;

    match answered {
        Ok(Ok(true)) => tracing::debug!(?window_id, "Window drew; releasing the waiting launches"),
        Ok(Ok(false)) => tracing::warn!(
            ?window_id,
            "Renderer never appeared; releasing the waiting launches anyway"
        ),
        Ok(Err(error)) => tracing::warn!(
            ?window_id,
            %error,
            "Could not ask the page whether it had drawn; releasing the waiting launches anyway"
        ),
        Err(_) => tracing::warn!(
            ?window_id,
            "Window did not report a draw in time; releasing the waiting launches anyway"
        ),
    }
    for signal in signals {
        signal.fire();
    }
}
