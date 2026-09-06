#![cfg(target_os = "windows")]

use crate::components::context_menu::{ContextMenuItem, ContextMenuSeparator, ContextMenuSubmenu};
use crate::components::icon::IconName;
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
            ContextMenuItem { label: "About Arto", shortcut: shortcut("app.about"), on_click: move |_| {
                crate::components::content::set_preferences_tab_to_about();
                state.open_preferences();
                close();
            } }
            ContextMenuItem { label: "Preferences...", shortcut: shortcut("file.preferences"), icon: Some(IconName::Gear), on_click: move |_| {
                state.open_preferences();
                close();
            } }

            ContextMenuSeparator {}

            // === File ===
            ContextMenuSubmenu { label: "File",
                ContextMenuItem { label: "New Window", shortcut: shortcut("window.new"), on_click: move |_| {
                    crate::window::create_main_window_sync(&dioxus::desktop::window(), crate::state::Tab::default(), crate::window::CreateMainWindowConfigParams::default());
                    close();
                } }
                ContextMenuItem { label: "New Tab", shortcut: shortcut("tab.new"), icon: Some(IconName::Add), on_click: move |_| {
                    state.add_empty_tab(true);
                    close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: "Open File...", shortcut: shortcut("file.open"), icon: Some(IconName::File), on_click: move |_| {
                    if let Some(file) = rfd::FileDialog::new().add_filter("Markdown", &["md", "markdown"]).pick_file() {
                        state.open_file(file);
                    }
                    close();
                } }
                ContextMenuItem { label: "Open Directory...", shortcut: shortcut("file.open_directory"), icon: Some(IconName::FolderOpen), on_click: move |_| {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.set_root_directory(dir);
                    }
                    close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: "Copy File Path", shortcut: shortcut("clipboard.copy_file_path"), icon: Some(IconName::Copy), disabled: !has_file, on_click: { let f = current_file.clone(); move |_| {
                    if let Some(file) = &f { crate::utils::clipboard::copy_text(file.to_string_lossy()); }
                    close();
                } } }
                ContextMenuItem { label: "Reveal in Finder", shortcut: shortcut("file.reveal_in_finder"), icon: Some(IconName::Folder), disabled: !has_file, on_click: { let f = current_file.clone(); move |_| {
                    if let Some(file) = &f { crate::utils::file_operations::reveal_in_finder(file); }
                    close();
                } } }
                ContextMenuSeparator {}
                ContextMenuItem { label: "Close Tab", shortcut: shortcut("tab.close"), on_click: move |_| {
                    let active = *state.active_tab.read();
                    state.close_tab(active);
                    close();
                } }
                ContextMenuItem { label: "Close All Tabs", shortcut: shortcut("tab.close_all"), on_click: move |_| {
                    let mut tabs = state.tabs.write();
                    tabs.clear();
                    tabs.push(crate::state::Tab::default());
                    state.active_tab.set(0);
                    close();
                } }
                ContextMenuItem { label: "Close Window", shortcut: shortcut("window.close"), on_click: move |_| {
                    dioxus::desktop::window().close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: "Print...", shortcut: shortcut("file.print"), on_click: { let f = current_file.clone(); move |_| {
                    close();
                    crate::utils::print::print_window(f.clone());
                } } }
            }

            // === Edit ===
            ContextMenuSubmenu { label: "Edit",
                ContextMenuItem { label: "Find...", shortcut: shortcut("search.open"), icon: Some(IconName::Search), on_click: move |_| {
                    state.open_search_with_text(None);
                    close();
                } }
                ContextMenuItem { label: "Find Next", shortcut: shortcut("search.next"), on_click: move |_| {
                    spawn(async move { let _ = document::eval("window.Arto.search.navigate('next')").await; });
                    close();
                } }
                ContextMenuItem { label: "Find Previous", shortcut: shortcut("search.prev"), on_click: move |_| {
                    spawn(async move { let _ = document::eval("window.Arto.search.navigate('prev')").await; });
                    close();
                } }
            }

            // === View ===
            ContextMenuSubmenu { label: "View",
                ContextMenuItem { label: "Toggle Left Sidebar", shortcut: shortcut("window.toggle_sidebar"), icon: Some(IconName::Sidebar), on_click: move |_| {
                    state.toggle_sidebar();
                    close();
                } }
                ContextMenuItem { label: "Toggle Right Sidebar", shortcut: shortcut("window.toggle_right_sidebar"), icon: Some(IconName::List), on_click: move |_| {
                    state.toggle_right_sidebar();
                    close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: "Actual Size", shortcut: shortcut("zoom.reset"), on_click: move |_| {
                    state.zoom_reset();
                    close();
                } }
                ContextMenuItem { label: "Zoom In", shortcut: shortcut("zoom.in"), icon: Some(IconName::Add), on_click: move |_| {
                    state.zoom_in();
                    close();
                } }
                ContextMenuItem { label: "Zoom Out", shortcut: shortcut("zoom.out"), on_click: move |_| {
                    state.zoom_out();
                    close();
                } }
            }

            // === History ===
            ContextMenuSubmenu { label: "History",
                ContextMenuItem { label: "Go Back", shortcut: shortcut("history.back"), icon: Some(IconName::ChevronLeft), on_click: move |_| {
                    state.save_scroll_and_go_back();
                    close();
                } }
                ContextMenuItem { label: "Go Forward", shortcut: shortcut("history.forward"), icon: Some(IconName::ChevronRight), on_click: move |_| {
                    state.save_scroll_and_go_forward();
                    close();
                } }
            }

            // === Window ===
            ContextMenuSubmenu { label: "Window",
                ContextMenuItem { label: "Close All Child Windows", shortcut: shortcut("window.close_all_child_windows"), on_click: move |_| {
                    crate::window::close_child_windows_for_last_focused();
                    close();
                } }
                ContextMenuItem { label: "Close All Windows", shortcut: shortcut("window.close_all_windows"), on_click: move |_| {
                    crate::window::close_all_main_windows();
                    close();
                } }
            }

            // === Help ===
            ContextMenuSubmenu { label: "Help",
                ContextMenuItem { label: "Go to Homepage", shortcut: shortcut("app.go_to_homepage"), icon: Some(IconName::ExternalLink), on_click: move |_| {
                    let _ = open::that("https://github.com/arto-app/Arto");
                    close();
                } }
            }

            ContextMenuSeparator {}

            // === Quit ===
            ContextMenuItem { label: "Quit", icon: Some(IconName::Close), on_click: move |_| {
                crate::window::shutdown_all_windows();
            } }
        }
    }
}
