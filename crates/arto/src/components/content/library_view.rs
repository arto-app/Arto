use chrono::Local;
use dioxus::prelude::*;

use crate::bookmarks::{BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::icon::{Icon, IconName};
use crate::state::AppState;
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many documents the library offers before deferring to the Recent face.
///
/// A landing screen is for picking up where reading left off, not for
/// browsing years; the panel's face is the browser.
const MAX_ROWS: usize = 24;

/// What a window shows when it is not showing a document.
///
/// A window with nothing open used to explain that nothing was open. This
/// says instead what there is to read: the documents last read, and the
/// places kept. It is the same history the palette and the Recent face draw,
/// narrowed by the same [`crate::visits::matches`], so the filter field here
/// behaves exactly like the palette's.
#[component]
pub fn LibraryView() -> Element {
    let mut state = use_context::<AppState>();
    let mut query = use_signal(String::new);
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    use_future(move || async move {
        let mut rx = BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit or a new place redraws; the number says nothing.
    let _ = revision();

    let needle = query();
    let visits = VISITS.read();
    let rows: Vec<Visit> = crate::visits::filter(&visits.items, &needle)
        .into_iter()
        .take(MAX_ROWS)
        .cloned()
        .collect();
    let groups = crate::visits::group(&rows, Local::now());
    let places = BOOKMARKS.read().places();

    rsx! {
        div {
            class: "library",

            div {
                class: "library-content",

                div {
                    class: "library-field",
                    Icon { name: IconName::Search, size: 14 }
                    input {
                        class: "library-input",
                        r#type: "text",
                        placeholder: "Find something you have read",
                        value: "{query}",
                        oninput: move |evt| query.set(evt.value()),
                    }
                }

                div {
                    class: "library-columns",

                    div {
                        class: "library-column",
                        h2 { class: "library-heading", "Recent" }

                        if groups.is_empty() {
                            p {
                                class: "library-empty",
                                "Open a Markdown file, or drop one here, and it will be waiting next time."
                            }
                        }

                        for (bucket, entries) in groups {
                            div { class: "library-group", "{bucket.heading()}" }
                            for visit in entries {
                                div {
                                    key: "{visit.path.display()}",
                                    class: "library-row",
                                    title: "{visit.path.display()}",
                                    onclick: {
                                        let path = visit.path.clone();
                                        move |_| state.open_file(&path)
                                    },
                                    Icon { name: IconName::File, size: 14 }
                                    span { class: "library-row-name", "{visit.display_name()}" }
                                }
                            }
                        }
                    }

                    div {
                        class: "library-column library-column-places",
                        h2 { class: "library-heading", "Places" }

                        if places.is_empty() {
                            p {
                                class: "library-empty",
                                "Star a folder and it will be here, in every window."
                            }
                        }

                        for place in places {
                            div {
                                key: "{place.display()}",
                                class: "library-row",
                                title: "{place.display()}",
                                onclick: {
                                    let place = place.clone();
                                    move |_| state.add_root(&place)
                                },
                                Icon { name: IconName::Folder, size: 14 }
                                span {
                                    class: "library-row-name",
                                    "{place.file_name().unwrap_or(place.as_os_str()).to_string_lossy()}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
