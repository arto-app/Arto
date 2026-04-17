use dioxus::prelude::*;

mod contents_tab;
mod search_tab;
mod tab_bar;

pub(crate) use contents_tab::ContentsTab;
pub(crate) use search_tab::SearchTab;
use tab_bar::TabBar;

use crate::markdown::HeadingInfo;
use crate::state::{AppState, FocusedPanel, SidebarPanel};

#[derive(Props, Clone, PartialEq)]
pub struct RightSidebarProps {
    pub headings: Vec<HeadingInfo>,
    pub on_pin_toggle: Option<EventHandler<()>>,
    pub on_resize_change: Option<EventHandler<bool>>,
}

#[component]
pub fn RightSidebar(props: RightSidebarProps) -> Element {
    let mut state = use_context::<AppState>();
    let width = *state.right_sidebar_width.read();
    let active_panel = *state.right_sidebar_panel.read();
    let zoom_level = *state.right_sidebar_zoom_level.read();
    let is_panel_focused = *state.focused_panel.read() == FocusedPanel::RightSidebar;
    let toc_cursor = *state.toc_cursor.read();
    let is_resizing = use_signal(|| false);

    // Get data for each tab
    let headings = props.headings.clone();

    let outer_style = format!("width: {}px;", width);
    let inner_style = format!("zoom: {};", zoom_level);

    rsx! {
        div {
            class: "right-sidebar visible",
            class: if is_resizing() { "resizing" },
            class: if is_panel_focused { "panel-focused" },
            style: "{outer_style}",

            // Resize handle
            RightSidebarResizeHandle { is_resizing, on_resize_change: props.on_resize_change }

            // Inner wrapper with zoom applied
            div {
                class: "right-sidebar-inner",
                style: "{inner_style}",

                // Tab bar
                TabBar {
                    active_panel,
                    on_change: move |panel| state.set_right_sidebar_panel(panel),
                    on_pin_toggle: props.on_pin_toggle,
                }

                // Tab content
                div {
                    class: "right-sidebar-content",

                    match active_panel {
                        SidebarPanel::Directory => rsx! {
                            crate::components::sidebar::file_explorer::FileExplorer {
                                on_pin_toggle: None,
                            }
                        },
                        SidebarPanel::Contents => rsx! {
                            ContentsTab {
                                headings,
                                cursor_index: if is_panel_focused { toc_cursor } else { None },
                            }
                        },
                        SidebarPanel::Search => rsx! { SearchTab {} },
                    }
                }
            }
        }
    }
}

#[component]
fn RightSidebarResizeHandle(
    is_resizing: Signal<bool>,
    on_resize_change: Option<EventHandler<bool>>,
) -> Element {
    use dioxus::document;
    let mut state = use_context::<AppState>();

    rsx! {
        div {
            class: "right-sidebar-resize-handle",
            class: if is_resizing() { "resizing" },
            onmousedown: move |evt| {
                evt.prevent_default();
                is_resizing.set(true);
                if let Some(handler) = on_resize_change {
                    handler.call(true);
                }
                let start_x = evt.page_coordinates().x;
                let start_width = *state.right_sidebar_width.read();

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
                                const maxWidth = window.innerWidth * 0.5;
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
                                    // Right sidebar resizes from left edge, so delta is inverted
                                    let delta = start_x - x;
                                    let max_width = msg.max_width.unwrap_or(400.0);
                                    let new_width = (start_width + delta).clamp(220.0, max_width);
                                    state.set_right_sidebar_width(new_width);
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
