use dioxus::document;
use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::keybindings::{Action, COMMAND_ACTIONS};
use crate::state::AppState;
use crate::visits::{Visit, VISITS, VISITS_CHANGED};

/// How many rows the palette will draw at once.
///
/// The list is a way back to something read recently, not a browser for the
/// whole history — that is the panel's Recent face, and the last row here
/// says so.
const MAX_ROWS: usize = 40;

/// One line in the palette.
///
/// Commands and documents share a list rather than sitting in two, because
/// the reader is answering one question — "what did I mean?" — and a query
/// that names a command should not have to be typed into a different box
/// from one that names a file.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// Something to do, named by [`Action::command_label`].
    Command(Action),
    /// Somewhere to go back to.
    Document(Visit),
}

/// Whether a command's name answers `query`.
///
/// The rule is [`crate::visits::matches`]'s: every whitespace-separated term
/// has to appear somewhere, case-insensitively. Sharing it means a query
/// behaves the same whichever kind of row it ends up finding.
pub fn command_matches(label: &str, query: &str) -> bool {
    let haystack = label.to_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

/// The commands answering `query`, in [`COMMAND_ACTIONS`] order.
///
/// An empty query answers with nothing: the palette opens on the history, and
/// a list of every command the app has would bury the two keystrokes that are
/// the point of opening it.
pub fn commands_for(query: &str) -> Vec<Action> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    COMMAND_ACTIONS
        .iter()
        .copied()
        .filter(|action| match action.command_label() {
            Some(label) => command_matches(label, query),
            None => false,
        })
        .collect()
}

/// The whole list for `query`: commands first, then documents.
///
/// Commands lead because a typed query that names one names it exactly, while
/// the documents below are the same history the palette opened on, narrowed.
pub fn rows_for(visits: &[Visit], query: &str) -> Vec<Row> {
    let commands = commands_for(query).into_iter().map(Row::Command);
    let documents = crate::visits::filter(visits, query)
        .into_iter()
        .cloned()
        .map(Row::Document);
    commands.chain(documents).take(MAX_ROWS).collect()
}

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

/// The quickest way back to something read recently, and the way to reach a
/// command by name.
///
/// It opens on the history rather than on an empty prompt: with nothing
/// typed the rows are the visits, newest first, so the common gesture is two
/// keys. Typing narrows the same list with [`crate::visits::matches`], which
/// is what the Recent face and the welcome page use, so a query that finds a
/// document in one of them finds it here — and puts above it any command
/// whose name the query also answers.
///
/// It is mounted only while it is open, so every opening starts with an
/// empty query and the cursor back on the previous document.
#[component]
pub fn Palette() -> Element {
    let mut state = use_context::<AppState>();
    let mut query = use_signal(String::new);
    let mut cursor = use_signal(|| usize::MAX);
    let mut composing = use_signal(|| false);
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
    let _ = state.visits_revision.read();
    let needle = query();
    let rows = {
        let visits = VISITS.read();
        rows_for(&visits.items, &needle)
    };

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

    let mut activate = move |index: usize| {
        let row = {
            let visits = VISITS.read();
            rows_for(&visits.items, &query()).get(index).cloned()
        };
        // Closing first matters: a command can put a modal file dialog or a
        // new window on screen, and the palette it was picked from has no
        // business still floating over that.
        close();
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
                        placeholder: "Go somewhere you have read, or name a command",
                        value: "{query}",
                        // Left alone while an input method is composing.
                        // This is a controlled field: every keystroke writes
                        // the value back, and writing it back mid-composition
                        // is what tears the composition up — the reader gets
                        // one letter of every word they type.
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
                            // Escape and the arrows are the IME's own keys —
                            // they confirm, cancel and choose a candidate.
                            // Taking them here would leave the reader unable
                            // to finish a word.
                            if evt.is_composing() {
                                return;
                            }
                            match evt.key() {
                                Key::Escape => {
                                    evt.prevent_default();
                                    close();
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
                }

                if rows.is_empty() {
                    div {
                        class: "palette-empty",
                        if needle.trim().is_empty() { "Nothing read yet" } else { "Nothing by that name" }
                    }
                }

                PaletteRows {
                    rows: rows.clone(),
                    current,
                    on_pick: activate,
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

/// The keys that reach `action` without the palette, if it has any.
///
/// Shown on the row so that a command found by name is also, once, a lesson
/// in the shortcut that would have found it faster.
fn shortcut_hint(action: Action) -> Option<String> {
    crate::keybindings::shortcut_hint_for_global_action(&action.to_string())
}

/// The directory a visit sits in, shortened the way the reader says it.
fn parent_label(visit: &Visit) -> String {
    crate::utils::paths::parent_label(&visit.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use std::path::PathBuf;

    fn visit(path: &str) -> Visit {
        Visit::new(PathBuf::from(path), Local::now())
    }

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

    #[test]
    fn an_empty_query_lists_the_history_alone() {
        let visits = vec![visit("/docs/README.md"), visit("/docs/CHANGELOG.md")];
        let rows = rows_for(&visits, "");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| matches!(row, Row::Document(_))));
    }

    #[test]
    fn a_query_puts_commands_above_documents() {
        let visits = vec![visit("/docs/printing.md")];
        let rows = rows_for(&visits, "print");
        assert_eq!(rows[0], Row::Command(Action::FilePrint));
        assert!(matches!(rows[1], Row::Document(_)));
    }

    #[test]
    fn every_term_has_to_appear() {
        assert!(command_matches("Close All Windows", "close windows"));
        assert!(command_matches("Close All Windows", "WINDOWS"));
        assert!(!command_matches("Close All Windows", "close tabs"));
    }

    #[test]
    fn commands_keep_their_listed_order() {
        let found = commands_for("window");
        let listed: Vec<Action> = COMMAND_ACTIONS
            .iter()
            .copied()
            .filter(|action| found.contains(action))
            .collect();
        assert_eq!(found, listed);
    }
}

/// The palette's rows: the commands a query names, then the documents it
/// finds.
///
/// Drawn here rather than in each screen that offers them, so that the list
/// the welcome page expands in place and the list `Cmd+K` floats over are the same
/// list — the same order, the same rows, the same cursor.
#[component]
pub fn PaletteRows(rows: Vec<Row>, current: usize, on_pick: EventHandler<usize>) -> Element {
    // The rows arrive commands first, documents after. Saying so with a word
    // above each is what makes a long list read as two short ones — the eye
    // stops looking for a command among the documents.
    let commands = rows
        .iter()
        .take_while(|row| matches!(row, Row::Command(_)))
        .count();

    rsx! {
        div {
            class: "palette-rows",
            for (index, row) in rows.iter().enumerate() {
                if index == 0 && commands > 0 {
                    div { class: "palette-group", "Commands" }
                }
                if index == commands && index < rows.len() {
                    div { class: "palette-group", "History" }
                }
                match row {
                    Row::Command(action) => rsx! {
                        div {
                            key: "command:{action}",
                            class: "palette-row",
                            class: if index == current { "current" },
                            onclick: move |_| on_pick.call(index),
                            Icon { name: IconName::Command, size: 14 }
                            span {
                                class: "palette-row-name",
                                "{action.command_label().unwrap_or_default()}"
                            }
                            span {
                                class: "palette-row-hint",
                                {shortcut_hint(*action).unwrap_or_default()}
                            }
                        }
                    },
                    Row::Document(visit) => rsx! {
                        div {
                            key: "document:{visit.path.display()}",
                            class: "palette-row",
                            class: if index == current { "current" },
                            title: "{visit.path.display()}",
                            onclick: move |_| on_pick.call(index),
                            Icon { name: IconName::File, size: 14 }
                            span {
                                class: "palette-row-name",
                                DocumentName { path: visit.path.clone() }
                            }
                            span { class: "palette-row-path", "{parent_label(visit)}" }
                        }
                    },
                }
            }
        }
    }
}
