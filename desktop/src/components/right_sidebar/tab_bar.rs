use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::state::{AppState, SidebarPanel};

#[component]
pub fn TabBar(
    active_panel: SidebarPanel,
    on_change: EventHandler<SidebarPanel>,
    on_pin_toggle: Option<EventHandler<()>>,
) -> Element {
    let state = use_context::<AppState>();
    let is_pinned = *state.right_sidebar_pinned.read();
    rsx! {
        div {
            class: "right-sidebar-tabs",

            // Contents tab
            button {
                class: if active_panel == SidebarPanel::Directory { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                onclick: move |_| on_change.call(SidebarPanel::Directory),
                span { "Directory" }
            }

            button {
                class: if active_panel == SidebarPanel::Contents { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                onclick: move |_| on_change.call(SidebarPanel::Contents),
                span { "Contents" }
            }

            button {
                class: if active_panel == SidebarPanel::Search { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                onclick: move |_| on_change.call(SidebarPanel::Search),
                span { "Search" }
            }

            // Pin/Unpin button
            if let Some(handler) = on_pin_toggle {
                button {
                    class: "right-sidebar-pin-button",
                    class: if is_pinned { "pinned" },
                    title: if is_pinned { "Unpin sidebar" } else { "Pin sidebar" },
                    onclick: move |_| handler.call(()),
                    Icon {
                        name: if is_pinned { IconName::PinFilled } else { IconName::Pin },
                        size: 20,
                    }
                }
            }
        }
    }
}
