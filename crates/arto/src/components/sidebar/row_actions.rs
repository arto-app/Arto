//! What a listed document can be done to, without opening it.

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::bookmark_button::BookmarkButton;
use crate::components::icon::{Icon, IconName};
use crate::state::AppState;

/// What a listed document can be done to, without opening it.
///
/// Drawn only under the pointer — a row at rest is a name — and standing where
/// the clock was, so the row is the same width either way.
///
/// The star comes last, at the row's own edge. It is the one of the three that
/// is also drawn at rest, so it is the one that has to stay where it is when
/// the others arrive; put first, it would jump left the moment the pointer
/// touched the row.
#[component]
pub fn RowActions(
    path: PathBuf,
    /// Whether this row can be forgotten.
    ///
    /// Forgetting drops a document from the reading history, so it is offered
    /// on rows that *are* history.
    #[props(default = false)]
    forgettable: bool,
    /// Whether every row of this list is bookmarked — the Starred face, and
    /// the places, which are the same list seen twice.
    ///
    /// There a star toggle would be a lit glyph on every row that has to be
    /// pressed to make something go away — which is a bin, drawn as a star. So
    /// it is drawn as a bin.
    #[props(default = false)]
    starred: bool,
    /// Whether this row is a folder, which can be made the one this window is
    /// in. A document cannot: it is read, not stood in.
    #[props(default = false)]
    rootable: bool,
    /// Whether this row is a root, which is the only kind of row with a folder
    /// *above* it that is not already drawn: everything under a root is in the
    /// tree, and the way out of the top of it is this.
    #[props(default = false)]
    root: bool,
    /// Whether this root is one of the places, rather than the folder this
    /// window is in. Going up moves whichever of the two the row belongs to.
    #[props(default = false)]
    place: bool,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut copied = use_signal(|| false);

    rsx! {
        div {
            class: "left-sidebar-row-actions",

            // Going up moves the row, not some other row: a place becomes its
            // parent and keeps its position on the list, and the window's own
            // folder becomes its parent. A single arrow that changed the group
            // below the one it was drawn in left the row it was pressed on
            // sitting exactly where it was.
            if root {
                if let Some(parent) = path.parent().map(std::path::Path::to_path_buf) {
                    button {
                        class: "left-sidebar-row-action",
                        title: if place { "Move this place up a folder" } else { "Go up to the folder above" },
                        onclick: {
                            let path = path.clone();
                            move |evt: Event<MouseData>| {
                                evt.stop_propagation();
                                if place {
                                    crate::bookmarks::replace_bookmark(&path, &parent);
                                    // The folder it was is one of the new
                                    // root's children, so opening it leaves
                                    // what was on screen on screen.
                                    state.sidebar.write().expand_towards(&parent, &path);
                                } else {
                                    state.add_root(&parent);
                                }
                            }
                        },
                        Icon { name: IconName::FolderUp, size: 12 }
                    }
                }
            }

            if rootable {
                button {
                    class: "left-sidebar-row-action",
                    title: "Make this the window's folder",
                    onclick: {
                        let path = path.clone();
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            state.add_root(&path);
                        }
                    },
                    Icon { name: IconName::FolderOpen, size: 12 }
                }
            }

            button {
                class: "left-sidebar-row-action",
                class: if copied() { "copied" },
                title: "Copy full path",
                onclick: {
                    let path = path.clone();
                    move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        crate::utils::clipboard::copy_text(path.to_string_lossy());
                        copied.set(true);
                        spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            copied.set(false);
                        });
                    }
                },
                Icon {
                    name: if copied() { IconName::Check } else { IconName::Copy },
                    size: 12,
                }
            }

            // Nothing is lost by forgetting a row — reading the document
            // again brings it back — so it is no louder than the rest.
            if forgettable {
                button {
                    class: "left-sidebar-row-action",
                    title: "Forget",
                    onclick: {
                        let path = path.clone();
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            crate::visits::forget_visit(&path);
                        }
                    },
                    Icon { name: IconName::Trash, size: 12 }
                }
            }

            if starred {
                button {
                    class: "left-sidebar-row-action left-sidebar-row-action-shown",
                    title: if place { "Remove from Places" } else { "Remove from Starred" },
                    onclick: {
                        let path = path.clone();
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            crate::bookmarks::toggle_bookmark(&path);
                        }
                    },
                    Icon { name: IconName::Trash, size: 12 }
                }
            } else {
                BookmarkButton { path: path.clone(), size: 12 }
            }
        }
    }
}
