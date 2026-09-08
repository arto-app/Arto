use chrono::Local;
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::bookmarks::{BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::palette::{step_cursor, PaletteRows, Row};
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
    let mut query = use_signal(String::new);
    let mut cursor = use_signal(|| 0usize);
    let mut focused = use_signal(|| false);
    let mut composing = use_signal(|| false);

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

    // Read so a recorded visit or a new place redraws; the numbers say nothing.
    let _ = revision();
    let _ = state.visits_revision.read();

    let now = Local::now();
    let palette_hint = crate::keybindings::shortcut_hint_for_global_action("palette.open");
    let needle = query();

    let visits = VISITS.read();
    let recent: Vec<Visit> = visits.items.iter().take(MAX_ROWS).cloned().collect();
    let more_recent = visits.items.len() > recent.len();
    let groups = crate::visits::group(&recent, now);
    // The same rows the palette shows, from the same function: what is typed
    // here and what is typed there answer alike.
    let rows = crate::components::palette::rows_for(&visits.items, &needle);
    let current = (*cursor.read()).min(rows.len().saturating_sub(1));
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

    let mut activate = move |index: usize| {
        let row = {
            let visits = VISITS.read();
            crate::components::palette::rows_for(&visits.items, &query())
                .get(index)
                .cloned()
        };
        // Cleared first: the page under the list comes back either way, and a
        // command can put a dialog on screen that a stale query has no
        // business sitting over.
        query.set(String::new());
        cursor.set(0);
        match row {
            Some(Row::Document(visit)) => state.open_file(&visit.path),
            Some(Row::Command(action)) => {
                crate::keybindings::dispatcher::dispatch_action(&action, state)
            }
            None => {}
        }
    };

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

                // The palette, expanded in place. Not a second way to find
                // a document: the same rows, in the same order, drawn by the
                // same component.
                //
                // It floats under the field instead of taking a place in the
                // page, and it is there from the moment the field is focused —
                // with nothing typed it is the history, which is what ⌘K opens
                // on too. A list that waited for a keystroke and shoved the
                // page about when it arrived would make the reader aim twice.
                div {
                    class: "welcome-search",

                    div {
                        class: "welcome-field",
                        Icon { name: IconName::Search, size: 14 }
                        input {
                            class: "welcome-input",
                            r#type: "text",
                            placeholder: "Find something you have read, or name a command",
                            value: "{query}",
                            onfocusin: move |_| focused.set(true),
                            onfocusout: move |_| focused.set(false),
                            // Left alone while an input method is composing:
                            // this is a controlled field, and writing the value
                            // back mid-composition tears the composition up.
                            oncompositionstart: move |_| composing.set(true),
                            oncompositionend: move |_| composing.set(false),
                            oninput: move |evt| {
                                if composing() {
                                    return;
                                }
                                query.set(evt.value());
                                cursor.set(0);
                            },
                            onkeydown: move |evt| {
                                // While an input method is composing, Enter,
                                // Escape and the arrows are the IME's own keys
                                // — they confirm, cancel and choose a
                                // candidate. Taking them here would leave the
                                // reader unable to finish a word.
                                if evt.is_composing() {
                                    return;
                                }
                                match evt.key() {
                                    Key::Escape => {
                                        evt.prevent_default();
                                        query.set(String::new());
                                    }
                                    Key::Enter => {
                                        evt.prevent_default();
                                        activate(current);
                                    }
                                    Key::ArrowDown => {
                                        evt.prevent_default();
                                        cursor.set(step_cursor(current, rows.len(), true));
                                    }
                                    Key::ArrowUp => {
                                        evt.prevent_default();
                                        cursor.set(step_cursor(current, rows.len(), false));
                                    }
                                    _ => {}
                                }
                            },
                        }
                        if let Some(hint) = palette_hint {
                            kbd { class: "welcome-field-hint", "{hint}" }
                        }
                    }

                    if focused() {
                        div {
                            class: "welcome-suggest",
                            // The press that picks a row would blur the field
                            // first, and the list would be gone before the
                            // click landed on anything.
                            onmousedown: move |evt| evt.prevent_default(),

                            if rows.is_empty() {
                                p { class: "welcome-empty", "Nothing by that name" }
                            } else {
                                PaletteRows {
                                    rows: rows.clone(),
                                    current,
                                    on_pick: activate,
                                }
                            }
                        }
                    }
                }

                div {
                    class: "welcome-columns",

                    div {
                        class: "welcome-column",
                        h2 { class: "welcome-heading", "Recent" }

                        if groups.is_empty() {
                            p {
                                class: "welcome-empty",
                                "Open a Markdown file, or drop one here, and it will be waiting next time."
                            }
                        }

                        for (bucket, entries) in groups {
                            div { class: "welcome-group", "{bucket.heading()}" }
                            for visit in entries {
                                div {
                                    key: "{visit.path.display()}",
                                    class: "welcome-row",
                                    title: "{visit.path.display()}",
                                    onclick: {
                                        let path = visit.path.clone();
                                        move |_| state.open_file(&path)
                                    },
                                    Icon { name: IconName::File, size: 14 }
                                    span {
                                        class: "welcome-row-name",
                                        DocumentName { path: visit.path.clone() }
                                    }
                                    span {
                                        class: "welcome-row-when",
                                        "{crate::visits::short_when(visit.at, now)}"
                                    }
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

                    div {
                        class: "welcome-column welcome-column-side",

                        if !starred.is_empty() {
                            div {
                                h2 { class: "welcome-heading", "Starred" }
                                for path in starred {
                                    div {
                                        key: "{path.display()}",
                                        class: "welcome-row",
                                        title: "{path.display()}",
                                        onclick: {
                                            let path = path.clone();
                                            move |_| state.open_file(&path)
                                        },
                                        Icon { name: IconName::File, size: 14 }
                                        span {
                                            class: "welcome-row-name",
                                            DocumentName { path: path.clone() }
                                        }
                                        if let Some(at) = crate::visits::last_read(&path) {
                                            span {
                                                class: "welcome-row-when",
                                                "{crate::visits::short_when(at, now)}"
                                            }
                                        }
                                    }
                                }

                                if more_starred {
                                    button {
                                        class: "welcome-more",
                                        onclick: move |_| state.show_face(Face::Starred),
                                        "All starred…"
                                    }
                                }
                            }
                        }

                        div {
                        h2 { class: "welcome-heading", "Places" }

                        if places.is_empty() {
                            p {
                                class: "welcome-empty",
                                "Star a folder and it will be here, in every window."
                            }
                        }

                        for place in places {
                            div {
                                key: "{place.display()}",
                                class: "welcome-row",
                                title: "{place.display()}",
                                onclick: {
                                    let place = place.clone();
                                    move |_| state.add_root(&place)
                                },
                                Icon { name: IconName::Folder, size: 14 }
                                span {
                                    class: "welcome-row-name",
                                    DocumentName { path: place.clone() }
                                }
                                if let Some(at) = crate::visits::last_read_under(&place) {
                                    span {
                                        class: "welcome-row-when",
                                        "{crate::visits::short_when(at, now)}"
                                    }
                                }
                            }
                        }

                        if more_places {
                            button {
                                class: "welcome-more",
                                onclick: move |_| state.show_face(Face::Files),
                                "All places…"
                            }
                        }
                        }
                    }
                }
            }
        }
    }
}
