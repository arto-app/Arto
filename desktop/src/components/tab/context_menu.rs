use std::path::PathBuf;

use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};

#[component]
pub fn TabContextMenu(
    position: (i32, i32),
    file_path: Option<PathBuf>,
    is_pinned: bool,
    on_close: EventHandler<()>,
    on_close_tab: EventHandler<()>,
    on_close_others: EventHandler<()>,
    on_close_all: EventHandler<()>,
    on_toggle_pin: EventHandler<()>,
    on_copy_path: EventHandler<()>,
    on_reload: EventHandler<()>,
    on_set_parent_as_root: EventHandler<()>,
    on_open_in_new_window: EventHandler<()>,
    on_move_to_window: EventHandler<WindowId>,
    on_reveal_in_finder: EventHandler<()>,
    other_windows: Vec<(WindowId, String)>,
    #[props(default = false)] disabled: bool,
) -> Element {
    let mut show_submenu = use_signal(|| false);
    let has_file = file_path.is_some();
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Content, action);

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

            // === Section 1: Close operations ===
            ContextMenuItem {
                label: "Close",
                shortcut: shortcut("tab.close"),
                icon: Some(IconName::Close),
                disabled: is_pinned,
                on_click: move |_| on_close_tab.call(()),
            }

            ContextMenuItem {
                label: "Close Others",
                shortcut: shortcut("tab.close_others"),
                on_click: move |_| on_close_others.call(()),
            }

            ContextMenuItem {
                label: "Close All",
                shortcut: shortcut("tab.close_all"),
                on_click: move |_| on_close_all.call(()),
            }

            // === Section 2: Pin ===
            ContextMenuSeparator {}

            ContextMenuItem {
                label: if is_pinned { "Unpin Tab" } else { "Pin Tab" },
                shortcut: shortcut("tab.toggle_pin"),
                icon: Some(if is_pinned { IconName::PinnedOff } else { IconName::Pin }),
                on_click: move |_| on_toggle_pin.call(()),
            }

            // === Section 3: Window operations ===
            ContextMenuSeparator {}

            ContextMenuItem {
                label: "Open in New Window",
                shortcut: shortcut("tab.open_in_new_window"),
                disabled: disabled,
                on_click: move |_| on_open_in_new_window.call(()),
            }

            // Move to Window (with submenu)
            div {
                class: if disabled { "context-menu-item disabled" } else { "context-menu-item has-submenu" },
                onmouseenter: move |_| {
                    if !disabled {
                        show_submenu.set(true);
                    }
                },
                onmouseleave: move |_| show_submenu.set(false),

                span { class: "context-menu-label", "Move to Window" }
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

            // === Section 4: File operations ===
            ContextMenuSeparator {}

            ContextMenuItem {
                label: "Copy File Path",
                shortcut: shortcut("clipboard.copy_file_path"),
                icon: Some(IconName::Copy),
                disabled: !has_file,
                on_click: move |_| on_copy_path.call(()),
            }

            ContextMenuItem {
                label: if cfg!(windows) { "Reveal in File Explorer" } else { "Reveal in Finder" },
                shortcut: shortcut("file.reveal_in_finder"),
                icon: Some(IconName::Folder),
                disabled: !has_file,
                on_click: move |_| on_reveal_in_finder.call(()),
            }

            // === Section 5: Tab operations ===
            ContextMenuSeparator {}

            ContextMenuItem {
                label: "Set Parent as Root",
                shortcut: shortcut("file.set_parent_as_root"),
                icon: Some(IconName::FolderOpen),
                disabled: !has_file,
                on_click: move |_| on_set_parent_as_root.call(()),
            }

            ContextMenuItem {
                label: "Reload",
                shortcut: shortcut("window.reload"),
                icon: Some(IconName::Refresh),
                disabled: !has_file,
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
