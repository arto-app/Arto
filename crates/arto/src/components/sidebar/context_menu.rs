use std::path::{Path, PathBuf};

use dioxus::prelude::*;

use crate::bookmarks::BOOKMARKS;
use crate::components::context_menu::{clamp_menu_position, ContextMenuItem, ContextMenuSeparator};
use crate::components::icon::{Icon, IconName};
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarItemKind {
    File,
    Directory,
}

impl SidebarItemKind {
    /// Whether this entry is a directory.
    pub fn is_dir(self) -> bool {
        matches!(self, Self::Directory)
    }
}

/// Estimated width of the context menu (CSS `min-width: 200px` + padding/border).
const MENU_WIDTH: i32 = 220;
/// Estimated height of the tallest menu variant (a directory with every section).
const MENU_HEIGHT: i32 = 360;
/// Gap kept between the menu and the viewport edge.
const VIEWPORT_MARGIN: i32 = 8;

/// Complete state for the hoisted sidebar file-tree context menu.
///
/// This lives in [`crate::state::AppState`] as a single `Option` and is rendered
/// exactly once at the `app-container` root by
/// [`crate::components::sidebar::file_explorer::SidebarContextMenuHost`].
/// Keeping it out of the file-tree subtree means watcher-driven remounts (keyed
/// on `sidebar_refresh_counter`) can no longer unmount an open menu.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarContextMenuData {
    /// Viewport-clamped top-left origin, in unscaled CSS pixels.
    pub position: (i32, i32),
    pub path: PathBuf,
    pub kind: SidebarItemKind,
}

/// Raise the panel's menu on a row, wherever the row is drawn.
///
/// The menu is rendered once at the window's root rather than inside the list
/// that asked for it, so a list rebuilding itself under an open menu cannot
/// take it away. This is what a row does to ask for it.
pub fn open_row_context_menu(
    mut state: AppState,
    path: &std::path::Path,
    kind: SidebarItemKind,
    evt: &Event<MouseData>,
) {
    evt.prevent_default();
    evt.stop_propagation();

    let cursor = {
        let coords = evt.data().client_coordinates();
        (coords.x as i32, coords.y as i32)
    };
    let viewport = {
        let size = *state.size.read();
        (size.width as i32, size.height as i32)
    };
    state
        .sidebar_context_menu
        .set(Some(SidebarContextMenuData::new(
            cursor,
            viewport,
            path.to_path_buf(),
            kind,
        )));
}

impl SidebarContextMenuData {
    /// Build menu state from a raw cursor position, clamped to the viewport.
    pub fn new(
        cursor: (i32, i32),
        viewport: (i32, i32),
        path: PathBuf,
        kind: SidebarItemKind,
    ) -> Self {
        let position =
            clamp_menu_position(cursor, (MENU_WIDTH, MENU_HEIGHT), viewport, VIEWPORT_MARGIN);
        Self {
            position,
            path,
            kind,
        }
    }
}

/// Whether a snapshotted context-menu action may still act on `path`.
///
/// The hoisted menu lives outside the watcher-keyed file tree, so it stays open
/// when a filesystem event deletes or renames its target. Path-dependent actions
/// (open, change root, reveal) must re-check existence at click time before
/// acting on a possibly-stale snapshot.
pub(super) fn context_action_should_proceed(path: &Path) -> bool {
    path.exists()
}

#[component]
pub fn SidebarContextMenu(
    position: (i32, i32),
    path: PathBuf,
    kind: SidebarItemKind,
    on_close: EventHandler<()>,
    on_open: EventHandler<()>,
    on_open_in_new_window: EventHandler<()>,
    on_change_root_directory: EventHandler<()>,
    on_toggle_bookmark: EventHandler<()>,
    on_copy_path: EventHandler<()>,
    on_reveal_in_finder: EventHandler<()>,
    on_reload: EventHandler<()>,
) -> Element {
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Sidebar, action);

    let is_file = kind == SidebarItemKind::File;
    let is_bookmarked = BOOKMARKS.read().contains(&path);

    // Dynamic labels based on item kind
    let open_label = if is_file {
        "Open File"
    } else {
        "Open Directory"
    };
    let copy_path_label = if is_file {
        "Copy File Path"
    } else {
        "Copy Directory Path"
    };

    rsx! {
        // Backdrop to close menu on outside click
        div {
            class: "context-menu-backdrop",
            onclick: move |_| on_close.call(()),
        }

        // Context menu
        div {
            class: "context-menu",
            style: "left: {position.0}px; top: {position.1}px;",
            onclick: move |evt| evt.stop_propagation(),

            // === Section 1: Open operations ===
            ContextMenuItem {
                label: open_label,
                icon: Some(if is_file { IconName::File } else { IconName::FolderOpen }),
                on_click: move |_| on_open.call(()),
            }

            if !is_file {
                ContextMenuItem {
                    label: "Change Root Directory",
                    shortcut: shortcut("cursor.enter"),
                    icon: Some(IconName::FolderOpen),
                    on_click: move |_| on_change_root_directory.call(()),
                }
            }

            ContextMenuItem {
                label: "Open in New Window",
                on_click: move |_| on_open_in_new_window.call(()),
            }

            // === Section 2: Starred ===
            ContextMenuSeparator {}

            // A folder that is kept is one of the places; a document that is
            // kept is one of the stars. One list, two names, and the row says
            // which of them it is about.
            div {
                class: "context-menu-item",
                onclick: move |_| on_toggle_bookmark.call(()),

                Icon {
                    name: if is_file {
                        if is_bookmarked { IconName::StarFilled } else { IconName::Star }
                    } else {
                        IconName::Bookmark
                    },
                    size: 14,
                    class: "context-menu-icon",
                }

                span {
                    class: "context-menu-label",
                    match (is_file, is_bookmarked) {
                        (true, true) => "Remove from Stars",
                        (true, false) => "Add to Stars",
                        (false, true) => "Remove from Places",
                        (false, false) => "Add to Places",
                    }
                }
            }

            ContextMenuSeparator {}

            ContextMenuItem {
                label: copy_path_label,
                shortcut: shortcut("clipboard.copy_file_path"),
                icon: Some(IconName::Copy),
                on_click: move |_| on_copy_path.call(()),
            }

            ContextMenuItem {
                label: "Reveal in Finder",
                shortcut: shortcut("file.reveal_in_finder"),
                icon: Some(IconName::Folder),
                on_click: move |_| on_reveal_in_finder.call(()),
            }

            // === Section 4: Reload ===
            ContextMenuSeparator {}


            ContextMenuItem {
                label: "Reload",
                shortcut: shortcut("window.reload"),
                icon: Some(IconName::Refresh),
                on_click: move |_| on_reload.call(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_action_proceeds_only_for_existing_paths() {
        // Existing target: the action may proceed.
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("note.md");
        std::fs::write(&existing, "content").unwrap();
        assert!(context_action_should_proceed(&existing));

        // Vanished target (deleted/renamed while the menu was open): no-op.
        let missing = dir.path().join("gone.md");
        assert!(!context_action_should_proceed(&missing));
    }

    #[test]
    fn new_clamps_the_position_at_a_corner() {
        // Clamping through the public constructor.
        let data = SidebarContextMenuData::new(
            (10_000, 10_000),
            (1000, 800),
            PathBuf::from("/tmp/example.md"),
            SidebarItemKind::File,
        );
        assert_eq!(
            data.position,
            (
                1000 - VIEWPORT_MARGIN - MENU_WIDTH,
                800 - VIEWPORT_MARGIN - MENU_HEIGHT
            )
        );
    }
}
