use crate::components::context_menu::{ContextMenuItem, ContextMenuSeparator, ContextMenuSubmenu};
use crate::components::icon::IconName;
use crate::state::AppState;
use crate::utils::task::spawn_detached;
use dioxus::prelude::*;
use rust_i18n::t;

#[component]
pub fn AppMenu(on_close: EventHandler<()>) -> Element {
    let mut state = use_context::<AppState>();

    // Helper to get keyboard shortcut hints
    let shortcut = |action| crate::keybindings::shortcut_hint_for_global_action(action);

    // Get information on the currently open file (for invalidation determination)
    let current_file = state.current_file();
    let has_file = current_file.is_some();
    // A path is not a document: one that failed to load has the first and not
    // the second, and focus mode needs something to read.
    let reading = !state.document().is_empty();

    let history = state.document().history;
    let can_go_back = history.can_go_back();
    let can_go_forward = history.can_go_forward();

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
            style: "position: absolute; left: 12px; top: var(--header-height); z-index: 999;",
            onclick: move |evt| evt.stop_propagation(),

            // === Arto (App) ===
            ContextMenuItem { label: t!("app_menu.about").to_string(), shortcut: shortcut("app.about"), icon: Some(IconName::InfoCircle), on_click: move |_| {
                crate::components::content::set_preferences_tab_to_about();
                state.open_preferences();
                close();
            } }
            ContextMenuItem { label: t!("app_menu.preferences").to_string(), shortcut: shortcut("file.preferences"), icon: Some(IconName::Gear), on_click: move |_| {
                state.open_preferences();
                close();
            } }

            ContextMenuSeparator {}

            // === File ===
            ContextMenuSubmenu { label: t!("app_menu.file.title").to_string(), icon: Some(IconName::File),
                ContextMenuItem { label: t!("app_menu.file.new_window").to_string(), shortcut: shortcut("window.new"), icon: Some(IconName::AppWindow), on_click: move |_| {
                    crate::window::create_main_window_sync(&dioxus::desktop::window(), crate::state::Document::default(), crate::window::CreateMainWindowConfigParams::default());
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.file.duplicate_window").to_string(), shortcut: shortcut("window.duplicate"), icon: Some(IconName::CopyPlus), on_click: move |_| {
                    crate::keybindings::dispatcher::dispatch_action(&crate::keybindings::Action::WindowDuplicate, state);
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.file.new_document").to_string(), shortcut: shortcut("window.new_document"), icon: Some(IconName::Add), on_click: move |_| {
                    state.update_document(|document| *document = crate::state::Document::default());
                    close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: t!("app_menu.file.open_file").to_string(), shortcut: shortcut("file.open"), icon: Some(IconName::File), on_click: move |_| {
                    if let Some(file) = rfd::FileDialog::new().add_filter("Markdown", &["md", "markdown"]).pick_file() {
                        state.open_file(file);
                    }
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.file.open_directory").to_string(), shortcut: shortcut("file.open_directory"), icon: Some(IconName::FolderOpen), on_click: move |_| {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.add_root(dir);
                    }
                    close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: t!("app_menu.file.copy_file_path").to_string(), shortcut: shortcut("clipboard.copy_file_path"), icon: Some(IconName::Copy), disabled: !has_file, on_click: { let f = current_file.clone(); move |_| {
                    if let Some(file) = &f { crate::utils::clipboard::copy_text(file.to_string_lossy()); }
                    close();
                } } }
                ContextMenuItem { label: t!("app_menu.file.reveal_in_finder").to_string(), shortcut: shortcut("file.reveal_in_finder"), icon: Some(IconName::Folder), disabled: !has_file, on_click: { let f = current_file.clone(); move |_| {
                    if let Some(file) = &f { crate::utils::file_operations::reveal_in_finder(file); }
                    close();
                } } }
                ContextMenuSeparator {}
                ContextMenuItem { label: t!("app_menu.file.close_window").to_string(), shortcut: shortcut("window.close"), icon: Some(IconName::Close), on_click: move |_| {
                    dioxus::desktop::window().close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: t!("app_menu.file.print").to_string(), shortcut: shortcut("file.print"), icon: Some(IconName::Printer), on_click: { let f = current_file.clone(); move |_| {
                    close();
                    crate::utils::print::print_window(f.clone());
                } } }
            }

            // === Edit ===
            ContextMenuSubmenu { label: t!("app_menu.edit.title").to_string(), icon: Some(IconName::Edit),
                ContextMenuItem { label: t!("app_menu.edit.find").to_string(), shortcut: shortcut("search.open"), icon: Some(IconName::Search), on_click: move |_| {
                    state.open_search_with_text(None);
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.edit.find_next").to_string(), shortcut: shortcut("search.next"), icon: Some(IconName::ChevronDown), on_click: move |_| {
                    spawn_detached(async move { let _ = document::eval("window.Arto.search.navigate('next')").await; });
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.edit.find_previous").to_string(), shortcut: shortcut("search.prev"), icon: Some(IconName::ChevronUp), on_click: move |_| {
                    spawn_detached(async move { let _ = document::eval("window.Arto.search.navigate('prev')").await; });
                    close();
                } }
            }

            // === View ===
            ContextMenuSubmenu { label: t!("app_menu.view.title").to_string(), icon: Some(IconName::Eye),
                ContextMenuItem { label: t!("app_menu.view.toggle_left_sidebar").to_string(), shortcut: shortcut("window.toggle_sidebar"), icon: Some(IconName::Sidebar), on_click: move |_| {
                    state.toggle_sidebar();
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.view.focus_mode").to_string(), shortcut: shortcut("window.toggle_focus_mode"), icon: Some(IconName::Focus2), disabled: !reading, on_click: move |_| {
                    state.toggle_focus_mode();
                    close();
                } }
                ContextMenuSeparator {}
                ContextMenuItem { label: t!("app_menu.view.actual_size").to_string(), shortcut: shortcut("zoom.reset"), icon: Some(IconName::ZoomReset), on_click: move |_| {
                    state.zoom_reset();
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.view.zoom_in").to_string(), shortcut: shortcut("zoom.in"), icon: Some(IconName::ZoomIn), on_click: move |_| {
                    state.zoom_in();
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.view.zoom_out").to_string(), shortcut: shortcut("zoom.out"), icon: Some(IconName::ZoomOut), on_click: move |_| {
                    state.zoom_out();
                    close();
                } }
            }

            // === History ===
            // Only where there is a history to move through: the header no
            // longer carries back and forward, so this is where they are, and
            // an item that cannot act reads as the feature being broken
            // rather than as the reader being at the start.
            if can_go_back || can_go_forward {
                ContextMenuSubmenu { label: t!("app_menu.history.title").to_string(), icon: Some(IconName::History),
                    if can_go_back {
                        ContextMenuItem { label: t!("app_menu.history.go_back").to_string(), shortcut: shortcut("history.back"), icon: Some(IconName::ChevronLeft), on_click: move |_| {
                            state.save_scroll_and_go_back();
                            close();
                        } }
                    }
                    if can_go_forward {
                        ContextMenuItem { label: t!("app_menu.history.go_forward").to_string(), shortcut: shortcut("history.forward"), icon: Some(IconName::ChevronRight), on_click: move |_| {
                            state.save_scroll_and_go_forward();
                            close();
                        } }
                    }
                }
            }

            // === Window ===
            ContextMenuSubmenu { label: t!("app_menu.window.title").to_string(), icon: Some(IconName::AppWindow),
                ContextMenuItem { label: t!("app_menu.window.close_all_child_windows").to_string(), shortcut: shortcut("window.close_all_child_windows"), icon: Some(IconName::Close), on_click: move |_| {
                    crate::window::close_child_windows_for_last_focused();
                    close();
                } }
                ContextMenuItem { label: t!("app_menu.window.close_all_windows").to_string(), shortcut: shortcut("window.close_all_windows"), icon: Some(IconName::Close), on_click: move |_| {
                    crate::window::close_all_main_windows();
                    close();
                } }
            }

            // === Help ===
            ContextMenuSubmenu { label: t!("app_menu.help.title").to_string(), icon: Some(IconName::HelpCircle),
                ContextMenuItem { label: t!("app_menu.help.go_to_homepage").to_string(), shortcut: shortcut("app.go_to_homepage"), icon: Some(IconName::ExternalLink), on_click: move |_| {
                    let _ = open::that("https://github.com/arto-app/Arto");
                    close();
                } }
            }

            ContextMenuSeparator {}

            // === Quit ===
            ContextMenuItem { label: t!("app_menu.quit").to_string(), icon: Some(IconName::Power), on_click: move |_| {
                crate::window::shutdown_all_windows();
            } }
        }
    }
}
