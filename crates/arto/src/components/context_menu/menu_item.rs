use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};

#[component]
pub fn ContextMenuItem(
    #[props(into)] label: String,
    #[props(default)] shortcut: Option<String>,
    #[props(default)] icon: Option<IconName>,
    #[props(default = false)] disabled: bool,
    on_click: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: if disabled { "context-menu-item disabled" } else { "context-menu-item" },
            onclick: move |_| {
                if !disabled {
                    on_click.call(());
                }
            },

            if let Some(icon) = icon {
                Icon {
                    name: icon,
                    size: 14,
                    class: "context-menu-icon",
                }
            }

            span { class: "context-menu-label", "{label}" }

            if let Some(shortcut) = shortcut {
                span { class: "context-menu-shortcut", "{shortcut}" }
            }
        }
    }
}

#[component]
pub fn ContextMenuSeparator() -> Element {
    rsx! {
        div { class: "context-menu-separator" }
    }
}

/// Reusable submenu component with hover-to-open behavior.
#[component]
pub fn ContextMenuSubmenu(label: String, children: Element) -> Element {
    let mut show = use_signal(|| false);

    rsx! {
        div {
            class: "context-menu-item has-submenu",
            onmouseenter: move |_| show.set(true),
            onmouseleave: move |_| show.set(false),

            span { class: "context-menu-label", "{label}" }
            span { class: "submenu-arrow", "›" }

            if *show.read() {
                div {
                    class: "context-submenu",
                    {children}
                }
            }
        }
    }
}
