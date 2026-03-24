use std::path::PathBuf;

use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

use crate::bookmarks::BOOKMARKS;
use crate::components::icon::{Icon, IconName};
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};

#[derive(Clone, Copy, PartialEq)]
pub enum SidebarItemKind {
    File,
    Directory,
}

#[component]
pub fn SidebarContextMenu(
    position: (i32, i32),
    path: PathBuf,
    kind: SidebarItemKind,
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
                        class: "context-submenu",

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
                label: if cfg!(windows) { "Reveal in File Explorer" } else { "Reveal in Finder" },
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
    #[props(into)]
    label: String,
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
