pub mod context_menu;
pub mod file_explorer;
pub mod quick_access;

use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::state::AppState;

#[component]
pub fn Sidebar() -> Element {
    let mut state = use_context::<AppState>();
    let sidebar_state = state.sidebar.read();
    let is_visible = sidebar_state.open;
    let width = sidebar_state.width;
    let zoom_level = sidebar_state.zoom_level;
    let mut is_resizing = use_signal(|| false);

    let outer_style = if is_visible {
        format!("width: {}px;", width)
    } else {
        "width: 0;".to_string()
    };

    let inner_style = format!("zoom: {};", zoom_level);

    rsx! {
        div {
            class: "left-sidebar",
            class: if is_visible { "visible" },
            class: if is_resizing() { "resizing" },
            style: "{outer_style}",

            // Panel header with close button (outside zoom wrapper)
            div {
                class: "sidebar-panel-header",
                button {
                    class: "sidebar-panel-close-button",
                    title: "Close Sidebar",
                    onclick: move |_| {
                        state.toggle_sidebar();
                    },
                    Icon {
                        name: IconName::SidebarLeftCollapse,
                        size: 18,
                    }
                }
            }

            // Inner wrapper with zoom applied
            div {
                class: "left-sidebar-inner",
                style: "{inner_style}",

                // File explorer content (always mounted for animation)
                file_explorer::FileExplorer {}
            }

            // Resize handle (only when visible)
            if is_visible {
                div {
                    class: "left-sidebar-resize-handle",
                    class: if is_resizing() { "resizing" },
                    onmousedown: move |evt| {
                        evt.prevent_default();
                        is_resizing.set(true);
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
}
