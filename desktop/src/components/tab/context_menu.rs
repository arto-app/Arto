use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

#[component]
pub fn TabContextMenu(
    position: (i32, i32),
    on_close: EventHandler<()>,
    on_open_in_new_window: EventHandler<()>,
    on_move_to_window: EventHandler<WindowId>,
    other_windows: Vec<(WindowId, String)>,
    #[props(default = false)] disabled: bool,
) -> Element {
    let mut show_submenu = use_signal(|| false);

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

            // Open in New Window
            div {
                class: if disabled { "context-menu-item disabled" } else { "context-menu-item" },
                onclick: move |_| {
                    if !disabled {
                        on_open_in_new_window.call(());
                    }
                },
                "Open in New Window"
            }

            // Move to Window (with submenu)
            div {
                class: if disabled { "context-menu-item disabled" } else { "context-menu-item" },
                onmouseenter: move |_| {
                    if !disabled {
                        show_submenu.set(true);
                    }
                },
                onmouseleave: move |_| show_submenu.set(false),

                "Move to Window"
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
        }
    }
}
