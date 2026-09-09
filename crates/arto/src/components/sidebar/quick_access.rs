//! The Starred face of the sidebar panel.
//!
//! The bookmarked *files*, in the order they were arranged. A bookmarked
//! folder is a place — the Files face heads a tree with it — so listing it
//! here as well would be the same folder twice, once in each face.
//!
//! A file name alone does not say which of three `README.md`s it is, so each
//! row carries the folder it sits in.

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::bookmarks::{move_bookmark, BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::sidebar::context_menu::{open_row_context_menu, SidebarItemKind};
use crate::components::sidebar::reorder::{drop_class, drop_side, DragRow};
use crate::components::sidebar::row_actions::RowActions;
use crate::state::{AppState, FocusedPanel};

/// One starred document, with what the row needs that the filesystem would
/// otherwise be asked for on every render.
#[derive(Clone)]
struct StarredDocument {
    /// Where it sits in the whole bookmark list, folders included: the order a
    /// drag rearranges is that list's, not this face's view of it.
    index: usize,
    path: PathBuf,
    exists: bool,
}

/// The starred documents, read once per change rather than once per render.
fn load_starred() -> Vec<StarredDocument> {
    BOOKMARKS
        .read()
        .items
        .iter()
        .enumerate()
        .filter(|(_, bookmark)| !bookmark.is_dir())
        .map(|(index, bookmark)| StarredDocument {
            index,
            path: bookmark.path.clone(),
            exists: bookmark.exists(),
        })
        .collect()
}

/// The Starred face: the documents that are bookmarked.
///
/// The folders are bookmarked too, and they are the tree's places — one list,
/// seen as two, with nothing in both.
#[component]
pub fn StarredFace() -> Element {
    let mut state = use_context::<AppState>();

    let mut starred = use_signal(load_starred);
    // What is being dragged, and the row it is resting on. Both carry the
    // position they were drawn at as well as the path: the position says which
    // way the row is travelling, and that is what decides which side of the
    // row it lands on.
    let mut dragging = use_signal(|| None::<DragRow>);
    let mut drop_target = use_signal(|| None::<DragRow>);

    use_future(move || async move {
        let mut rx = BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            starred.set(load_starred());
        }
    });

    let current = state.current_file();
    let focused = *state.focused_panel.read() == FocusedPanel::Panel;
    let cursor = state.panel_cursor.read().clone();
    let rows = starred.read().clone();

    rsx! {
        div {
            class: "left-sidebar-face",

            div {
                class: "left-sidebar-face-list",

                // The same word-on-a-hairline the other faces head their rows
                // with. It names the list and, as much to the point, gives the
                // first row the same air every other face gives its first row.
                div {
                    class: "left-sidebar-root-group-label",
                    span { "Starred" }
                }

                if rows.is_empty() {
                    div { class: "left-sidebar-explorer-empty", "Nothing starred yet" }
                }

                div {
                    class: "left-sidebar-starred-list",
                    ondragover: move |evt| {
                        evt.stop_propagation();
                        evt.prevent_default();
                    },
                    for row in rows.iter() {
                        StarredRow {
                            key: "{row.path.display()}",
                            index: row.index,
                            path: row.path.clone(),
                            exists: row.exists,
                            is_dragging: dragging.read().as_ref().map(|(index, _)| *index) == Some(row.index),
                            drop_side: drop_side(&dragging.read(), &drop_target.read(), row.index),
                            is_keyboard_focused: focused
                                && cursor.as_ref().is_some_and(|(_, at)| *at == row.path),
                            is_current: current.as_deref() == Some(row.path.as_path()),
                            on_click: move |path: PathBuf| state.open_from_panel(&path),
                            on_drag_start: move |row| dragging.set(Some(row)),
                            on_drag_over: move |row| {
                                if dragging.read().is_some() {
                                    drop_target.set(Some(row));
                                }
                            },
                            on_drag_leave: move |_| drop_target.set(None),
                            on_drag_end: move |_| {
                                if let (Some((from, moved)), Some((to, target))) =
                                    (dragging.take(), drop_target.take())
                                {
                                    move_bookmark(&moved, &target, from < to);
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

/// One starred document, drawn by the same rules as a row of the tree.
#[component]
fn StarredRow(
    index: usize,
    path: PathBuf,
    /// Whether the file is still there, read when the bookmarks changed rather
    /// than on every render.
    exists: bool,
    is_dragging: bool,
    /// Which side of this row the dragged one would land on, when it is the
    /// row being rested on: `Some(true)` after it, `Some(false)` before it.
    drop_side: Option<bool>,
    is_keyboard_focused: bool,
    /// Whether this is the document the window is reading.
    is_current: bool,
    on_click: EventHandler<PathBuf>,
    on_drag_start: EventHandler<DragRow>,
    on_drag_over: EventHandler<DragRow>,
    on_drag_leave: EventHandler<()>,
    on_drag_end: EventHandler<()>,
) -> Element {
    let state = use_context::<AppState>();
    let last_read = crate::visits::last_read(&path)
        .map(|at| crate::visits::short_when(at, chrono::Local::now()))
        .unwrap_or_default();

    rsx! {
        div {
            class: "left-sidebar-tree-node-content left-sidebar-starred-row",
            class: if !exists { "missing" },
            class: if is_dragging { "dragging" },
            class: "{drop_class(drop_side)}",
            class: if is_keyboard_focused { "keyboard-focused" },
            class: if is_current { "active" },
            draggable: "true",
            ondragstart: {
                let path = path.clone();
                move |evt: Event<DragData>| {
                    evt.stop_propagation();
                    on_drag_start.call((index, path.clone()));
                }
            },
            ondragover: {
                let path = path.clone();
                move |evt: Event<DragData>| {
                    evt.stop_propagation();
                    evt.prevent_default();
                    on_drag_over.call((index, path.clone()));
                }
            },
            ondragleave: move |evt: Event<DragData>| {
                evt.stop_propagation();
                on_drag_leave.call(());
            },
            ondragend: move |evt: Event<DragData>| {
                evt.stop_propagation();
                on_drag_end.call(());
            },
            onclick: {
                let path = path.clone();
                move |_| {
                    if exists {
                        on_click.call(path.clone());
                    }
                }
            },
            oncontextmenu: {
                let path = path.clone();
                move |evt: Event<MouseData>| {
                    open_row_context_menu(state, &path, SidebarItemKind::File, &evt);
                }
            },

            Icon { name: IconName::File, size: 16, class: "left-sidebar-tree-icon" }

            span {
                class: "left-sidebar-tree-label",
                DocumentName { path: path.clone() }
            }

            // When it was last read. The first thing to go when the panel is
            // narrowed: it qualifies the row, it is not the row.
            if !last_read.is_empty() {
                span { class: "left-sidebar-row-when", "{last_read}" }
            }

            RowActions { path: path.clone(), starred: true }
        }
    }
}
