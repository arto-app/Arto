use std::path::{Path, PathBuf};

use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

use crate::bookmarks::BOOKMARKS;
use crate::components::context_menu::{
    clamp_menu_position, clamp_submenu_top, submenu_opens_left, ContextMenuItem,
    ContextMenuSeparator,
};
use crate::components::icon::{Icon, IconName};
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};

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
/// Width of the "Open in Window" flyout (CSS `min-width: 200px` + padding/border).
const SUBMENU_WIDTH: i32 = 208;
/// Gap kept between the menu and the viewport edge.
const VIEWPORT_MARGIN: i32 = 8;
/// Estimated height of a single context-menu row (padding + line height).
const MENU_ROW_HEIGHT: i32 = 33;
/// Estimated vertical padding around a submenu's item list (top + bottom).
const SUBMENU_VPADDING: i32 = 8;

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
    /// Whether the "Open in Window" flyout opens to the left of the menu,
    /// used near the viewport's right edge so the flyout stays on-screen.
    pub submenu_left: bool,
    /// Vertical offset (in unscaled CSS pixels) applied to the "Open in Window"
    /// flyout so its full height stays within the viewport. Zero when the flyout
    /// fits at its natural anchor; negative when it must shift up near the
    /// viewport's bottom edge.
    pub submenu_offset_y: i32,
    /// Titles of the other visible windows, for the "Open in Window" submenu.
    pub other_windows: Vec<(WindowId, String)>,
}

impl SidebarContextMenuData {
    /// Build menu state from a raw cursor position, clamping it to the viewport
    /// and choosing a submenu direction that keeps the flyout on-screen.
    pub fn new(
        cursor: (i32, i32),
        viewport: (i32, i32),
        path: PathBuf,
        kind: SidebarItemKind,
        other_windows: Vec<(WindowId, String)>,
    ) -> Self {
        let position =
            clamp_menu_position(cursor, (MENU_WIDTH, MENU_HEIGHT), viewport, VIEWPORT_MARGIN);
        let submenu_left = submenu_opens_left(
            position.0,
            MENU_WIDTH,
            SUBMENU_WIDTH,
            viewport.0,
            VIEWPORT_MARGIN,
        );
        // Anchor the flyout at the "Open in Window" row and clamp its bottom into
        // the viewport. The row count above the flyout differs by item kind (a
        // directory adds a "Change Root Directory" row).
        let rows_above_flyout = if kind.is_dir() { 3 } else { 2 };
        let anchor_y = position.1 + rows_above_flyout * MENU_ROW_HEIGHT;
        let visible_rows = other_windows.len().max(1) as i32;
        let submenu_height = visible_rows * MENU_ROW_HEIGHT + SUBMENU_VPADDING;
        let submenu_top = clamp_submenu_top(anchor_y, submenu_height, viewport.1, VIEWPORT_MARGIN);
        let submenu_offset_y = submenu_top - anchor_y;
        Self {
            position,
            path,
            kind,
            submenu_left,
            submenu_offset_y,
            other_windows,
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
    submenu_left: bool,
    submenu_offset_y: i32,
    on_close: EventHandler<()>,
    on_open: EventHandler<()>,
    on_open_in_new_window: EventHandler<()>,
    on_move_to_window: EventHandler<WindowId>,
    on_change_root_directory: EventHandler<()>,
    on_toggle_bookmark: EventHandler<()>,
    on_copy_path: EventHandler<()>,
    on_reveal_in_finder: EventHandler<()>,
    on_reload: EventHandler<()>,
    other_windows: Vec<(WindowId, String)>,
) -> Element {
    let mut show_submenu = use_signal(|| false);
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

            // Open in Window (with submenu)
            div {
                class: "context-menu-item has-submenu",
                onmouseenter: move |_| show_submenu.set(true),
                onmouseleave: move |_| show_submenu.set(false),

                span { class: "context-menu-label", "Open in Window" }
                span { class: "submenu-arrow", "›" }

                if *show_submenu.read() {
                    div {
                        class: if submenu_left { "context-submenu flip-left" } else { "context-submenu" },
                        style: "top: {submenu_offset_y}px;",

                        if other_windows.is_empty() {
                            div {
                                class: "context-menu-item disabled",
                                "No other windows"
                            }
                        } else {
                            for (window_id, title) in other_windows.iter() {
                                {
                                    let window_id = *window_id;
                                    let title = title.clone();
                                    rsx! {
                                        div {
                                            key: "{window_id:?}",
                                            class: "context-menu-item",
                                            onclick: move |_| on_move_to_window.call(window_id),
                                            "{title}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // === Section 2: Quick Access ===
            ContextMenuSeparator {}

            div {
                class: "context-menu-item",
                onclick: move |_| on_toggle_bookmark.call(()),

                Icon {
                    name: if is_bookmarked { IconName::StarFilled } else { IconName::Star },
                    size: 14,
                    class: "context-menu-icon",
                }

                span {
                    class: "context-menu-label",
                    if is_bookmarked { "Remove from Quick Access" } else { "Add to Quick Access" }
                }
            }

            // === Section 3: File operations ===
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
    fn new_clamps_position_and_flips_submenu_at_corner() {
        // Integration of clamp + flip through the public constructor.
        let data = SidebarContextMenuData::new(
            (10_000, 10_000),
            (1000, 800),
            PathBuf::from("/tmp/example.md"),
            SidebarItemKind::File,
            Vec::new(),
        );
        assert_eq!(
            data.position,
            (
                1000 - VIEWPORT_MARGIN - MENU_WIDTH,
                800 - VIEWPORT_MARGIN - MENU_HEIGHT
            )
        );
        assert!(data.submenu_left);
    }
}
