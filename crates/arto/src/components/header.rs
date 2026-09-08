mod breadcrumb_menu;

use dioxus::prelude::*;

use crate::components::app_menu::AppMenu;
use crate::components::bookmark_button::BookmarkButton;
use crate::components::header::breadcrumb_menu::Breadcrumb;
use crate::components::icon::{Icon, IconName};
use crate::components::theme_selector::ThemeSelector;
use crate::state::AppState;

#[component]
pub fn Header() -> Element {
    let mut state = use_context::<AppState>();

    let mut is_menu_open = use_signal(|| false);

    let document = state.document();
    let file_path = document.file().map(|file| file.to_path_buf());
    let file = document.display_name();

    let is_reloading = use_signal(|| false);
    let mut is_reloading_write = is_reloading;

    let on_reload = move |_| {
        // Set reloading state
        is_reloading_write.set(true);

        state.reload_document();

        // Reset reloading state after animation
        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
            is_reloading_write.set(false);
        });
    };

    // Copy feedback state
    let mut is_copied = use_signal(|| false);

    rsx! {
        div {
            class: "header",
            style: "position: relative;",

            // File name display (left side) with navigation buttons
            div {
                class: "header-left",

                // Everything the app can do, in the window it applies to.
                // macOS has a menu bar of its own, but it belongs to whichever
                // window is frontmost and sits at the top of the screen rather
                // than at the top of the window, so this one is drawn there
                // too.
                button {
                    class: "nav-button app-menu-button",
                    class: if *is_menu_open.read() { "active" },
                    // No `title`: the menu opens directly under this glyph,
                    // and the tooltip would land on its first item. Taking the
                    // attribute away once the menu is open is too late —
                    // WebKit reads it when the pointer arrives and shows it a
                    // moment later, and by then the pointer has not moved, so
                    // the text it already read is what appears. A control that
                    // opens a panel under itself gets no tooltip at all; the
                    // panel says what the tooltip would.
                    "aria-label": "Menu",
                    onclick: move |_| is_menu_open.toggle(),
                    Icon { name: IconName::Menu2 }
                }

                // The name of what is being read is also the way back to
                // what was read before it.
                Breadcrumb { label: file }

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
                            }
                        }

                        // Reload button (next to copy button)
                        button {
                            class: "nav-button reload-button",
                            class: if *is_reloading.read() { "reloading" },
                            onclick: on_reload,
                            title: "Reload file",
                            Icon { name: IconName::Refresh }
                        }
                    }
                }
            }

            // Right side controls
            div {
                class: "header-right",

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
                    Icon { name: IconName::Search }
                }

                // Full-width content toggle
                button {
                    class: "nav-button full-width-button",
                    class: if *state.content_full_width.read() { "active" },
                    title: if *state.content_full_width.read() { "Disable full-width content" } else { "Full-width content" },
                    onclick: move |_| state.toggle_content_full_width(),
                    Icon {
                        name: if *state.content_full_width.read() { IconName::ViewportNarrow } else { IconName::ViewportWide },
                    }
                }

                // Theme selector
                ThemeSelector { current_theme: state.current_theme }
            }

            if *is_menu_open.read() {
                AppMenu {
                    on_close: move |_| is_menu_open.set(false),
                }
            }

        }
    }
}
