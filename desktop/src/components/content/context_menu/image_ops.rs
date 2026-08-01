use dioxus::prelude::*;

use crate::components::context_menu::{ContextMenuItem, ContextMenuSubmenu};
use crate::keybindings::{shortcut_hint_for_context_action, KeyContext};

/// "Copy Image As..." submenu: Image / Image with Background / Markdown / Path
#[component]
pub(super) fn CopyImageAsSubmenu(
    src: String,
    alt: Option<String>,
    on_close: EventHandler<()>,
) -> Element {
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Content, action);

    rsx! {
        ContextMenuSubmenu {
            label: "Copy Image As...",

            ContextMenuItem {
                label: "Image",
                on_click: {
                    let src = src.clone();
                    move |_| {
                        let src = src.clone();
                        spawn(async move {
                            crate::keybindings::dispatcher::copy_image_from_src(src, false).await;
                        });
                        on_close.call(());
                    }
                },
            }

            ContextMenuItem {
                label: "Image with Background",
                shortcut: shortcut("clipboard.copy_image_with_background"),
                on_click: {
                    let src = src.clone();
                    move |_| {
                        let src = src.clone();
                        spawn(async move {
                            crate::keybindings::dispatcher::copy_image_from_src(src, true).await;
                        });
                        on_close.call(());
                    }
                },
            }

            ContextMenuItem {
                label: "Markdown",
                on_click: {
                    let alt_text = alt.as_deref().unwrap_or("").to_string();
                    let src = src.clone();
                    move |_| {
                        let md = format!("![{}]({})", alt_text, src);
                        crate::utils::clipboard::copy_text(&md);
                        crate::keybindings::dispatcher::show_action_feedback("Copied");
                        on_close.call(());
                    }
                },
            }

            ContextMenuItem {
                label: "Path",
                shortcut: shortcut("clipboard.copy_image_path"),
                on_click: {
                    let src = src.clone();
                    move |_| {
                        crate::utils::clipboard::copy_text(&src);
                        crate::keybindings::dispatcher::show_action_feedback("Copied");
                        on_close.call(());
                    }
                },
            }
        }
    }
}

/// "Copy Image As..." submenu: Image / Image with Background
#[component]
pub(super) fn CopySpecialBlockAsSubmenu(is_mermaid: bool, on_close: EventHandler<()>) -> Element {
    let shortcut = |action| shortcut_hint_for_context_action(KeyContext::Content, action);

    rsx! {
        ContextMenuSubmenu {
            label: "Copy Image As...",

            ContextMenuItem {
                label: "Image",
                on_click: {
                    move |_| {
                        super::copy_special_block_image(is_mermaid, false);
                        on_close.call(());
                    }
                },
            }

            ContextMenuItem {
                label: "Image with Background",
                shortcut: shortcut("clipboard.copy_image_with_background"),
                on_click: {
                    move |_| {
                        super::copy_special_block_image(is_mermaid, true);
                        on_close.call(());
                    }
                },
            }
        }
    }
}
