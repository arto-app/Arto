use dioxus::document;
use dioxus::prelude::{spawn, ReadableExt, WritableExt};
use dioxus_desktop::muda::accelerator::Accelerator;
use dioxus_desktop::muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use dioxus_desktop::window;
use std::path::PathBuf;

use crate::components::content::set_preferences_tab_to_about;
use crate::keybindings::shortcut_hint_for_global_action;
use crate::state::AppState;
use crate::window::{self, settings::normalize_zoom_level, CreateMainWindowConfigParams};

/// Menu identifier enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuId {
    About,
    NewWindow,
    NewTab,
    Open,
    OpenDirectory,
    RevealInFinder,
    CopyFilePath,
    CloseTab,
    CloseAllTabs,
    CloseWindow,
    CloseAllChildWindows,
    CloseAllWindows,
    Preferences,
    Find,
    FindNext,
    FindPrevious,
    ToggleLeftSidebar,
    ToggleRightSidebar,
    ActualSize,
    ZoomIn,
    ZoomOut,
    GoBack,
    GoForward,
    GoToHomepage,
}

impl MenuId {
    /// Convert menu ID string to enum variant
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "app.about" => Some(Self::About),
            "file.new_window" => Some(Self::NewWindow),
            "file.new_tab" => Some(Self::NewTab),
            "file.open" => Some(Self::Open),
            "file.open_directory" => Some(Self::OpenDirectory),
            "file.reveal_in_finder" => Some(Self::RevealInFinder),
            "file.copy_file_path" => Some(Self::CopyFilePath),
            "file.close_tab" => Some(Self::CloseTab),
            "file.close_all_tabs" => Some(Self::CloseAllTabs),
            "file.close_window" => Some(Self::CloseWindow),
            "window.close_all_child_windows" => Some(Self::CloseAllChildWindows),
            "window.close_all_windows" => Some(Self::CloseAllWindows),
            "app.preferences" => Some(Self::Preferences),
            "edit.find" => Some(Self::Find),
            "edit.find_next" => Some(Self::FindNext),
            "edit.find_previous" => Some(Self::FindPrevious),
            "view.toggle_left_sidebar" => Some(Self::ToggleLeftSidebar),
            "view.toggle_right_sidebar" => Some(Self::ToggleRightSidebar),
            "view.actual_size" => Some(Self::ActualSize),
            "view.zoom_in" => Some(Self::ZoomIn),
            "view.zoom_out" => Some(Self::ZoomOut),
            "history.back" => Some(Self::GoBack),
            "history.forward" => Some(Self::GoForward),
            "help.homepage" => Some(Self::GoToHomepage),
            _ => None,
        }
    }

    /// Get the string ID for this menu item
    fn as_str(self) -> &'static str {
        match self {
            Self::About => "app.about",
            Self::NewWindow => "file.new_window",
            Self::NewTab => "file.new_tab",
            Self::Open => "file.open",
            Self::OpenDirectory => "file.open_directory",
            Self::RevealInFinder => "file.reveal_in_finder",
            Self::CopyFilePath => "file.copy_file_path",
            Self::CloseTab => "file.close_tab",
            Self::CloseAllTabs => "file.close_all_tabs",
            Self::CloseWindow => "file.close_window",
            Self::CloseAllChildWindows => "window.close_all_child_windows",
            Self::CloseAllWindows => "window.close_all_windows",
            Self::Preferences => "app.preferences",
            Self::Find => "edit.find",
            Self::FindNext => "edit.find_next",
            Self::FindPrevious => "edit.find_previous",
            Self::ToggleLeftSidebar => "view.toggle_left_sidebar",
            Self::ToggleRightSidebar => "view.toggle_right_sidebar",
            Self::ActualSize => "view.actual_size",
            Self::ZoomIn => "view.zoom_in",
            Self::ZoomOut => "view.zoom_out",
            Self::GoBack => "history.back",
            Self::GoForward => "history.forward",
            Self::GoToHomepage => "help.homepage",
        }
    }
}

/// Helper to create a menu item without an accelerator.
///
/// All keyboard shortcuts are handled by the keybinding engine, not muda.
fn create_menu_item(id: MenuId, label: &str) -> MenuItem {
    let display_label = menu_label_with_shortcut(id, label);
    MenuItem::with_id(id.as_str(), &display_label, true, None::<Accelerator>)
}

/// Build a menu label with a right-aligned shortcut hint (without muda Accelerator).
///
/// We intentionally avoid `with_accelerator()` because keyboard handling is owned by
/// the keybinding engine. This only mirrors the hint in the menu UI.
fn menu_label_with_shortcut(id: MenuId, base_label: &str) -> String {
    let Some(action) = menu_action_for_id(id) else {
        return base_label.to_string();
    };
    let Some(shortcut) = shortcut_hint_for_global_action(action) else {
        return base_label.to_string();
    };
    format!("{base_label}\t{shortcut}")
}

/// Map menu items to keybinding actions for hint display.
fn menu_action_for_id(id: MenuId) -> Option<&'static str> {
    Some(match id {
        MenuId::About => "app.about",
        MenuId::NewWindow => "window.new",
        MenuId::NewTab => "tab.new",
        MenuId::Open => "file.open",
        MenuId::OpenDirectory => "file.open_directory",
        MenuId::RevealInFinder => "file.reveal_in_finder",
        MenuId::CopyFilePath => "clipboard.copy_file_path",
        MenuId::CloseTab => "tab.close",
        MenuId::CloseAllTabs => "tab.close_all",
        MenuId::CloseWindow => "window.close",
        MenuId::CloseAllChildWindows => "window.close_all_child_windows",
        MenuId::CloseAllWindows => "window.close_all_windows",
        MenuId::Preferences => "file.preferences",
        MenuId::Find => "search.open",
        MenuId::FindNext => "search.next",
        MenuId::FindPrevious => "search.prev",
        MenuId::ToggleLeftSidebar => "window.toggle_sidebar",
        MenuId::ToggleRightSidebar => "window.toggle_right_sidebar",
        MenuId::ActualSize => "zoom.reset",
        MenuId::ZoomIn => "zoom.in",
        MenuId::ZoomOut => "zoom.out",
        MenuId::GoBack => "history.back",
        MenuId::GoForward => "history.forward",
        MenuId::GoToHomepage => "app.go_to_homepage",
    })
}

/// Build the application menu bar
pub fn build_menu() -> Menu {
    #[cfg(target_os = "macos")]
    disable_automatic_window_tabbing();

    let menu = Menu::new();

    add_app_menu(&menu);
    add_file_menu(&menu);
    add_edit_menu(&menu);
    add_view_menu(&menu);
    add_history_menu(&menu);
    add_window_menu(&menu);
    add_help_menu(&menu);

    menu
}

fn add_app_menu(menu: &Menu) {
    let arto_menu = Submenu::new("Arto", true);

    arto_menu
        .append_items(&[
            &create_menu_item(MenuId::About, if cfg!(windows) { "About" } else { "About Arto" }),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::Preferences, if cfg!(windows) { "Settings" } else { "Preferences..." }),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(Some(if cfg!(windows) { "Exit" } else { "Quit" })),
        ])
        .unwrap();

    menu.append(&arto_menu).unwrap();
}

fn add_file_menu(menu: &Menu) {
    let file_menu = Submenu::new("File", true);

    file_menu
        .append_items(&[
            &create_menu_item(MenuId::NewWindow, "New Window"),
            &create_menu_item(MenuId::NewTab, "New Tab"),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::Open, "Open File..."),
            &create_menu_item(MenuId::OpenDirectory, "Open Directory..."),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::CopyFilePath, "Copy File Path"),
            &create_menu_item(MenuId::RevealInFinder, if cfg!(windows) { "Reveal in File Explorer" } else { "Reveal in Finder" }),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::CloseTab, "Close Tab"),
            &create_menu_item(MenuId::CloseAllTabs, "Close All Tabs"),
            &create_menu_item(MenuId::CloseWindow, "Close Window"),
        ])
        .unwrap();

    menu.append(&file_menu).unwrap();
}

fn add_edit_menu(menu: &Menu) {
    let edit_menu = Submenu::new("Edit", true);

    edit_menu
        .append_items(&[
            &PredefinedMenuItem::cut(Some("Cut")),
            &PredefinedMenuItem::copy(Some("Copy")),
            &PredefinedMenuItem::paste(Some("Paste")),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::select_all(Some("Select All")),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::Find, "Find..."),
            &create_menu_item(MenuId::FindNext, "Find Next"),
            &create_menu_item(MenuId::FindPrevious, "Find Previous"),
        ])
        .unwrap();

    menu.append(&edit_menu).unwrap();
}

fn add_view_menu(menu: &Menu) {
    let view_menu = Submenu::new("View", true);

    view_menu
        .append_items(&[
            &create_menu_item(MenuId::ToggleLeftSidebar, "Toggle Left Sidebar"),
            &create_menu_item(MenuId::ToggleRightSidebar, "Toggle Right Sidebar"),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::ActualSize, "Actual Size"),
            &create_menu_item(MenuId::ZoomIn, "Zoom In"),
            &create_menu_item(MenuId::ZoomOut, "Zoom Out"),
        ])
        .unwrap();

    menu.append(&view_menu).unwrap();
}

fn add_history_menu(menu: &Menu) {
    let history_menu = Submenu::new("History", true);

    history_menu
        .append_items(&[
            &create_menu_item(MenuId::GoBack, "Go Back"),
            &create_menu_item(MenuId::GoForward, "Go Forward"),
        ])
        .unwrap();

    menu.append(&history_menu).unwrap();
}

fn add_window_menu(menu: &Menu) {
    let window_menu = Submenu::new("Window", true);

    window_menu
        .append_items(&[
            &create_menu_item(MenuId::CloseAllChildWindows, "Close All Child Windows"),
            &create_menu_item(MenuId::CloseAllWindows, "Close All Windows"),
        ])
        .unwrap();

    menu.append(&window_menu).unwrap();
}

fn add_help_menu(menu: &Menu) {
    let help_menu = Submenu::new("Help", true);

    help_menu
        .append(&create_menu_item(MenuId::GoToHomepage, "Go to Homepage"))
        .unwrap();

    menu.append(&help_menu).unwrap();
}

/// Check if a menu event is a close action (Close Tab or Close Window)
pub fn is_close_action(event: &MenuEvent) -> bool {
    matches!(
        MenuId::from_str(event.id().0.as_ref()),
        Some(MenuId::CloseTab | MenuId::CloseWindow)
    )
}

/// Handle menu events that do NOT require per-window AppState.
///
/// # Handled events
/// - `NewWindow`: Creates a new window (no state needed)
/// - `NewTab` (no windows exist): Creates a window as fallback
/// - `Preferences`: Declined here (requires per-window state), returns `false`
/// - `CloseAllChildWindows`: Uses window manager API
/// - `CloseAllWindows`: Uses window manager API
/// - `GoToHomepage`: Opens URL in external browser
///
/// # Returns
/// `true` if the event was fully handled, `false` if it needs state-dependent handling.
pub fn handle_menu_event_global(event: &MenuEvent) -> bool {
    let menu_id = event.id().0.as_ref();

    let id = match MenuId::from_str(menu_id) {
        Some(id) => id,
        None => return false,
    };

    match id {
        MenuId::NewWindow => {
            window::create_main_window_sync(
                &window(),
                crate::state::Tab::default(),
                CreateMainWindowConfigParams::default(),
            );
        }
        MenuId::NewTab => {
            if !window::has_any_main_windows() {
                window::create_main_window_sync(
                    &window(),
                    crate::state::Tab::default(),
                    CreateMainWindowConfigParams::default(),
                );
                return true;
            }
            return false;
        }
        MenuId::Preferences => {
            // Declined here; requires per-window AppState access
            return false;
        }
        MenuId::CloseAllChildWindows => {
            window::close_child_windows_for_last_focused();
        }
        MenuId::CloseAllWindows => {
            window::close_all_main_windows();
        }
        MenuId::GoToHomepage => {
            let _ = open::that("https://github.com/arto-app/Arto");
        }
        _ => return false,
    }

    true
}

/// Handle menu events that require per-window AppState.
///
/// Only processes events for the currently focused window.
///
/// # Handled events
/// - `About`: Opens preferences page on About tab
/// - `Preferences`: Opens preferences page
/// - `NewTab`: Adds an empty tab to current window
/// - `Open`: Opens file picker for markdown files
/// - `OpenDirectory`: Opens directory picker
/// - `CloseTab` / `CloseAllTabs` / `CloseWindow`: Tab/window management
/// - `ToggleLeftSidebar`: Toggles left sidebar pin state
/// - `ToggleRightSidebar`: Toggles right sidebar pin state
/// - `ActualSize` / `ZoomIn` / `ZoomOut`: Zoom controls
/// - `GoBack` / `GoForward`: Navigation history
/// - `RevealInFinder` / `CopyFilePath`: File operations
/// - `Find` / `FindNext` / `FindPrevious`: Search operations
///
/// # Returns
/// `true` if the event was handled, `false` otherwise.
pub fn handle_menu_event_with_state(event: &MenuEvent, state: &mut AppState) -> bool {
    // Check if current window is focused
    if !window().is_focused() {
        return false;
    }

    let menu_id = event.id().0.as_ref();
    tracing::debug!("State menu event (focused window): {}", menu_id);

    let id = match MenuId::from_str(menu_id) {
        Some(id) => id,
        None => return false,
    };

    match id {
        MenuId::About => {
            // Set the preferences tab to About before opening
            set_preferences_tab_to_about();
            state.open_preferences();
        }
        MenuId::Preferences => {
            state.open_preferences();
        }
        MenuId::NewTab => {
            state.add_empty_tab(true);
        }
        MenuId::Open => {
            if let Some(file) = pick_markdown_file() {
                state.open_file(file);
            }
        }
        MenuId::OpenDirectory => {
            if let Some(dir) = pick_directory() {
                state.set_root_directory(dir);
            }
        }
        MenuId::CloseTab => {
            let active_tab = *state.active_tab.read();
            state.close_tab(active_tab);
        }
        MenuId::CloseAllTabs => {
            // Close all tabs except one, then clear it
            let mut tabs = state.tabs.write();
            tabs.clear();
            tabs.push(crate::state::Tab::default());
            state.active_tab.set(0);
        }
        MenuId::CloseWindow => {
            window().close();
        }
        MenuId::ToggleLeftSidebar => {
            state.toggle_sidebar();
        }
        MenuId::ToggleRightSidebar => {
            state.toggle_right_sidebar();
        }
        MenuId::ActualSize => {
            state.zoom_level.set(1.0);
        }
        MenuId::ZoomIn => {
            let current = normalize_zoom_level(*state.zoom_level.read());
            let next = normalize_zoom_level(current + 0.1);
            state.zoom_level.set(next);
        }
        MenuId::ZoomOut => {
            let current = normalize_zoom_level(*state.zoom_level.read());
            let next = normalize_zoom_level(current - 0.1);
            state.zoom_level.set(next);
        }
        MenuId::GoBack => {
            state.save_scroll_and_go_back();
        }
        MenuId::GoForward => {
            state.save_scroll_and_go_forward();
        }
        MenuId::RevealInFinder => {
            if let Some(file) = get_current_file(state) {
                crate::utils::file_operations::reveal_in_finder(&file);
            }
        }
        MenuId::CopyFilePath => {
            if let Some(file) = get_current_file(state) {
                crate::utils::clipboard::copy_text(file.to_string_lossy());
            }
        }
        MenuId::Find => {
            // None = get selected text from JavaScript
            state.open_search_with_text(None);
        }
        MenuId::FindNext => {
            spawn(async move {
                let _ = document::eval("window.Arto.search.navigate('next')").await;
            });
        }
        MenuId::FindPrevious => {
            spawn(async move {
                let _ = document::eval("window.Arto.search.navigate('prev')").await;
            });
        }
        _ => return false,
    }

    true
}

/// Get the current file path from state if viewing a file
fn get_current_file(state: &AppState) -> Option<PathBuf> {
    let tabs = state.tabs.read();
    let active_tab = *state.active_tab.read();
    tabs.get(active_tab).and_then(|tab| {
        if let crate::state::TabContent::File(path) = &tab.content {
            Some(path.clone())
        } else {
            None
        }
    })
}

/// Show file picker dialog and return selected file
fn pick_markdown_file() -> Option<PathBuf> {
    use rfd::FileDialog;

    tracing::debug!("Opening file picker dialog...");
    let start = std::time::Instant::now();

    let file = FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_directory(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
        .pick_file();

    tracing::debug!("File picker completed in {:?}", start.elapsed());

    file
}

/// Show directory picker dialog and return selected directory
fn pick_directory() -> Option<PathBuf> {
    use rfd::FileDialog;

    tracing::debug!("Opening directory picker dialog...");
    let start = std::time::Instant::now();

    let dir = FileDialog::new()
        .set_directory(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
        .pick_folder();

    tracing::debug!("Directory picker completed in {:?}", start.elapsed());

    dir
}

#[cfg(target_os = "macos")]
fn disable_automatic_window_tabbing() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindow;
    let marker = MainThreadMarker::new().expect("Failed to get main thread marker");
    NSWindow::setAllowsAutomaticWindowTabbing(false, marker);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All MenuId variants must roundtrip through as_str/from_str.
    /// This guarantees safety for Phase 3-5 handler refactoring.
    #[test]
    fn test_menu_id_roundtrip() {
        let all_ids = [
            "app.about",
            "file.new_window",
            "file.new_tab",
            "file.open",
            "file.open_directory",
            "file.reveal_in_finder",
            "file.copy_file_path",
            "file.close_tab",
            "file.close_all_tabs",
            "file.close_window",
            "window.close_all_child_windows",
            "window.close_all_windows",
            "app.preferences",
            "edit.find",
            "edit.find_next",
            "edit.find_previous",
            "view.toggle_left_sidebar",
            "view.toggle_right_sidebar",
            "view.actual_size",
            "view.zoom_in",
            "view.zoom_out",
            "history.back",
            "history.forward",
            "help.homepage",
        ];
        for id_str in &all_ids {
            let parsed = MenuId::from_str(id_str);
            assert!(
                parsed.is_some(),
                "MenuId::from_str({id_str}) should succeed"
            );
            assert_eq!(
                parsed.unwrap().as_str(),
                *id_str,
                "Roundtrip failed for {id_str}"
            );
        }
    }

    /// from_str returns None for unknown IDs
    #[test]
    fn test_menu_id_unknown_returns_none() {
        assert!(MenuId::from_str("unknown.action").is_none());
        assert!(MenuId::from_str("").is_none());
    }

    /// is_close_action correctly identifies close events
    #[test]
    fn test_is_close_action() {
        use dioxus_desktop::muda::MenuId as MudaMenuId;
        let close_tab = MenuEvent {
            id: MudaMenuId("file.close_tab".to_string()),
        };
        let close_window = MenuEvent {
            id: MudaMenuId("file.close_window".to_string()),
        };
        let new_tab = MenuEvent {
            id: MudaMenuId("file.new_tab".to_string()),
        };

        assert!(is_close_action(&close_tab));
        assert!(is_close_action(&close_window));
        assert!(!is_close_action(&new_tab));
    }
}
