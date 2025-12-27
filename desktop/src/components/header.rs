use dioxus::document;
use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::components::theme_selector::ThemeSelector;
use crate::state::AppState;

#[component]
pub fn Header() -> Element {
    let mut state = use_context::<AppState>();

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

    let is_sidebar_open = state.sidebar.read().open;

    let on_back = move |_| {
        state.update_current_tab(|tab| {
            if let Some(path) = tab.history.go_back() {
                tab.content = crate::state::TabContent::File(path.to_owned());
            }
        });
    };

    let on_forward = move |_| {
        state.update_current_tab(|tab| {
            if let Some(path) = tab.history.go_forward() {
                tab.content = crate::state::TabContent::File(path.to_owned());
            }
        });
    };

    let is_reloading = use_signal(|| false);
    let mut is_reloading_write = is_reloading;

    let on_reload = move |_| {
        // Set reloading state
        is_reloading_write.set(true);

        state.update_current_tab(|tab| {
            if let Some(path) = tab.file() {
                // Reload by reassigning the same file path
                tab.content = crate::state::TabContent::File(path.to_owned());
            }
        });

        // Reset reloading state after animation
        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
            is_reloading_write.set(false);
        });
    };

    // Check if there's a file to reload/copy
    let can_reload = file_path.is_some();

    // Copy feedback state
    let mut is_copied = use_signal(|| false);

    rsx! {
        div {
            class: "header",

            // File name display (left side) with navigation buttons
            div {
                class: "header-left",

                // Sidebar toggle button
                button {
                    class: "sidebar-toggle-button",
                    class: if is_sidebar_open { "active" },
                    onclick: move |_| {
                        state.toggle_sidebar();
                    },
                    Icon {
                        name: IconName::Sidebar,
                        size: 20,
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

                // Copy path button
                if let Some(path) = file_path {
                    button {
                        class: "nav-button copy-button",
                        class: if *is_copied.read() { "copied" },
                        title: "Copy full path",
                        onclick: {
                            let path_str = path.to_string_lossy().to_string();
                            move |_| {
                                let escaped = path_str.replace('\\', "\\\\").replace('`', "\\`");
                                spawn(async move {
                                    let js = format!("navigator.clipboard.writeText(`{}`)", escaped);
                                    let _ = document::eval(&js).await;
                                    // Show success feedback
                                    is_copied.set(true);
                                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                    is_copied.set(false);
                                });
                            }
                        },
                        Icon {
                            name: if *is_copied.read() { IconName::Check } else { IconName::Copy },
                            size: 16,
                        }
                    }
                }
            }

            // Right side controls
            div {
                class: "header-right",

                // Reload button
                button {
                    class: "nav-button reload-button",
                    class: if *is_reloading.read() { "reloading" },
                    disabled: !can_reload,
                    onclick: on_reload,
                    title: "Reload file",
                    Icon { name: IconName::Refresh }
                }

                // Theme selector
                ThemeSelector { current_theme: state.current_theme }
            }
        }
    }
}
