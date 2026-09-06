//! Reusable bookmark toggle button component for Quick Access feature.

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::bookmarks::{toggle_bookmark, BOOKMARKS, BOOKMARKS_CHANGED};
use crate::components::icon::{Icon, IconName};

/// Reusable bookmark toggle button
#[component]
pub fn BookmarkButton(
    /// Path to bookmark/unbookmark
    path: PathBuf,
    /// Icon size in pixels (default: 14)
    #[props(default = 14)]
    size: u32,
) -> Element {
    // Re-render trigger for bookmark changes from other windows/components.
    // When BOOKMARKS_CHANGED fires, toggling this signal causes a re-render,
    // which re-evaluates `is_bookmarked` below with the current `path` prop.
    let mut bookmark_dirty = use_signal(|| false);
    use_future(move || async move {
        let mut rx = BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            bookmark_dirty.set(!bookmark_dirty());
        }
    });

    // Compute bookmark status from current path prop and current bookmarks.
    // This is re-evaluated when:
    // - `path` prop changes (component re-renders due to prop change)
    // - `bookmark_dirty` signal changes (bookmarks modified elsewhere)
    // Reading the signal here subscribes this component to its changes.
    let _ = *bookmark_dirty.read();
    let is_bookmarked = BOOKMARKS.read().contains(&path);

    let handle_click = {
        let path = path.clone();
        move |evt: Event<MouseData>| {
            evt.stop_propagation();
            toggle_bookmark(&path);
        }
    };

    let icon_name = if is_bookmarked {
        IconName::StarFilled
    } else {
        IconName::Star
    };

    let title = if is_bookmarked {
        "Remove from Quick Access"
    } else {
        "Add to Quick Access"
    };

    let bookmarked_class = if is_bookmarked { "bookmarked" } else { "" };

    rsx! {
        button {
            class: "bookmark-button {bookmarked_class}",
            title: "{title}",
            draggable: false,
            onclick: handle_click,
            Icon { name: icon_name, size }
        }
    }
}
