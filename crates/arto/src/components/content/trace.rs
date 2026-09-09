use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
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
pub fn MarginTrace(count: usize, visible: bool) -> Element {
    let mut state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit redraws the trace; the numbers say nothing.
    // Two of them: the broadcast carries what other windows did, and the
    // window's own signal carries what this one did, in the same frame.
    let _ = revision();
    let _ = state.visits_revision.read();

    let current = state.current_file();
    let rows: Vec<Visit> = {
        let visits = VISITS.read();
        // Documents, not days: the history keeps a row per day a document was
        // read, and the same name twice in a column of five is one document
        // taking two of the five places it has.
        crate::visits::documents(&visits.items)
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
            class: if !visible { "away" },
            "aria-label": "Recently read",
            "aria-hidden": if visible { "false" } else { "true" },

            div { class: "margin-trace-label", "Recent" }

            for visit in rows {
                button {
                    key: "{visit.path.display()}",
                    class: "margin-trace-row",
                    title: "{visit.path.display()}",
                    onclick: {
                        let path = visit.path.clone();
                        move |_| state.open_file(&path)
                    },
                    DocumentName { path: visit.path.clone() }
                }
            }
        }
    }
}
