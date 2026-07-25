use std::path::{Path, PathBuf};

use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

use crate::bookmarks::BOOKMARKS;
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

/// Clamp a menu's top-left origin so the whole menu stays within the viewport.
///
/// If the menu is larger than the available space on an axis, it is pinned to
/// `margin` on that axis (the top/left is always visible).
fn clamp_menu_position(
    cursor: (i32, i32),
    menu: (i32, i32),
    viewport: (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let clamp_axis = |pos: i32, size: i32, extent: i32| {
        let max_pos = (extent - margin - size).max(margin);
        pos.clamp(margin, max_pos)
    };
    (
        clamp_axis(cursor.0, menu.0, viewport.0),
        clamp_axis(cursor.1, menu.1, viewport.1),
    )
}

/// Decide whether the submenu flyout should open to the left of the menu
/// instead of the right, to avoid spilling past the viewport's right edge.
fn submenu_opens_left(
    menu_x: i32,
    menu_width: i32,
    submenu_width: i32,
    viewport_width: i32,
    margin: i32,
) -> bool {
    menu_x + menu_width + submenu_width + margin > viewport_width
}

/// Clamp the submenu flyout's top so its full height stays within the viewport.
///
/// Returns the top the flyout should render at: `anchor_y` when it fits, a
/// smaller value when it would spill past the bottom edge, and `margin` when the
/// flyout is taller than the available space (its top stays visible).
fn clamp_submenu_top(anchor_y: i32, submenu_height: i32, viewport_height: i32, margin: i32) -> i32 {
    let max_top = (viewport_height - margin - submenu_height).max(margin);
    anchor_y.clamp(margin, max_top)
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

// ============================================================================
// Helper Components
// ============================================================================

#[derive(Props, Clone, PartialEq)]
struct ContextMenuItemProps {
    label: &'static str,
    #[props(default)]
    shortcut: Option<String>,
    #[props(default)]
    icon: Option<IconName>,
    #[props(default = false)]
    disabled: bool,
    on_click: EventHandler<()>,
}

#[component]
fn ContextMenuItem(props: ContextMenuItemProps) -> Element {
    rsx! {
        div {
            class: if props.disabled { "context-menu-item disabled" } else { "context-menu-item" },
            onclick: move |_| {
                if !props.disabled {
                    props.on_click.call(());
                }
            },

            if let Some(icon) = props.icon {
                Icon {
                    name: icon,
                    size: 14,
                    class: "context-menu-icon",
                }
            }

            span { class: "context-menu-label", "{props.label}" }

            if let Some(shortcut) = props.shortcut {
                span { class: "context-menu-shortcut", "{shortcut}" }
            }
        }
    }
}

#[component]
fn ContextMenuSeparator() -> Element {
    rsx! {
        div { class: "context-menu-separator" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MENU: (i32, i32) = (200, 300);
    const VIEWPORT: (i32, i32) = (1000, 800);
    const MARGIN: i32 = 8;

    #[test]
    fn keeps_position_unchanged_when_menu_fits() {
        // A cursor with room for the whole menu is used as-is.
        let pos = clamp_menu_position((100, 100), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos, (100, 100));
    }

    #[test]
    fn shifts_left_when_menu_overflows_right_edge() {
        // Cursor near the right edge: x is pulled in so the menu's right edge
        // sits at viewport_width - margin.
        let pos = clamp_menu_position((950, 100), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos.0, VIEWPORT.0 - MARGIN - MENU.0); // 1000 - 8 - 200 = 792
        assert_eq!(pos.1, 100);
    }

    #[test]
    fn shifts_up_when_menu_overflows_bottom_edge() {
        // Cursor near the bottom edge: y is pulled in so the menu's bottom edge
        // sits at viewport_height - margin.
        let pos = clamp_menu_position((100, 780), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos.0, 100);
        assert_eq!(pos.1, VIEWPORT.1 - MARGIN - MENU.1); // 800 - 8 - 300 = 492
    }

    #[test]
    fn clamps_bottom_right_corner_into_view() {
        // A corner click keeps the whole menu on-screen on both axes.
        let pos = clamp_menu_position((999, 799), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos, (792, 492));
    }

    #[test]
    fn pins_to_margin_when_menu_larger_than_viewport() {
        // Degenerate viewport smaller than the menu: the top-left stays visible.
        let tiny = (150, 150);
        let pos = clamp_menu_position((999, 999), MENU, tiny, MARGIN);
        assert_eq!(pos, (MARGIN, MARGIN));
    }

    #[test]
    fn never_places_origin_before_margin() {
        // A cursor above/left of the margin is pushed back to the margin.
        let pos = clamp_menu_position((0, 0), MENU, VIEWPORT, MARGIN);
        assert_eq!(pos, (MARGIN, MARGIN));
    }

    #[test]
    fn submenu_opens_right_with_room() {
        // Plenty of horizontal room: the flyout opens to the right.
        assert!(!submenu_opens_left(100, 220, 208, 1000, MARGIN));
    }

    #[test]
    fn submenu_flips_left_near_right_edge() {
        // Menu hugging the right edge: right-side flyout would spill, so flip.
        assert!(submenu_opens_left(700, 220, 208, 1000, MARGIN));
    }

    #[test]
    fn submenu_top_unchanged_when_flyout_fits() {
        // Plenty of room below the anchor: the flyout stays at its anchor.
        let top = clamp_submenu_top(100, 200, 800, MARGIN);
        assert_eq!(top, 100);
    }

    #[test]
    fn submenu_top_shifts_up_near_bottom_edge() {
        // Anchor near the bottom: the top is pulled up so the flyout's bottom
        // sits at viewport_height - margin.
        let top = clamp_submenu_top(700, 200, 800, MARGIN);
        assert_eq!(top, 800 - MARGIN - 200); // 592
    }

    #[test]
    fn submenu_top_pins_to_margin_when_taller_than_viewport() {
        // A flyout taller than the available space keeps its top visible.
        let top = clamp_submenu_top(700, 1000, 800, MARGIN);
        assert_eq!(top, MARGIN);
    }

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
