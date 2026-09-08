//! The Starred face of the sidebar panel.
//!
//! The bookmarked *files*, in the order they were arranged. A bookmarked
//! folder is a place — the Files face heads a tree with it — so listing it
//! here as well would be the same folder twice, once in each face.
//!
//! A file name alone does not say which of three `README.md`s it is, so each
//! row carries the folder it sits in.

use dioxus::prelude::*;

use crate::bookmarks::{move_bookmark, Bookmark, BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::document_name::DocumentName;
use crate::components::icon::{Icon, IconName};
use crate::components::sidebar::reorder::{drop_class, drop_side, DragRow};
use crate::components::sidebar::row_actions::RowActions;
use crate::state::{AppState, FocusedPanel};

/// Bookmark with cached filesystem status to avoid filesystem calls during render
#[derive(Clone)]
struct CachedBookmark {
    /// Where the bookmark sits in the whole list, folders included: the order
    /// a drag rearranges is that list's, not this face's view of it.
    index: usize,
    bookmark: Bookmark,
    exists: bool,
}

/// The bookmarked files, with cached exists status.
fn load_cached_bookmarks() -> Vec<CachedBookmark> {
    BOOKMARKS
        .read()
        .items
        .iter()
        .enumerate()
        .filter(|(_, bookmark)| !bookmark.is_dir())
        .map(|(index, bookmark)| CachedBookmark {
            index,
            bookmark: bookmark.clone(),
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

    // Local signal to track bookmark items with cached exists status
    let mut bookmarks = use_signal(load_cached_bookmarks);

    let mut filter = use_signal(String::new);

    // What is being dragged, and the row it is resting on. Both carry the
    // position they were drawn at as well as the path: the position says which
    // way the row is travelling, and that is what decides which side of the
    // row it lands on.
    let mut dragging = use_signal(|| None::<DragRow>);
    let mut drop_target = use_signal(|| None::<DragRow>);

    // Subscribe to bookmark changes and refresh exists status
    use_future(move || async move {
        let mut rx = BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            bookmarks.set(load_cached_bookmarks());
        }
    });

    let current = state.current_file();
    let is_qa_focused = *state.focused_panel.read() == FocusedPanel::QuickAccess;
    let quick_access_cursor = *state.quick_access_cursor.read();
    let needle = filter();
    let all = bookmarks.read();
    // Narrowed by the same rule as every other list on this history, so a
    // query means one thing wherever it is typed.
    let items: Vec<CachedBookmark> = all
        .iter()
        .filter(|cached| crate::visits::matches_path(&cached.bookmark.path, &needle))
        .cloned()
        .collect();
    let nothing_starred = all.is_empty();
    drop(all);

    rsx! {
        div {
            class: "left-sidebar-face left-sidebar-quick-access",

            div {
                class: "left-sidebar-face-list",

                // The same word-on-a-hairline the other faces head their rows
                // with. It names the list and, as much to the point, gives the
                // first row the same air every other face gives its first row.
                div {
                    class: "left-sidebar-root-group-label",
                    span { "Starred" }
                }

            if nothing_starred {
                div { class: "left-sidebar-explorer-empty", "Nothing starred yet" }
            } else if items.is_empty() {
                div { class: "left-sidebar-explorer-empty", "Nothing here answers that" }
            }

            // Bookmark items
            div {
                class: "left-sidebar-quick-access-list",
                ondragover: move |evt| {
                    evt.stop_propagation();
                    evt.prevent_default();
                },
                for cached in items.iter() {
                    QuickAccessItem {
                        key: "{cached.bookmark.path.display()}",
                        index: cached.index,
                        bookmark: cached.bookmark.clone(),
                        exists: cached.exists,
                        is_dragging: dragging.read().as_ref().map(|(index, _)| *index) == Some(cached.index),
                        drop_side: drop_side(&dragging.read(), &drop_target.read(), cached.index),
                        is_keyboard_focused: is_qa_focused && quick_access_cursor == Some(cached.index),
                        is_current: current.as_deref() == Some(cached.bookmark.path.as_path()),
                        on_click: move |bookmark: Bookmark| {
                            state.open_file(&bookmark.path);
                        },
                        on_drag_start: move |row| {
                            dragging.set(Some(row));
                        },
                        on_drag_over: move |row| {
                            if dragging.read().is_some() {
                                drop_target.set(Some(row));
                            }
                        },
                        on_drag_leave: move |_| {
                            drop_target.set(None);
                        },
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

            // At the foot: the list is what the face is for, and a field above
            // it would be the first thing between the reader and it.
            div {
                class: "left-sidebar-filter",
                Icon { name: IconName::Search, size: 12 }
                input {
                    class: "left-sidebar-filter-input",
                    r#type: "text",
                    placeholder: "Filter",
                    value: "{filter}",
                    oninput: move |evt| filter.set(evt.value()),
                }
            }
        }
    }
}

/// A single bookmark item in the Quick Access list
#[component]
fn QuickAccessItem(
    index: usize,
    bookmark: Bookmark,
    /// Cached exists status (computed when bookmarks change, not on every render)
    exists: bool,
    is_dragging: bool,
    /// Which side of this row the dragged one would land on, when it is the
    /// row being rested on: `Some(true)` after it, `Some(false)` before it.
    drop_side: Option<bool>,
    is_keyboard_focused: bool,
    /// Whether this is the document the window is reading.
    is_current: bool,
    on_click: EventHandler<Bookmark>,
    on_drag_start: EventHandler<DragRow>,
    on_drag_over: EventHandler<DragRow>,
    on_drag_leave: EventHandler<()>,
    on_drag_end: EventHandler<()>,
) -> Element {
    let path = bookmark.path.clone();
    let last_read = crate::visits::last_read(&path)
        .map(|at| crate::visits::short_when(at, chrono::Local::now()))
        .unwrap_or_default();

    let mut classes = vec!["left-sidebar-quick-access-item"];
    if !exists {
        classes.push("missing");
    }
    if is_dragging {
        classes.push("dragging");
    }
    classes.push(drop_class(drop_side));
    if is_keyboard_focused {
        classes.push("keyboard-focused");
    }
    if is_current {
        classes.push("active");
    }
    let class_str = classes.join(" ");

    rsx! {
        div {
            class: "{class_str}",
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
            ondragleave: move |evt| {
                evt.stop_propagation();
                on_drag_leave.call(());
            },
            ondragend: move |evt| {
                evt.stop_propagation();
                on_drag_end.call(());
            },
            onclick: {
                let bookmark = bookmark.clone();
                move |_| {
                    if exists {
                        on_click.call(bookmark.clone());
                    }
                }
            },

            Icon {
                name: IconName::File,
                size: 16,
                class: "left-sidebar-quick-access-item-icon",
            }

            span {
                class: "left-sidebar-quick-access-item-name",
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
