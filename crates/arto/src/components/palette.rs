use dioxus::document;
use dioxus::prelude::*;

use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::matched::Matched;
use crate::files::Listing;
use crate::fuzzy::{best, Query};
use crate::keybindings::{Action, COMMAND_ACTIONS};
use crate::state::AppState;
use crate::visits::{Visit, VISITS, VISITS_CHANGED};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// How many rows the palette will draw at once.
///
/// The list is a way back to something read recently, not a browser for the
/// whole history — that is the panel's Recent face, and the last row here
/// says so.
const MAX_ROWS: usize = 40;

/// How many rows each kind contributes to a narrowed list, at most.
///
/// A fuzzy query answers with far more than a literal one does, so without a
/// share each the best-scoring kind would take the whole list: a query that
/// finds forty files would leave no room for the command it also names. The
/// shares add up to more than [`MAX_ROWS`], deliberately — a kind that finds
/// nothing gives its room to the kinds below it.
///
/// They are shares of a *narrowed* list. With nothing typed there is only
/// one kind — the history — and it has the whole of [`MAX_ROWS`]: the
/// palette opens as a way back through what was read, and a dozen rows is
/// not that.
const MAX_COMMANDS: usize = 8;
const MAX_DOCUMENTS: usize = 12;
const MAX_KEPT: usize = 6;
const MAX_FILE_ROWS: usize = 16;

/// One line in the palette.
///
/// Everything the reader can reach shares a list rather than sitting in five,
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
    /// A document under the folder this window is working in, read or not.
    File(PathBuf),
}

impl Row {
    /// The heading this row sits under.
    ///
    /// Short lists read faster than one long one: the eye stops looking for a
    /// command among the documents.
    fn group(&self) -> &'static str {
        match self {
            Self::Command(_) => "Commands",
            Self::Document(_) => "Recent",
            Self::Starred(_) => "Starred",
            Self::Place(_) => "Places",
            Self::File(_) => "Files",
        }
    }

    /// The document or folder this row leads to, if it leads anywhere.
    fn path(&self) -> Option<&Path> {
        match self {
            Self::Command(_) => None,
            Self::Document(visit) => Some(&visit.path),
            Self::Starred(path) | Self::Place(path) | Self::File(path) => Some(path),
        }
    }
}

/// Where the palette's rows come from.
///
/// Every list the palette can draw, gathered by the caller so that the rule
/// turning them into rows stays a function of its arguments — the component
/// reads the app's state, the dispatcher reads the same state again, and
/// [`rows_for`] cannot tell the difference.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sources<'a> {
    /// The reading history, newest first.
    pub visits: &'a [Visit],
    /// The documents starred.
    pub starred: &'a [PathBuf],
    /// The folders kept.
    pub places: &'a [PathBuf],
    /// The files under the folder this window is in, once they are listed.
    pub files: Option<&'a Listing>,
}

/// The commands `query` names, best first and at most `cap` of them.
///
/// The rule is [`crate::fuzzy`]'s, which is every other list's: sharing it
/// means a query behaves the same whichever kind of row it ends up finding.
///
/// An empty query answers with nothing: the palette opens on the history, and
/// a list of every command the app has would bury the two keystrokes that are
/// the point of opening it.
fn ranked_commands(query: &Query, cap: usize) -> Vec<Action> {
    if query.is_empty() {
        return Vec::new();
    }
    let ranked = COMMAND_ACTIONS.iter().copied().filter_map(|action| {
        let label = action.command_label()?;
        Some((query.rank(label)?, action))
    });
    best(ranked, cap)
}

/// The whole list for `query`: commands, the history, the stars, the places,
/// and the files under the folder this window is in.
///
/// Commands lead because a typed query that names one names it exactly. The
/// history follows, because that is what the palette opens on and what it is
/// mostly for. The stars and the places come next and only once something is
/// typed: they are short lists the reader already knows by heart, so putting
/// them above an untyped history would push the answer down the screen for
/// nothing. The files come last because they are the longest list and the
/// least particular — everything under the folder, whether or not anyone has
/// ever opened it.
///
/// Within each kind the order is the score's: a fuzzy query matches loosely
/// enough that the answer has to be the first row rather than somewhere in
/// the list.
pub fn rows_for(sources: Sources<'_>, query: &str) -> Vec<Row> {
    let query = Query::new(query);

    let commands = ranked_commands(&query, MAX_COMMANDS)
        .into_iter()
        .map(Row::Command);
    // The history has a share of a narrowed list and the whole of an
    // unnarrowed one, where it is the only kind there is.
    let documents_cap = if query.is_empty() {
        MAX_ROWS
    } else {
        MAX_DOCUMENTS
    };
    let documents = crate::visits::ranked(sources.visits, &query, documents_cap)
        .into_iter()
        .cloned()
        .map(Row::Document);
    let stars = kept_paths(sources.starred, &query)
        .into_iter()
        .map(Row::Starred);
    let folders = kept_paths(sources.places, &query)
        .into_iter()
        .map(Row::Place);

    let mut rows: Vec<Row> = commands
        .chain(documents)
        .chain(stars)
        .chain(folders)
        .collect();

    // Last, and against the rows already found: a document that is in the
    // history is offered by the history, and offering it again under another
    // heading would make the list longer without making it wider.
    let shown: HashSet<PathBuf> = rows
        .iter()
        .filter_map(Row::path)
        .map(Path::to_path_buf)
        .collect();
    rows.extend(
        files_for(sources.files, &query, &shown)
            .into_iter()
            .map(Row::File),
    );

    rows.truncate(MAX_ROWS);
    rows
}

/// The starred documents or the kept places a query names.
///
/// Nothing until something is typed: these are short lists the reader keeps,
/// and the palette opens on the history.
fn kept_paths(paths: &[PathBuf], query: &Query) -> Vec<PathBuf> {
    if query.is_empty() {
        return Vec::new();
    }
    let ranked = paths
        .iter()
        .filter_map(|path| Some((query.rank_path(path)?, path)));
    best(ranked, MAX_KEPT).into_iter().cloned().collect()
}

/// The files under the window's folder that a query names, less the ones
/// another kind of row already offers.
///
/// Matched against the path relative to that folder: the folder is the same
/// for every candidate, so its name is noise in the middle of every haystack
/// — and once it scores, noise that reorders the answer.
fn files_for(listing: Option<&Listing>, query: &Query, shown: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let Some(listing) = listing else {
        return Vec::new();
    };
    if query.is_empty() {
        return Vec::new();
    }
    // Ranked as references and cloned only once the list is cut to size: this
    // is the one candidate list that can run to thousands, and it is ranked
    // again on every keystroke.
    let ranked = listing
        .files
        .iter()
        .filter(|path| !shown.contains(*path))
        .filter_map(|path| Some((query.rank_path(listing.relative(path))?, path)));
    best(ranked, MAX_FILE_ROWS).into_iter().cloned().collect()
}

/// Everything the palette is offering right now, as its keys and its rows
/// both see it.
///
/// The component draws this and the dispatcher acts on it, so it is one
/// function rather than two that have to agree: a row's index means the same
/// thing to the key that moves onto it and to the click that takes it.
pub fn current_rows(state: &AppState) -> Vec<Row> {
    let visits = VISITS.read();
    let (starred, places) = kept();
    let listing = window_files(state);
    rows_for(
        Sources {
            visits: &visits.items,
            starred: &starred,
            places: &places,
            files: listing.as_deref(),
        },
        &state.palette_query.read(),
    )
}

/// The folder this window is working in.
fn window_root(state: &AppState) -> Option<PathBuf> {
    state.sidebar.read().primary_root().cloned()
}

/// The files under that folder, as far as they have been listed.
fn window_files(state: &AppState) -> Option<Arc<Listing>> {
    let root = window_root(state)?;
    crate::files::listing(&root, state.sidebar.read().show_all_files)
}

/// Ask for the window's folder to be listed, if it has not been lately.
///
/// Called as the palette opens: a window whose palette is never opened never
/// walks a directory, and one that is opened again a moment later is answered
/// from what the first opening read.
fn ensure_files(state: &AppState) {
    if let Some(root) = window_root(state) {
        crate::files::ensure(&root, state.sidebar.read().show_all_files);
    }
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
/// keys. Typing narrows every list the window can offer — the commands, the
/// history, the stars, the places, and the files under the folder this
/// window is in — with one rule, [`crate::fuzzy`]'s, so that a query means
/// the same thing wherever it lands and the best answer is the first row.
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

    // The folder this window is in, read on the way in rather than on the way
    // up: a window whose palette is never opened never walks a directory.
    use_hook(move || ensure_files(&state));

    use_future(move || async move {
        let mut visited = VISITS_CHANGED.subscribe();
        let mut listed = crate::files::FILES_CHANGED.subscribe();
        loop {
            let changed = tokio::select! {
                result = visited.recv() => result.is_ok(),
                // A scan that finishes while the palette is open is rows the
                // reader is waiting for, so they arrive rather than waiting
                // for the next opening.
                result = listed.recv() => result.is_ok(),
            };
            if !changed {
                break;
            }
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

    // The list, worked out again only when something it is made of has
    // changed. A memo rather than a line in the draw, because a draw is not
    // the same thing as a change: writing how many rows there are draws
    // again, and so does moving the cursor between them, and ranking every
    // file under the folder a second and third time for one keystroke is
    // most of what the keystroke would cost. The reads below are what it
    // watches — the query, and the numbers that stand for the lists the
    // rows come from.
    let rows = use_memo(move || {
        let _ = revision();
        let _ = state.visits_revision.read();
        current_rows(&state)
    });
    let needle = query();
    let rows = rows();
    // Whether the folder was too large to list whole, and a file row is on
    // screen for the reader to wonder about the completeness of.
    let partial = rows.iter().any(|row| matches!(row, Row::File(_)))
        && window_files(&state).is_some_and(|listing| listing.truncated);

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
                        placeholder: "Find a file, go back to one you have read, or name a command",
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
                    query: needle.clone(),
                    on_pick: activate,
                }

                // Said once, and only where it changes what the list means: a
                // folder too large to list whole is one where "not here" is
                // not an answer the palette can give. Without a number,
                // because a walk stops for whichever of its bounds it
                // reaches first and the reader would read a count as the
                // one that stopped this one.
                if partial {
                    div {
                        class: "palette-note",
                        "This folder was too large to list whole — some files are not offered."
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

    fn sources<'a>(
        visits: &'a [Visit],
        starred: &'a [PathBuf],
        places: &'a [PathBuf],
    ) -> Sources<'a> {
        Sources {
            visits,
            starred,
            places,
            files: None,
        }
    }

    fn listing(root: &str, files: &[&str]) -> Listing {
        Listing {
            root: PathBuf::from(root),
            all_files: false,
            files: files.iter().map(PathBuf::from).collect(),
            truncated: false,
        }
    }

    #[test]
    fn an_empty_query_lists_the_history_alone() {
        let visits = vec![visit("/docs/README.md"), visit("/docs/CHANGELOG.md")];
        let (starred, places) = kept_paths();
        let listed = listing("/docs", &["/docs/UNREAD.md"]);
        let rows = rows_for(
            Sources {
                files: Some(&listed),
                ..sources(&visits, &starred, &places)
            },
            "",
        );
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| matches!(row, Row::Document(_))));
    }

    #[test]
    fn an_empty_query_lists_the_history_to_the_end_of_the_screen() {
        // The shares are shares of a narrowed list; unnarrowed, the history
        // is the only kind there is and has the whole of it.
        let visits: Vec<Visit> = (0..MAX_ROWS * 2)
            .map(|at| visit(&format!("/docs/note-{at}.md")))
            .collect();
        let rows = rows_for(sources(&visits, &[], &[]), "");

        assert_eq!(rows.len(), MAX_ROWS);
    }

    #[test]
    fn a_query_puts_commands_above_documents() {
        let visits = vec![visit("/docs/printing.md")];
        let rows = rows_for(sources(&visits, &[], &[]), "print");
        assert_eq!(rows[0], Row::Command(Action::FilePrint));
        assert!(matches!(rows[1], Row::Document(_)));
    }

    #[test]
    fn a_query_reaches_the_stars_and_the_places_too() {
        let visits = vec![visit("/docs/spec-notes.md")];
        let (starred, places) = kept_paths();
        let rows = rows_for(sources(&visits, &starred, &places), "spec");

        let groups: Vec<_> = rows.iter().map(|row| row.group()).collect();
        assert_eq!(groups, vec!["Recent", "Starred", "Places"]);
    }

    #[test]
    fn what_the_query_does_not_name_stays_out() {
        let (starred, places) = kept_paths();
        let rows = rows_for(sources(&[], &starred, &places), "nothing like it");
        assert!(rows.is_empty());
    }

    #[test]
    fn a_query_reaches_a_file_nobody_has_opened() {
        let listed = listing("/work", &["/work/docs/architecture.md"]);
        let rows = rows_for(
            Sources {
                files: Some(&listed),
                ..Sources::default()
            },
            "arch",
        );

        assert_eq!(
            rows,
            vec![Row::File(PathBuf::from("/work/docs/architecture.md"))]
        );
    }

    #[test]
    fn the_files_come_after_everything_the_reader_already_has() {
        let visits = vec![visit("/work/spec-notes.md")];
        let (starred, places) = kept_paths();
        let listed = listing("/work", &["/work/docs/spec-draft.md"]);
        let rows = rows_for(
            Sources {
                files: Some(&listed),
                ..sources(&visits, &starred, &places)
            },
            "spec",
        );

        let groups: Vec<_> = rows.iter().map(|row| row.group()).collect();
        assert_eq!(groups, vec!["Recent", "Starred", "Places", "Files"]);
    }

    #[test]
    fn a_file_already_offered_is_not_offered_twice() {
        let visits = vec![visit("/work/guide.md")];
        let listed = listing("/work", &["/work/guide.md", "/work/guidelines.md"]);
        let rows = rows_for(
            Sources {
                files: Some(&listed),
                ..sources(&visits, &[], &[])
            },
            "guide",
        );

        assert_eq!(
            rows,
            vec![
                Row::Document(visits[0].clone()),
                Row::File(PathBuf::from("/work/guidelines.md")),
            ]
        );
    }

    #[test]
    fn the_folder_every_file_shares_does_not_answer_for_them() {
        // Every candidate sits under `/work/reports`, so a query naming the
        // folder alone is naming all of them and none of them.
        let listed = listing(
            "/work/reports",
            &["/work/reports/january.md", "/work/reports/rota.md"],
        );
        let rows = rows_for(
            Sources {
                files: Some(&listed),
                ..Sources::default()
            },
            "rep",
        );

        assert!(rows.is_empty(), "{rows:?}");
    }

    #[test]
    fn the_closest_match_leads_its_kind() {
        let visits = vec![visit("/docs/g-u-i-d-e-lines.md"), visit("/docs/guide.md")];
        let rows = rows_for(sources(&visits, &[], &[]), "guide");

        assert_eq!(rows[0], Row::Document(visits[1].clone()));
    }

    #[test]
    fn one_kind_cannot_take_the_whole_list() {
        let files: Vec<String> = (0..MAX_ROWS * 2)
            .map(|at| format!("/work/note-{at}.md"))
            .collect();
        let listed = listing(
            "/work",
            &files.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let visits: Vec<Visit> = (0..MAX_ROWS)
            .map(|at| visit(&format!("/read/note-{at}.md")))
            .collect();
        let rows = rows_for(
            Sources {
                files: Some(&listed),
                ..sources(&visits, &[], &[])
            },
            "note",
        );

        let files_shown = rows
            .iter()
            .filter(|row| matches!(row, Row::File(_)))
            .count();
        let documents_shown = rows
            .iter()
            .filter(|row| matches!(row, Row::Document(_)))
            .count();
        assert_eq!(documents_shown, MAX_DOCUMENTS);
        assert_eq!(files_shown, MAX_FILE_ROWS);
        assert!(rows.len() <= MAX_ROWS);
    }

    fn commands_for(query: &str) -> Vec<Action> {
        ranked_commands(&Query::new(query), usize::MAX)
    }

    fn names(query: &str) -> Vec<&'static str> {
        commands_for(query)
            .into_iter()
            .filter_map(|action| action.command_label())
            .collect()
    }

    #[test]
    fn characters_in_order_are_enough_to_name_a_command() {
        assert!(names("close windows").contains(&"Close All Windows"));
        assert!(names("cw").contains(&"Close All Windows"));
        assert!(!names("close tabs").contains(&"Close All Windows"));
    }

    #[test]
    fn nothing_typed_names_no_command() {
        assert!(commands_for("").is_empty());
        assert!(commands_for("   ").is_empty());
    }

    #[test]
    fn the_command_named_most_closely_comes_first() {
        let found = commands_for("close all windows");
        assert_eq!(found.first(), Some(&Action::WindowCloseAllWindows));
    }

    #[test]
    fn commands_that_score_alike_keep_their_listed_order() {
        let found = commands_for("theme set");
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
///
/// `query` is passed down rather than matched again from the row: what a
/// fuzzy match answers with is not always visible in the name it found, so
/// each row marks the characters that put it there.
#[component]
pub fn PaletteRows(
    rows: Vec<Row>,
    current: usize,
    #[props(default)] query: String,
    on_pick: EventHandler<usize>,
) -> Element {
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
                                Matched {
                                    text: action.command_label().unwrap_or_default().to_string(),
                                    query: query.clone(),
                                }
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
                            query: query.clone(),
                            on_pick: move |_| on_pick.call(index),
                        }
                    },
                    Row::Starred(path) => rsx! {
                        PalettePath {
                            key: "starred:{path.display()}",
                            path: path.clone(),
                            icon: IconName::StarFilled,
                            current: index == current,
                            query: query.clone(),
                            on_pick: move |_| on_pick.call(index),
                        }
                    },
                    Row::Place(path) => rsx! {
                        PalettePath {
                            key: "place:{path.display()}",
                            path: path.clone(),
                            icon: IconName::Bookmark,
                            current: index == current,
                            query: query.clone(),
                            on_pick: move |_| on_pick.call(index),
                        }
                    },
                    Row::File(path) => rsx! {
                        PalettePath {
                            key: "file:{path.display()}",
                            path: path.clone(),
                            icon: IconName::FileSearch,
                            current: index == current,
                            query: query.clone(),
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
fn PalettePath(
    path: PathBuf,
    icon: IconName,
    current: bool,
    #[props(default)] query: String,
    on_pick: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "palette-row",
            class: if current { "keyboard-focused" },
            title: "{path.display()}",
            onclick: move |_| on_pick.call(()),
            Icon { name: icon, size: 14 }
            span {
                class: "palette-row-name",
                DocumentName { path: path.clone(), query: query.clone() }
            }
            span { class: "palette-row-path", "{parent_label(&path)}" }
        }
    }
}
