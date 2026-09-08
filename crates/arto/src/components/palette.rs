use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::state::AppState;
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many rows the palette will draw at once.
///
/// The list is a way back to something read recently, not a browser for the
/// whole history — that is the panel's Recent face, and the last row here
/// says so.
const MAX_ROWS: usize = 40;

/// Where the cursor sits when the palette opens.
///
/// The first row is the document already on screen, so resting on it would
/// make the quickest gesture in the app — open, press return — do nothing.
/// The second row is the one before it, which is what "take me back" means.
pub fn initial_cursor(len: usize) -> usize {
    if len > 1 {
        1
    } else {
        0
    }
}

/// Move the cursor by one row, wrapping at both ends.
///
/// Wrapping costs nothing here: the list is short, and a reader holding the
/// key down should not have to notice where it stops.
pub fn step_cursor(cursor: usize, len: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if forward {
        (cursor + 1) % len
    } else if cursor == 0 {
        len - 1
    } else {
        cursor - 1
    }
}

/// The quickest way back to something read recently.
///
/// It opens on the history rather than on an empty prompt: with nothing
/// typed the rows are the visits, newest first, so the common gesture is two
/// keys. Typing narrows the same list with [`crate::visits::matches`], which
/// is what the Recent face and the library use, so a query that finds a
/// document in one of them finds it here.
///
/// It is mounted only while it is open, so every opening starts with an
/// empty query and the cursor back on the previous document.
#[component]
pub fn Palette() -> Element {
    let mut state = use_context::<AppState>();
    let mut query = use_signal(String::new);
    let mut cursor = use_signal(|| usize::MAX);
    let mut revision = use_signal(|| 0u32);

    use_future(move || async move {
        let mut rx = VISITS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            *revision.write() += 1;
        }
    });

    // Focus the input as it appears: the palette is a keyboard gesture, and
    // one that asked for a click first would not be worth the keystroke.
    use_effect(move || {
        spawn(async {
            let _ = document::eval("document.querySelector('.palette-input')?.focus()").await;
        });
    });

    // Read so a recorded visit redraws the list; the number says nothing.
    let _ = revision();
    let needle = query();
    let visits = VISITS.read();
    let rows: Vec<Visit> = crate::visits::filter(&visits.items, &needle)
        .into_iter()
        .take(MAX_ROWS)
        .cloned()
        .collect();

    // The cursor rests on the second row on the first draw and is clamped
    // afterwards, so narrowing the list can never leave it past the end.
    let current = if *cursor.read() == usize::MAX {
        initial_cursor(rows.len())
    } else {
        (*cursor.read()).min(rows.len().saturating_sub(1))
    };

    let mut close = move || {
        state.palette_open.set(false);
    };

    let mut open_row = move |index: usize| {
        let path = {
            let visits = VISITS.read();
            crate::visits::filter(&visits.items, &query())
                .get(index)
                .map(|visit| visit.path.clone())
        };
        if let Some(path) = path {
            state.open_file(&path);
        }
        close();
    };

    rsx! {
        div {
            class: "palette-backdrop",
            onclick: move |_| close(),

            div {
                class: "palette",
                onclick: move |evt| evt.stop_propagation(),

                div {
                    class: "palette-field",
                    Icon { name: IconName::Search, size: 14 }
                    input {
                        class: "palette-input",
                        r#type: "text",
                        placeholder: "Go to a document you have read",
                        value: "{query}",
                        oninput: move |evt| {
                            query.set(evt.value());
                            cursor.set(0);
                        },
                        onkeydown: move |evt| {
                            match evt.key() {
                                Key::Escape => {
                                    evt.prevent_default();
                                    close();
                                }
                                Key::Enter => {
                                    evt.prevent_default();
                                    open_row(current);
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
                }

                if rows.is_empty() {
                    div { class: "palette-empty", "Nothing read yet" }
                }

                div {
                    class: "palette-rows",
                    for (index, visit) in rows.iter().enumerate() {
                        div {
                            key: "{visit.path.display()}",
                            class: "palette-row",
                            class: if index == current { "current" },
                            title: "{visit.path.display()}",
                            onclick: move |_| open_row(index),
                            Icon { name: IconName::File, size: 14 }
                            span { class: "palette-row-name", "{visit.display_name()}" }
                            span { class: "palette-row-path", "{parent_label(visit)}" }
                        }
                    }
                }

                div {
                    class: "palette-row palette-row-all",
                    onclick: move |_| {
                        state.show_face(crate::state::Face::Recent);
                        close();
                    },
                    Icon { name: IconName::History, size: 14 }
                    span { class: "palette-row-name", "All history…" }
                }
            }
        }
    }
}

/// The directory a visit sits in, shortened to the home-relative form a
/// reader recognises.
fn parent_label(visit: &Visit) -> String {
    let Some(parent) = visit.path.parent() else {
        return String::new();
    };
    let parent = parent.to_string_lossy().to_string();
    match dirs::home_dir() {
        Some(home) => {
            let home = home.to_string_lossy().to_string();
            match parent.strip_prefix(&home) {
                Some(rest) => format!("~{rest}"),
                None => parent,
            }
        }
        None => parent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_opens_on_the_previous_document() {
        assert_eq!(initial_cursor(0), 0);
        assert_eq!(initial_cursor(1), 0);
        assert_eq!(initial_cursor(2), 1);
        assert_eq!(initial_cursor(30), 1);
    }

    #[test]
    fn stepping_wraps_at_both_ends() {
        assert_eq!(step_cursor(0, 3, true), 1);
        assert_eq!(step_cursor(2, 3, true), 0);
        assert_eq!(step_cursor(0, 3, false), 2);
        assert_eq!(step_cursor(1, 3, false), 0);
    }

    #[test]
    fn stepping_an_empty_list_stays_put() {
        assert_eq!(step_cursor(0, 0, true), 0);
        assert_eq!(step_cursor(5, 0, false), 0);
    }
}
