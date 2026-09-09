use chrono::Local;
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::bookmarks::{BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::state::{AppState, Face};
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many documents the welcome page offers before deferring to the Recent face.
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
pub fn WelcomeView() -> Element {
    let mut state = use_context::<AppState>();
    let mut revision = use_signal(|| 0u32);

    // Anything this page lists, changing anywhere: a document read in another
    // window, a folder starred in this one.
    use_future(move || async move {
        let mut visited = VISITS_CHANGED.subscribe();
        let mut bookmarked = BOOKMARKS_CHANGED.subscribe();
        loop {
            let changed = tokio::select! {
                result = visited.recv() => result.is_ok(),
                result = bookmarked.recv() => result.is_ok(),
            };
            if !changed {
                break;
            }
            *revision.write() += 1;
        }
    });

    // Read so a recorded visit or a new place redraws; the numbers say nothing.
    let _ = revision();
    let _ = state.visits_revision.read();

    let now = Local::now();
    let palette_hint = crate::keybindings::shortcut_hint_for_global_action("palette.open");

    let visits = VISITS.read();
    let recent: Vec<Visit> = visits.items.iter().take(MAX_ROWS).cloned().collect();
    let more_recent = visits.items.len() > recent.len();
    let groups = crate::visits::group(&recent, now);
    let bookmarks = BOOKMARKS.read();
    let all_places = bookmarks.places();
    let places: Vec<PathBuf> = all_places.iter().take(MAX_ROWS).cloned().collect();
    let more_places = all_places.len() > places.len();
    // A starred folder is a place, and the places are listed as places.
    let all_starred: Vec<PathBuf> = bookmarks
        .items
        .iter()
        .filter(|bookmark| !bookmark.is_dir())
        .map(|bookmark| bookmark.path.clone())
        .collect();
    let starred: Vec<PathBuf> = all_starred.iter().take(MAX_ROWS).cloned().collect();
    let more_starred = all_starred.len() > starred.len();
    drop(bookmarks);

    rsx! {
        div {
            class: "welcome",

            div {
                class: "welcome-content",

                // The app's mark. A window with no document in it is the one
                // screen with nothing for it to take attention away from.
                div {
                    class: "welcome-mark",
                    img {
                        class: "welcome-mark-image light",
                        src: crate::assets::welcome_mark_data_url(false),
                        alt: "Arto",
                    }
                    img {
                        class: "welcome-mark-image dark",
                        src: crate::assets::welcome_mark_data_url(true),
                        alt: "Arto",
                    }
                    p { class: "welcome-tagline", "The Art of Reading Markdown" }
                }

                // The way in is a keystroke, said once. A field here would be
                // a second one to aim at, and it would answer with the same
                // list the palette already floats over whatever is on screen.
                p {
                    class: "welcome-hint",
                    if let Some(hint) = palette_hint {
                        "Press "
                        kbd { class: "welcome-key", "{hint}" }
                        " to find something you have read, or to name a command."
                    } else {
                        "Open the command palette to find something you have read, or to name a command."
                    }
                }

                div {
                    class: "welcome-columns",

                    // What is kept and what is worked in, on the left: two
                    // short lists that answer "where do I go" rather than
                    // "what was I doing".
                    div {
                        class: "welcome-column welcome-column-side",

                        div {
                            h2 {
                                class: "welcome-heading",
                                Icon { name: IconName::Folder, size: 12 }
                                "Places"
                            }

                            if places.is_empty() {
                                p {
                                    class: "welcome-empty",
                                    "Star a folder and it will be here, in every window."
                                }
                            }

                            for place in places {
                                WelcomeRow {
                                    key: "{place.display()}",
                                    path: place.clone(),
                                    icon: IconName::Folder,
                                    when: when_read(crate::visits::last_read_under(&place), now),
                                    on_pick: move |place: PathBuf| state.add_root(&place),
                                }
                            }

                            if more_places {
                                button {
                                    class: "welcome-more",
                                    onclick: move |_| state.show_face(Face::Places),
                                    "All places…"
                                }
                            }
                        }

                        if !starred.is_empty() {
                            div {
                                h2 {
                                    class: "welcome-heading",
                                    Icon { name: IconName::Star, size: 12 }
                                    "Starred"
                                }

                                for path in starred {
                                    WelcomeRow {
                                        key: "{path.display()}",
                                        path: path.clone(),
                                        icon: IconName::File,
                                        when: when_read(crate::visits::last_read(&path), now),
                                        on_pick: move |path: PathBuf| state.open_file(&path),
                                    }
                                }

                                if more_starred {
                                    button {
                                        class: "welcome-more",
                                        onclick: move |_| state.show_face(Face::Starred),
                                        "All stars…"
                                    }
                                }
                            }
                        }
                    }

                    // What has been read, on the right: the longest list, and
                    // the one a reader scans rather than picks from.
                    div {
                        class: "welcome-column",
                        h2 {
                            class: "welcome-heading",
                            Icon { name: IconName::History, size: 12 }
                            "Recent"
                        }

                        if groups.is_empty() {
                            p {
                                class: "welcome-empty",
                                "Open a Markdown file, or drop one here, and it will be waiting next time."
                            }
                        }

                        for (bucket, entries) in groups {
                            div { class: "welcome-group", "{bucket.heading()}" }
                            for visit in entries {
                                WelcomeRow {
                                    key: "{visit.path.display()}",
                                    path: visit.path.clone(),
                                    icon: IconName::File,
                                    when: crate::visits::short_when(visit.at, now),
                                    on_pick: move |path: PathBuf| state.open_file(&path),
                                }
                            }
                        }

                        // The list is a landing, not a browser: what does not
                        // fit is one row away, in the face that holds it all.
                        if more_recent {
                            button {
                                class: "welcome-more",
                                onclick: move |_| state.show_face(Face::Recent),
                                "All history…"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// When something was last read, worded for a row, or nothing at all.
fn when_read(at: Option<chrono::DateTime<Local>>, now: chrono::DateTime<Local>) -> String {
    at.map(|at| crate::visits::short_when(at, now))
        .unwrap_or_default()
}

/// One offer on the landing page: a document to read, or a folder to work in.
///
/// Which of the two it is shows in its glyph and in what picking it does; the
/// row itself is the same, because all three columns are lists of one thing to
/// go to next.
#[component]
fn WelcomeRow(
    path: PathBuf,
    icon: IconName,
    when: String,
    on_pick: EventHandler<PathBuf>,
) -> Element {
    rsx! {
        div {
            class: "welcome-row",
            title: "{path.display()}",
            onclick: {
                let path = path.clone();
                move |_| on_pick.call(path.clone())
            },
            Icon { name: icon, size: 14 }
            span {
                class: "welcome-row-name",
                DocumentName { path: path.clone() }
            }
            if !when.is_empty() {
                span { class: "welcome-row-when", "{when}" }
            }
        }
    }
}
