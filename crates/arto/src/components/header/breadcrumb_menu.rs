use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::state::{AppState, Face};
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many documents the breadcrumb drops.
///
/// Enough to cover a morning's reading; the last row goes to the face that
/// holds the rest, so this list never has to grow into a browser.
const MAX_ROWS: usize = 12;

/// The name of what is being read, and the way back to what was read before.
///
/// The header used to state the file name and stop there. The name is also
/// the natural place to ask "what else have I had open", so it became the
/// control that answers: clicking it drops the recent documents, and the last
/// row opens the panel's Recent face with all of them.
#[component]
pub fn Breadcrumb(label: String) -> Element {
    let mut state = use_context::<AppState>();
    let mut is_open = use_signal(|| false);
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit redraws the list; the number says nothing.
    let _ = revision();

    let current = state.current_file();
    let rows: Vec<Visit> = {
        let visits = VISITS.read();
        visits
            .items
            .iter()
            // What is on screen is not somewhere to go back to.
            .filter(|visit| current.as_deref() != Some(visit.path.as_path()))
            .take(MAX_ROWS)
            .cloned()
            .collect()
    };

    rsx! {
        div {
            class: "breadcrumb",

            button {
                class: "breadcrumb-label",
                class: if is_open() { "open" },
                title: "Recently read",
                onclick: move |_| is_open.toggle(),
                span { class: "file-name", "{label}" }
                Icon { name: IconName::ChevronDown, size: 12 }
            }

            if is_open() {
                // Catches the click that closes the menu, so the rows below
                // do not have to guess where the pointer went.
                div {
                    class: "breadcrumb-backdrop",
                    onclick: move |_| is_open.set(false),
                }

                div {
                    class: "breadcrumb-menu",

                    if rows.is_empty() {
                        div { class: "breadcrumb-empty", "Nothing else read yet" }
                    }

                    for visit in rows {
                        div {
                            key: "{visit.path.display()}",
                            class: "breadcrumb-row",
                            title: "{visit.path.display()}",
                            onclick: {
                                let path = visit.path.clone();
                                move |_| {
                                    state.open_file(&path);
                                    is_open.set(false);
                                }
                            },
                            Icon { name: IconName::File, size: 14 }
                            span { class: "breadcrumb-row-name", "{visit.display_name()}" }
                        }
                    }

                    div {
                        class: "breadcrumb-row breadcrumb-row-all",
                        onclick: move |_| {
                            state.show_face(Face::Recent);
                            is_open.set(false);
                        },
                        Icon { name: IconName::History, size: 14 }
                        span { class: "breadcrumb-row-name", "All history…" }
                    }
                }
            }
        }
    }
}
