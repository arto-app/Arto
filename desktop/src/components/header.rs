use dioxus::prelude::*;

use crate::components::ai_button::AiButton;
use crate::components::bookmark_button::BookmarkButton;
use crate::components::icon::{Icon, IconName};
use crate::components::theme_selector::ThemeSelector;
use crate::state::AppState;

#[cfg(target_os = "windows")]
use crate::components::win_hamburger::WindowsMenu;

#[component]
pub fn Header() -> Element {
    let mut state = use_context::<AppState>();

    let mut is_menu_open = use_signal(|| false);

    let current_tab = state.current_tab();
    let file_path = current_tab.as_ref().and_then(|tab| tab.file());
    let file = file_path
        .as_ref()
        .map(|f| {
            f.file_name()
                .unwrap_or(f.as_os_str())
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| "No file opened".to_string());

    let can_go_back = current_tab
        .as_ref()
        .is_some_and(|tab| tab.history.can_go_back());
    let can_go_forward = current_tab
        .as_ref()
        .is_some_and(|tab| tab.history.can_go_forward());

    let on_back = move |_| {
        state.save_scroll_and_go_back();
    };

    let on_forward = move |_| {
        state.save_scroll_and_go_forward();
    };

    let is_reloading = use_signal(|| false);
    let mut is_reloading_write = is_reloading;

    let on_reload = move |_| {
        // Set reloading state
        is_reloading_write.set(true);

        state.reload_current_tab();

        // Reset reloading state after animation
        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
            is_reloading_write.set(false);
        });
    };

    // Copy feedback state
    let mut is_copied = use_signal(|| false);

    let menu_overlay = {
        #[cfg(target_os = "windows")]
        {
            if *is_menu_open.read() {
                rsx! {
                    WindowsMenu {
                        on_close: move |_| is_menu_open.set(false),
                    }
                }
            } else {
                rsx! {}
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            rsx! {}
        }
    };

    rsx! {
        div {
            class: "header",
            style: "position: relative;",

            // File name display (left side) with navigation buttons
            div {
                class: "header-left",

                    // Hamburger Menu Button
                    if cfg!(target_os = "windows") {
                        button {
                            class: "nav-button hamburger-button",
                            class: if *is_menu_open.read() { "active" },
                            title: "Menu",
                            onclick: move |_| is_menu_open.toggle(),
                            Icon { name: IconName::Menu2 }
                        }
                    }


                // Back button
                button {
                    class: "nav-button",
                    disabled: !can_go_back,
                    onclick: on_back,
                    Icon { name: IconName::ChevronLeft }
                }

                // Forward button
                button {
                    class: "nav-button",
                    disabled: !can_go_forward,
                    onclick: on_forward,
                    Icon { name: IconName::ChevronRight }
                }

                // File name
                span {
                    class: "file-name",
                    "{file}"
                }

                div {
                    class: "file-action-buttons",

                    // Bookmark, copy path, and reload buttons (shown on hover)
                    if let Some(path) = file_path {
                        // Bookmark button
                        BookmarkButton { path: path.to_path_buf() }

                        button {
                            class: "nav-button copy-button",
                            class: if *is_copied.read() { "copied" },
                            title: "Copy full path",
                            onclick: {
                                let path_str = path.to_string_lossy().to_string();
                                move |_| {
                                    crate::utils::clipboard::copy_text(&path_str);
                                    // Show success feedback
                                    is_copied.set(true);
                                    spawn(async move {
                                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                        is_copied.set(false);
                                    });
                                }
                            },
                            Icon {
                                name: if *is_copied.read() { IconName::Check } else { IconName::Copy },
                                size: 14,
                            }
                        }

                        // Reload button (next to copy button)
                        button {
                            class: "nav-button reload-button",
                            class: if *is_reloading.read() { "reloading" },
                            onclick: on_reload,
                            title: "Reload file",
                            Icon { name: IconName::Refresh, size: 14 }
                        }
                    }
                }
            }

            // Right side controls
            div {
                class: "header-right",

                // AI integrations (only rendered when at least one provider is configured)
                AiButton {}

                // Search button
                button {
                    class: "nav-button search-button",
                    class: if *state.search_open.read() { "active" },
                    title: "Search in page",
                    onclick: move |_| {
                        let was_closed = !*state.search_open.read();
                        state.toggle_search();
                        if was_closed {
                            // Focus the search input after opening
                            spawn(async {
                                let _ = document::eval(
                                    "document.querySelector('.search-input')?.focus()",
                                )
                                .await;
                            });
                        }
                    },
                    Icon { name: IconName::Search, size: 20 }
                }

                // Theme selector
                ThemeSelector { current_theme: state.current_theme }
            }

            {menu_overlay}

        }
    }
}
