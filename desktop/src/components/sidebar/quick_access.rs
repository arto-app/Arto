//! Quick Access section component for the sidebar.
//!
//! Displays bookmarked files and directories for quick navigation.
//! Supports drag-and-drop reordering of bookmarks.

use dioxus::prelude::*;

use crate::bookmarks::{reorder_bookmark, Bookmark, BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::bookmark_button::BookmarkButton;
use crate::components::icon::{Icon, IconName};
use crate::state::AppState;

/// Bookmark with cached filesystem status to avoid filesystem calls during render
#[derive(Clone)]
struct CachedBookmark {
    bookmark: Bookmark,
}

impl CachedBookmark {
    fn from_bookmark(bookmark: Bookmark) -> Self {
        Self { bookmark }
    }
}

/// Load bookmarks without filesystem probing.
fn load_cached_bookmarks() -> Vec<CachedBookmark> {
    BOOKMARKS
        .read()
        .items
        .iter()
        .map(|b| CachedBookmark::from_bookmark(b.clone()))
        .collect()
}

/// Quick Access section in the sidebar
#[component]
pub fn QuickAccess() -> Element {
    let mut state = use_context::<AppState>();

    // Local signal to track bookmark items with cached exists status
    let mut bookmarks = use_signal(load_cached_bookmarks);

    // Drag state
    let mut dragging_index = use_signal(|| None::<usize>);
    let mut drop_target_index = use_signal(|| None::<usize>);

    // Subscribe to bookmark changes and refresh exists status
    use_future(move || async move {
        let mut rx = BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            bookmarks.set(load_cached_bookmarks());
        }
    });

    let items = bookmarks.read();

    // Don't render if no bookmarks
    if items.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "left-sidebar-quick-access",

            // Header
            div {
                class: "left-sidebar-quick-access-header",
                Icon {
                    name: IconName::StarFilled,
                    size: 14,
                    class: "left-sidebar-quick-access-header-icon",
                }
                span { class: "left-sidebar-quick-access-title", "QUICK ACCESS" }
            }

            // Bookmark items
            div {
                class: "left-sidebar-quick-access-list",
                ondragover: move |evt| {
                    evt.stop_propagation();
                    evt.prevent_default();
                },
                for (index, cached) in items.iter().enumerate() {
                    QuickAccessItem {
                        key: "{cached.bookmark.path.display()}",
                        index,
                        bookmark: cached.bookmark.clone(),
                        is_dragging: *dragging_index.read() == Some(index),
                        is_drop_target: *drop_target_index.read() == Some(index),
                        on_click: move |bookmark: Bookmark| {
                            if !bookmark.exists() {
                                return;
                            }
                            if bookmark.is_dir() {
                                state.set_root_directory(&bookmark.path);
                            } else {
                                state.open_file(&bookmark.path);
                            }
                        },
                        on_drag_start: move |idx| {
                            dragging_index.set(Some(idx));
                        },
                        on_drag_over: move |idx| {
                            if dragging_index.read().is_some() {
                                drop_target_index.set(Some(idx));
                            }
                        },
                        on_drag_leave: move |_| {
                            drop_target_index.set(None);
                        },
                        on_drag_end: move |_| {
                            // Perform the reorder if we have valid indices
                            if let (Some(from), Some(to)) = (*dragging_index.read(), *drop_target_index.read()) {
                                if from != to {
                                    reorder_bookmark(from, to);
                                }
                            }
                            dragging_index.set(None);
                            drop_target_index.set(None);
                        },
                    }
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
    is_dragging: bool,
    is_drop_target: bool,
    on_click: EventHandler<Bookmark>,
    on_drag_start: EventHandler<usize>,
    on_drag_over: EventHandler<usize>,
    on_drag_leave: EventHandler<()>,
    on_drag_end: EventHandler<()>,
) -> Element {
    let path = bookmark.path.clone();
    let display_name = bookmark.display_name().to_string();

    // Avoid filesystem probing during render.
    // Use a conservative heuristic so files/directories are visually distinguishable.
    let icon_name = if bookmark.path.extension().is_some() {
        IconName::File
    } else {
        IconName::Folder
    };

    let mut classes = vec!["left-sidebar-quick-access-item"];
    if is_dragging {
        classes.push("dragging");
    }
    if is_drop_target && !is_dragging {
        classes.push("drop-target");
    }
    let class_str = classes.join(" ");

    let title = path.to_string_lossy().to_string();

    rsx! {
        div {
            class: "{class_str}",
            title: "{title}",
            draggable: "true",
            ondragstart: move |evt| {
                evt.stop_propagation();
                on_drag_start.call(index);
            },
            ondragover: move |evt| {
                evt.stop_propagation();
                evt.prevent_default();
                on_drag_over.call(index);
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
                    on_click.call(bookmark.clone());
                }
            },

            // Drag handle indicator (visual only, drag is on parent)
            span {
                class: "left-sidebar-quick-access-item-drag-handle",
                draggable: false,
                Icon {
                    name: IconName::ArrowsMove,
                    size: 12,
                }
            }

            // Icon
            Icon {
                name: icon_name,
                size: 14,
                class: "left-sidebar-quick-access-item-icon",
            }

            // Name
            span {
                class: "left-sidebar-quick-access-item-name",
                "{display_name}"
            }

            // Remove button
            BookmarkButton { path: path.clone(), size: 12 }
        }
    }
}
