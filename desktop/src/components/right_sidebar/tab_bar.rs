use dioxus::prelude::*;

use super::RightSidebarTab;
use crate::components::icon::{Icon, IconName};
use crate::state::AppState;

#[component]
pub fn TabBar(
    active_tab: RightSidebarTab,
    on_change: EventHandler<RightSidebarTab>,
    on_pin_toggle: Option<EventHandler<()>>,
) -> Element {
    let state = use_context::<AppState>();
    let is_pinned = state.right_sidebar.read().pinned;
    rsx! {
        div {
            class: "right-sidebar-tabs",

            // Contents tab
            button {
                class: if active_tab == RightSidebarTab::Contents { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                onclick: move |_| on_change.call(RightSidebarTab::Contents),
                span { "Contents" }
            }

            // Search tab
            button {
                class: if active_tab == RightSidebarTab::Search { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                onclick: move |_| on_change.call(RightSidebarTab::Search),
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
