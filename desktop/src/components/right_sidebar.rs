use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

mod contents_tab;
mod search_tab;
mod tab_bar;

use contents_tab::ContentsTab;
use search_tab::SearchTab;
use tab_bar::TabBar;

use crate::markdown::HeadingInfo;
use crate::state::{AppState, FocusedPanel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RightSidebarTab {
    #[default]
    Contents,
    Search,
}

#[derive(Props, Clone, PartialEq)]
pub struct RightSidebarProps {
    pub headings: Vec<HeadingInfo>,
}

#[component]
pub fn RightSidebar(props: RightSidebarProps) -> Element {
    let mut state = use_context::<AppState>();
    let is_open = *state.right_sidebar_open.read();
    let width = *state.right_sidebar_width.read();
    let active_tab = *state.right_sidebar_tab.read();
    let is_panel_focused = *state.focused_panel.read() == FocusedPanel::RightSidebar;
    let zoom_level = state.zoom_level;
    let is_resizing = use_signal(|| false);

    // Get data for each tab
    let headings = props.headings.clone();

    let outer_style = if is_open {
        format!("width: {}px;", width)
    } else {
        "width: 0;".to_string()
    };

    let inner_style = format!("zoom: {};", zoom_level());

    rsx! {
        div {
            class: "right-sidebar",
            class: if is_open { "visible" },
            class: if is_resizing() { "resizing" },
            class: if is_panel_focused { "panel-focused" },
            style: "{outer_style}",

            // Resize handle
            if is_open {
                RightSidebarResizeHandle { is_resizing }
            }

            // Inner wrapper with zoom applied
            div {
                class: "right-sidebar-inner",
                style: "{inner_style}",

                // Tab bar
                TabBar {
                    active_tab,
                    on_change: move |tab| state.set_right_sidebar_tab(tab),
                }

                // Tab content
                div {
                    class: "right-sidebar-content",

                    match active_tab {
                        RightSidebarTab::Contents => rsx! { ContentsTab { headings } },
                        RightSidebarTab::Search => rsx! { SearchTab {} },
                    }
                }
            }
        }
    }
}

#[component]
fn RightSidebarResizeHandle(is_resizing: Signal<bool>) -> Element {
    use dioxus::document;
    let mut state = use_context::<AppState>();

    rsx! {
        div {
            class: "right-sidebar-resize-handle",
            class: if is_resizing() { "resizing" },
            onmousedown: move |evt| {
                evt.prevent_default();
                is_resizing.set(true);
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
                                    let new_width = (start_width + delta).clamp(150.0, max_width);
                                    state.set_right_sidebar_width(new_width);
                                }
                            }
                            "end" => {
                                is_resizing.set(false);
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
