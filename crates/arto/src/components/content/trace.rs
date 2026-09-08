use dioxus::prelude::*;

use crate::state::AppState;
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// The documents read just before this one, left in the margin.
///
/// The other three windows on the history are asked for; this one is simply
/// there, at the edge of the page, the way a bookmark ribbon is. It costs a
/// column of margin the document was not using and answers "what was I
/// reading before" without a keystroke.
///
/// It is the one window a reader can turn off, because it is the one that is
/// never asked for — the choice is `sidebar.recentTrace` and the widths it
/// yields to are Phase 5's; until then it appears when the panel is not out,
/// which is the default that setting names.
#[component]
pub fn MarginTrace(count: usize) -> Element {
    let mut state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit redraws the trace; the number says nothing.
    let _ = revision();

    let current = state
        .current_tab()
        .and_then(|tab| tab.file().map(|file| file.to_path_buf()));
    let rows: Vec<Visit> = {
        let visits = VISITS.read();
        visits
            .items
            .iter()
            // What is on screen is not a trace of where the reader has been.
            .filter(|visit| current.as_deref() != Some(visit.path.as_path()))
            .take(count)
            .cloned()
            .collect()
    };

    if rows.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "margin-trace",
            "aria-label": "Recently read",

            for visit in rows {
                button {
                    key: "{visit.path.display()}",
                    class: "margin-trace-row",
                    title: "{visit.path.display()}",
                    onclick: {
                        let path = visit.path.clone();
                        move |_| state.open_file(&path)
                    },
                    "{visit.display_name()}"
                }
            }
        }
    }
}
