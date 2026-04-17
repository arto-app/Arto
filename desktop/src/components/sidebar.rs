pub mod context_menu;
pub mod file_explorer;
pub mod quick_access;

use dioxus::document;
use dioxus::prelude::*;

use super::right_sidebar::{ContentsTab, SearchTab};
use crate::state::{AppState, FocusedPanel, SidebarPanel};

#[component]
pub fn Sidebar(
    on_pin_toggle: Option<EventHandler<()>>,
    on_resize_change: Option<EventHandler<bool>>,
) -> Element {
    let mut state = use_context::<AppState>();
    let sidebar_state = state.sidebar.read();
    let width = sidebar_state.width;
    let zoom_level = sidebar_state.zoom_level;
    drop(sidebar_state);
    let active_panel = *state.left_sidebar_panel.read();
    let focused_panel = *state.focused_panel.read();
    let is_panel_focused =
        focused_panel == FocusedPanel::LeftSidebar || focused_panel == FocusedPanel::QuickAccess;
    let mut is_resizing = use_signal(|| false);
    let headings = state.right_sidebar_headings.read().clone();
    let toc_cursor = *state.toc_cursor.read();
    let is_pinned = state.sidebar.read().pinned;

    let outer_style = format!("width: {}px;", width);
    let inner_style = format!("zoom: {};", zoom_level);

    rsx! {
        div {
            class: "left-sidebar visible",
            class: if is_resizing() { "resizing" },
            class: if is_panel_focused { "panel-focused" },
            style: "{outer_style}",

            // Inner wrapper with zoom applied
            div {
                class: "left-sidebar-inner",
                style: "{inner_style}",

                div {
                    class: "right-sidebar-tabs",

                    button {
                        class: if active_panel == SidebarPanel::Directory { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                        onclick: move |_| state.set_left_sidebar_panel(SidebarPanel::Directory),
                        span { "Directory" }
                    }

                    button {
                        class: if active_panel == SidebarPanel::Contents { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                        onclick: move |_| state.set_left_sidebar_panel(SidebarPanel::Contents),
                        span { "Contents" }
                    }

                    button {
                        class: if active_panel == SidebarPanel::Search { "right-sidebar-tab active" } else { "right-sidebar-tab" },
                        onclick: move |_| state.set_left_sidebar_panel(SidebarPanel::Search),
                        span { "Search" }
                    }

                    if let Some(handler) = on_pin_toggle {
                        button {
                            class: "right-sidebar-pin-button",
                            class: if is_pinned { "pinned" },
                            title: if is_pinned { "Unpin sidebar" } else { "Pin sidebar" },
                            onclick: move |_| handler.call(()),
                            crate::components::icon::Icon {
                                name: if is_pinned {
                                    crate::components::icon::IconName::PinFilled
                                } else {
                                    crate::components::icon::IconName::Pin
                                },
                                size: 20,
                            }
                        }
                    }
                }

                match active_panel {
                    SidebarPanel::Directory => rsx! {
                        file_explorer::FileExplorer {
                            on_pin_toggle: None,
                        }
                    },
                    SidebarPanel::Contents => rsx! {
                        ContentsTab {
                            headings,
                            cursor_index: if focused_panel == FocusedPanel::LeftSidebar {
                                toc_cursor
                            } else {
                                None
                            },
                        }
                    },
                    SidebarPanel::Search => rsx! { SearchTab {} },
                }
            }

            // Resize handle
            div {
                class: "left-sidebar-resize-handle",
                class: if is_resizing() { "resizing" },
                    onmousedown: move |evt| {
                        evt.prevent_default();
                        is_resizing.set(true);
                        if let Some(handler) = on_resize_change {
                            handler.call(true);
                        }
                        let start_x = evt.page_coordinates().x;
                        let start_width = state.sidebar.read().width;

                        spawn(async move {
                            #[derive(serde::Deserialize)]
                            struct DragMessage {
                                r#type: String,
                                x: Option<f64>,
                                #[serde(rename = "maxWidth")]
                                max_width: Option<f64>,
                            }

                            let mut eval = document::eval(r#"
                                new Promise((resolve) => {
                                    const handleMouseMove = (e) => {
                                        const maxWidth = window.innerWidth * 0.7;
                                        dioxus.send({ type: 'move', x: e.pageX, maxWidth });
                                    };
                                    const handleMouseUp = () => {
                                        document.removeEventListener('mousemove', handleMouseMove);
                                        document.removeEventListener('mouseup', handleMouseUp);
                                        dioxus.send({ type: 'end' });
                                        resolve();
                                    };
                                    document.addEventListener('mousemove', handleMouseMove);
                                    document.addEventListener('mouseup', handleMouseUp);
                                })
                            "#);

                            while let Ok(msg) = eval.recv::<DragMessage>().await {
                                match msg.r#type.as_str() {
                                    "move" => {
                                        if let Some(x) = msg.x {
                                            let delta = x - start_x;
                                            let max_width = msg.max_width.unwrap_or(600.0);
                                            let new_width = (start_width + delta).clamp(200.0, max_width);
                                            state.sidebar.write().width = new_width;
                                        }
                                    }
                                    "end" => {
                                        is_resizing.set(false);
                                        if let Some(handler) = on_resize_change {
                                            handler.call(false);
                                        }
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        });
                    }
            }
        }
    }
}
