use dioxus::document;
use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::keybindings::{Action, COMMAND_ACTIONS};
use crate::state::AppState;
use crate::visits::{Visit, VISITS, VISITS_CHANGED};
use std::path::PathBuf;

/// How many rows the palette will draw at once.
///
/// The list is a way back to something read recently, not a browser for the
/// whole history — that is the panel's Recent face, and the last row here
/// says so.
const MAX_ROWS: usize = 40;

/// One line in the palette.
///
/// Everything the reader can reach shares a list rather than sitting in four,
/// because they are answering one question — "what did I mean?" — and a query
/// that names a command should not have to be typed into a different box from
/// one that names a folder.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// Something to do, named by [`Action::command_label`].
    Command(Action),
    /// Somewhere to go back to.
    Document(Visit),
    /// A document that was kept, whether or not it has been read lately.
    Starred(PathBuf),
    /// A folder to work in.
    Place(PathBuf),
}

impl Row {
    /// The heading this row sits under.
    ///
    /// Four short lists read faster than one long one: the eye stops looking
    /// for a command among the documents.
    fn group(&self) -> &'static str {
        match self {
            Self::Command(_) => "Commands",
            Self::Document(_) => "Recent",
            Self::Starred(_) => "Starred",
            Self::Place(_) => "Places",
        }
    }
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

/// The whole list for `query`: commands, the history, the stars, the places.
///
/// Commands lead because a typed query that names one names it exactly. The
/// history follows, because that is what the palette opens on and what it is
/// mostly for. The stars and the places come last and only once something is
/// typed: they are short lists the reader already knows by heart, so putting
/// them above an untyped history would push the answer down the screen for
/// nothing.
pub fn rows_for(
    visits: &[Visit],
    starred: &[PathBuf],
    places: &[PathBuf],
    query: &str,
) -> Vec<Row> {
    let named = |paths: &[PathBuf]| -> Vec<PathBuf> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        paths
            .iter()
            .filter(|path| crate::visits::matches_path(path, query))
            .cloned()
            .collect()
    };

    let commands = commands_for(query).into_iter().map(Row::Command);
    let documents = crate::visits::filter(visits, query)
        .into_iter()
        .cloned()
        .map(Row::Document);
    let stars = named(starred).into_iter().map(Row::Starred);
    let folders = named(places).into_iter().map(Row::Place);

    commands
        .chain(documents)
        .chain(stars)
        .chain(folders)
        .take(MAX_ROWS)
        .collect()
}

/// Which row the keys act on: the cursor, clamped to the list as it stands.
///
/// A cursor that has not moved yet rests on the row that "take me back" means
/// rather than on the document already open.
pub fn cursor_row(state: &AppState, len: usize) -> usize {
    match *state.palette_cursor.read() {
        None => initial_cursor(len),
        Some(at) => at.min(len.saturating_sub(1)),
    }
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
    // The query is the window's, because the keys that move through this list
    // are bindings, dispatched from outside the component.
    let query = state.palette_query;
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
    let (starred, places) = kept();
    let rows = {
        let visits = VISITS.read();
        rows_for(&visits.items, &starred, &places, &needle)
    };

    // How long the list is, so the keys that move through it know where it
    // ends. Written every draw; `peek` because writing what one has just read
    // is how an effect wakes itself.
    if *state.palette_rows.peek() != rows.len() {
        state.palette_rows.set(rows.len());
    }

    let current = cursor_row(&state, rows.len());

    let mut close = move || state.close_palette();

    let activate = move |index: usize| {
        crate::keybindings::dispatcher::activate_palette_row(state, index);
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
                    PaletteField {
                        query,
                        placeholder: "Go somewhere you have read, or name a command",
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

/// The field the palette is typed into.
///
/// What is typed, and nothing else: the keys that act on the list under it —
/// move, confirm, close — are bindings like any other, dispatched from the
/// keybinding engine in the `palette` context, so that they can be changed.
///
/// The field is not controlled. Writing the value back on every keystroke is
/// what tears an input method's composition apart, and guarding against that
/// by ignoring what is typed while composing loses the one keystroke that
/// matters: the browser sends the composed word as an ordinary `input` event
/// *before* it says the composition ended, so the confirmed word was the one
/// thing thrown away. Nothing writes to the field — the palette is mounted
/// when it opens and gone when it closes, so it starts empty on its own.
#[component]
pub fn PaletteField(query: Signal<String>, placeholder: &'static str) -> Element {
    let mut state = use_context::<AppState>();
    let mut query = query;

    rsx! {
        input {
            class: "palette-input",
            r#type: "text",
            placeholder,
            autocorrect: "off",
            autocapitalize: "off",
            spellcheck: "false",
            oninput: move |evt| {
                query.set(evt.value());
                // A narrowed list is a new list, so the keys start at the top
                // of it rather than wherever they had reached in the old one.
                state.palette_cursor.set(Some(0));
            },
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

/// The directory a row's path sits in, shortened the way the reader says it.
fn parent_label(path: &std::path::Path) -> String {
    crate::utils::paths::parent_label(path)
}

/// What the reader kept, as two lists: the starred documents and the places.
pub fn kept() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let bookmarks = crate::bookmarks::BOOKMARKS.read();
    let (places, starred): (Vec<_>, Vec<_>) = bookmarks
        .items
        .iter()
        .partition(|bookmark| bookmark.is_dir());
    (
        starred.into_iter().map(|b| b.path.clone()).collect(),
        places.into_iter().map(|b| b.path.clone()).collect(),
    )
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

    fn kept_paths() -> (Vec<PathBuf>, Vec<PathBuf>) {
        (
            vec![PathBuf::from("/kept/spec.md")],
            vec![PathBuf::from("/work/spec-project")],
        )
    }

    #[test]
    fn an_empty_query_lists_the_history_alone() {
        let visits = vec![visit("/docs/README.md"), visit("/docs/CHANGELOG.md")];
        let (starred, places) = kept_paths();
        let rows = rows_for(&visits, &starred, &places, "");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| matches!(row, Row::Document(_))));
    }

    #[test]
    fn a_query_puts_commands_above_documents() {
        let visits = vec![visit("/docs/printing.md")];
        let rows = rows_for(&visits, &[], &[], "print");
        assert_eq!(rows[0], Row::Command(Action::FilePrint));
        assert!(matches!(rows[1], Row::Document(_)));
    }

    #[test]
    fn a_query_reaches_the_stars_and_the_places_too() {
        let visits = vec![visit("/docs/spec-notes.md")];
        let (starred, places) = kept_paths();
        let rows = rows_for(&visits, &starred, &places, "spec");

        let groups: Vec<_> = rows.iter().map(|row| row.group()).collect();
        assert_eq!(groups, vec!["Recent", "Starred", "Places"]);
    }

    #[test]
    fn what_the_query_does_not_name_stays_out() {
        let (starred, places) = kept_paths();
        let rows = rows_for(&[], &starred, &places, "nothing like it");
        assert!(rows.is_empty());
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
    // A word above the first row of each kind. It is what makes a long list
    // read as several short ones, and it costs one row each.
    let headings: Vec<Option<&'static str>> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let first = index == 0 || rows[index - 1].group() != row.group();
            first.then(|| row.group())
        })
        .collect();

    rsx! {
        div {
            class: "palette-rows",
            for (index, row) in rows.iter().enumerate() {
                if let Some(heading) = headings[index] {
                    div { class: "palette-group", "{heading}" }
                }
                match row {
                    Row::Command(action) => rsx! {
                        div {
                            key: "command:{action}",
                            class: "palette-row",
                            class: if index == current { "keyboard-focused" },
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
                        PalettePath {
                            key: "document:{visit.path.display()}",
                            path: visit.path.clone(),
                            icon: IconName::File,
                            current: index == current,
                            on_pick: move |_| on_pick.call(index),
                        }
                    },
                    Row::Starred(path) => rsx! {
                        PalettePath {
                            key: "starred:{path.display()}",
                            path: path.clone(),
                            icon: IconName::StarFilled,
                            current: index == current,
                            on_pick: move |_| on_pick.call(index),
                        }
                    },
                    Row::Place(path) => rsx! {
                        PalettePath {
                            key: "place:{path.display()}",
                            path: path.clone(),
                            icon: IconName::Bookmark,
                            current: index == current,
                            on_pick: move |_| on_pick.call(index),
                        }
                    },
                }
            }
        }
    }
}

/// A row that offers somewhere to go: its name, and the folder it is in.
#[component]
fn PalettePath(path: PathBuf, icon: IconName, current: bool, on_pick: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "palette-row",
            class: if current { "keyboard-focused" },
            title: "{path.display()}",
            onclick: move |_| on_pick.call(()),
            Icon { name: icon, size: 14 }
            span {
                class: "palette-row-name",
                DocumentName { path: path.clone() }
            }
            span { class: "palette-row-path", "{parent_label(&path)}" }
        }
    }
}
