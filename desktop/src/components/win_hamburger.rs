#![cfg(target_os = "windows")]

use crate::components::icon::{Icon, IconName};
use crate::state::AppState;
use dioxus::prelude::*;

#[component]
pub fn WindowsMenu(on_close: EventHandler<()>) -> Element {
    let mut state = use_context::<AppState>();

    // Helper to get keyboard shortcut hints
    let shortcut = |action| crate::keybindings::shortcut_hint_for_global_action(action);

    // Get information on the currently open file (for invalidation determination)
    let current_tab = state.current_tab();
    let current_file = current_tab
        .as_ref()
        .and_then(|t| t.file().map(|f| f.to_path_buf()));
    let has_file = current_file.is_some();

    let close = move || on_close.call(());

    rsx! {
        // Transparent background to close when clicking outside menu
        div {
            class: "context-menu-backdrop",
            style: "position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; z-index: 998;",
            onclick: move |_| close(),
        }

        // Menu body
        div {
            class: "context-menu",
            style: "position: absolute; left: 12px; top: 40px; z-index: 999;",
            onclick: move |evt| evt.stop_propagation(),

            // === Arto (App) ===
            HeaderMenuItem { label: "About Arto", shortcut: shortcut("app.about"), on_click: move |_| {
                crate::components::content::set_preferences_tab_to_about();
                state.open_preferences();
                close();
            } }
            HeaderMenuItem { label: "Preferences...", shortcut: shortcut("file.preferences"), icon: Some(IconName::Gear), on_click: move |_| {
                state.open_preferences();
                close();
            } }

            HeaderMenuSeparator {}

            // === File ===
            HeaderSubmenu { label: "File",
                HeaderMenuItem { label: "New Window", shortcut: shortcut("window.new"), on_click: move |_| {
                    crate::window::create_main_window_sync(&dioxus::desktop::window(), crate::state::Tab::default(), crate::window::CreateMainWindowConfigParams::default());
                    close();
                } }
                HeaderMenuItem { label: "New Tab", shortcut: shortcut("tab.new"), icon: Some(IconName::Add), on_click: move |_| {
                    state.add_empty_tab(true);
                    close();
                } }
                HeaderMenuSeparator {}
                HeaderMenuItem { label: "Open File...", shortcut: shortcut("file.open"), icon: Some(IconName::File), on_click: move |_| {
                    if let Some(file) = rfd::FileDialog::new().add_filter("Markdown", &["md", "markdown"]).pick_file() {
                        state.open_file(file);
                    }
                    close();
                } }
                HeaderMenuItem { label: "Open Directory...", shortcut: shortcut("file.open_directory"), icon: Some(IconName::FolderOpen), on_click: move |_| {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.set_root_directory(dir);
                    }
                    close();
                } }
                HeaderMenuSeparator {}
                HeaderMenuItem { label: "Copy File Path", shortcut: shortcut("clipboard.copy_file_path"), icon: Some(IconName::Copy), disabled: !has_file, on_click: { let f = current_file.clone(); move |_| {
                    if let Some(file) = &f { crate::utils::clipboard::copy_text(file.to_string_lossy()); }
                    close();
                } } }
                HeaderMenuItem { label: "Reveal in Finder", shortcut: shortcut("file.reveal_in_finder"), icon: Some(IconName::Folder), disabled: !has_file, on_click: { let f = current_file.clone(); move |_| {
                    if let Some(file) = &f { crate::utils::file_operations::reveal_in_finder(file); }
                    close();
                } } }
                HeaderMenuSeparator {}
                HeaderMenuItem { label: "Close Tab", shortcut: shortcut("tab.close"), on_click: move |_| {
                    let active = *state.active_tab.read();
                    state.close_tab(active);
                    close();
                } }
                HeaderMenuItem { label: "Close All Tabs", shortcut: shortcut("tab.close_all"), on_click: move |_| {
                    let mut tabs = state.tabs.write();
                    tabs.clear();
                    tabs.push(crate::state::Tab::default());
                    state.active_tab.set(0);
                    close();
                } }
                HeaderMenuItem { label: "Close Window", shortcut: shortcut("window.close"), on_click: move |_| {
                    dioxus::desktop::window().close();
                } }
                HeaderMenuSeparator {}
                HeaderMenuItem { label: "Print...", shortcut: shortcut("file.print"), on_click: { let f = current_file.clone(); move |_| {
                    close();
                    crate::utils::print::print_window(f.clone());
                } } }
            }

            // === Edit ===
            HeaderSubmenu { label: "Edit",
                HeaderMenuItem { label: "Find...", shortcut: shortcut("search.open"), icon: Some(IconName::Search), on_click: move |_| {
                    state.open_search_with_text(None);
                    close();
                } }
                HeaderMenuItem { label: "Find Next", shortcut: shortcut("search.next"), on_click: move |_| {
                    spawn(async move { let _ = document::eval("window.Arto.search.navigate('next')").await; });
                    close();
                } }
                HeaderMenuItem { label: "Find Previous", shortcut: shortcut("search.prev"), on_click: move |_| {
                    spawn(async move { let _ = document::eval("window.Arto.search.navigate('prev')").await; });
                    close();
                } }
            }

            // === View ===
            HeaderSubmenu { label: "View",
                HeaderMenuItem { label: "Toggle Left Sidebar", shortcut: shortcut("window.toggle_sidebar"), icon: Some(IconName::Sidebar), on_click: move |_| {
                    state.toggle_sidebar();
                    close();
                } }
                HeaderMenuItem { label: "Toggle Right Sidebar", shortcut: shortcut("window.toggle_right_sidebar"), icon: Some(IconName::List), on_click: move |_| {
                    state.toggle_right_sidebar();
                    close();
                } }
                HeaderMenuSeparator {}
                HeaderMenuItem { label: "Actual Size", shortcut: shortcut("zoom.reset"), on_click: move |_| {
                    state.zoom_level.set(1.0);
                    close();
                } }
                HeaderMenuItem { label: "Zoom In", shortcut: shortcut("zoom.in"), icon: Some(IconName::Add), on_click: move |_| {
                    let current = crate::window::settings::normalize_zoom_level(*state.zoom_level.read());
                    state.zoom_level.set(crate::window::settings::normalize_zoom_level(current + 0.1));
                    close();
                } }
                HeaderMenuItem { label: "Zoom Out", shortcut: shortcut("zoom.out"), on_click: move |_| {
                    let current = crate::window::settings::normalize_zoom_level(*state.zoom_level.read());
                    state.zoom_level.set(crate::window::settings::normalize_zoom_level(current - 0.1));
                    close();
                } }
            }

            // === History ===
            HeaderSubmenu { label: "History",
                HeaderMenuItem { label: "Go Back", shortcut: shortcut("history.back"), icon: Some(IconName::ChevronLeft), on_click: move |_| {
                    state.save_scroll_and_go_back();
                    close();
                } }
                HeaderMenuItem { label: "Go Forward", shortcut: shortcut("history.forward"), icon: Some(IconName::ChevronRight), on_click: move |_| {
                    state.save_scroll_and_go_forward();
                    close();
                } }
            }

            // === Window ===
            HeaderSubmenu { label: "Window",
                HeaderMenuItem { label: "Close All Child Windows", shortcut: shortcut("window.close_all_child_windows"), on_click: move |_| {
                    crate::window::close_child_windows_for_last_focused();
                    close();
                } }
                HeaderMenuItem { label: "Close All Windows", shortcut: shortcut("window.close_all_windows"), on_click: move |_| {
                    crate::window::close_all_main_windows();
                    close();
                } }
            }

            // === Help ===
            HeaderSubmenu { label: "Help",
                HeaderMenuItem { label: "Go to Homepage", shortcut: shortcut("app.go_to_homepage"), icon: Some(IconName::ExternalLink), on_click: move |_| {
                    let _ = open::that("https://github.com/arto-app/Arto");
                    close();
                } }
            }

            HeaderMenuSeparator {}

            // === Quit ===
            HeaderMenuItem { label: "Quit", icon: Some(IconName::Close), on_click: move |_| {
                crate::window::shutdown_all_windows();
            } }
        }
    }
}

// Component for expanding submenus
#[component]
fn HeaderSubmenu(#[props(into)] label: String, children: Element) -> Element {
    let mut show = use_signal(|| false);

    rsx! {
        div {
            class: "context-menu-item has-submenu",
            onmouseenter: move |_| show.set(true),
            onmouseleave: move |_| show.set(false),

            span { class: "context-menu-label", "{label}" }
            span { class: "submenu-arrow", "›" }

            if *show.read() {
                div {
                    class: "context-submenu",
                    {children}
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct HeaderMenuItemProps {
    #[props(into)]
    label: String,
    #[props(default)]
    icon: Option<IconName>,
    #[props(default)]
    shortcut: Option<String>,
    #[props(default = false)]
    disabled: bool,
    on_click: EventHandler<()>,
}

#[component]
fn HeaderMenuItem(props: HeaderMenuItemProps) -> Element {
    let on_click = props.on_click;
    let disabled = props.disabled;

    rsx! {
        div {
            class: if disabled { "context-menu-item disabled" } else { "context-menu-item" },
            onclick: move |_| {
                if !disabled {
                    on_click.call(());
                }
            },

            if let Some(icon) = props.icon {
                Icon {
                    name: icon,
                    size: 14,
                    class: "context-menu-icon",
                }
            }
            span { class: "context-menu-label", "{props.label}" }

            if let Some(shortcut) = props.shortcut {
                span { class: "context-menu-shortcut", "{shortcut}" }
            }
        }
    }
}

#[component]
fn HeaderMenuSeparator() -> Element {
    rsx! {
        div { class: "context-menu-separator" }
    }
}
