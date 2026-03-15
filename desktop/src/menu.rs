use dioxus_desktop::muda::accelerator::Accelerator;
use dioxus_desktop::muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use dioxus_desktop::window;
use std::cell::RefCell;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::keybindings::dispatcher::dispatch_action;
use crate::keybindings::raw_key_for_global_action;
use crate::keybindings::Action;
use crate::state::AppState;
use crate::window::{self, CreateMainWindowConfigParams};

/// Flag set by async tasks when keybindings change; cleared by the main-thread
/// event loop after updating the native menu accelerators.
static MENU_ACCEL_DIRTY: AtomicBool = AtomicBool::new(false);

// MenuItem is !Send, so we store references via thread_local on the main
// (event-loop) thread.  Both `build_menu()` and `update_menu_accelerators_if_dirty()`
// are called from the Tao event loop, guaranteeing the same thread.
thread_local! {
    static MENU_ITEMS: RefCell<Vec<(MenuId, MenuItem)>> = const { RefCell::new(Vec::new()) };
}

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
    ToggleSidebar,
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
            "view.toggle_sidebar" => Some(Self::ToggleSidebar),
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
            Self::ToggleSidebar => "view.toggle_sidebar",
            Self::ActualSize => "view.actual_size",
            Self::ZoomIn => "view.zoom_in",
            Self::ZoomOut => "view.zoom_out",
            Self::GoBack => "history.back",
            Self::GoForward => "history.forward",
            Self::GoToHomepage => "help.homepage",
        }
    }
}

/// Helper to create a menu item with a native accelerator for shortcut display.
///
/// Keyboard shortcuts are primarily handled by the keybinding engine, but we also
/// set native accelerators so macOS renders them correctly (right-aligned, dimmed).
/// Each created item is registered in MENU_ITEMS for later accelerator updates.
fn create_menu_item(id: MenuId, label: &str) -> MenuItem {
    let accel = accelerator_for_menu(id);
    let item = MenuItem::with_id(id.as_str(), label, true, accel);
    MENU_ITEMS.with(|items| items.borrow_mut().push((id, item.clone())));
    item
}

/// Convert a menu item's keybinding to a native muda Accelerator.
///
/// Returns None for items that:
/// - Have no keybinding
/// - Use multi-chord sequences (not supported by native menus)
fn accelerator_for_menu(id: MenuId) -> Option<Accelerator> {
    let action = menu_action_for_id(id)?;
    let key = raw_key_for_global_action(action)?;

    // Multi-chord sequences can't be represented as native accelerators
    if key.contains(' ') {
        return None;
    }

    key.parse::<Accelerator>().ok()
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
        MenuId::ToggleSidebar => "window.toggle_sidebar",
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
            &create_menu_item(MenuId::About, "About Arto"),
            &PredefinedMenuItem::separator(),
            &create_menu_item(MenuId::Preferences, "Preferences..."),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(Some("Quit")),
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
            &create_menu_item(MenuId::RevealInFinder, "Reveal in Finder"),
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
            &create_menu_item(MenuId::ToggleSidebar, "Toggle Sidebar"),
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

/// Mark menu accelerators as stale.  Safe to call from any thread.
///
/// The actual update happens on the next event-loop cycle via
/// `update_menu_accelerators_if_dirty`, which runs on the main thread.
pub fn mark_menu_accelerators_dirty() {
    MENU_ACCEL_DIRTY.store(true, Ordering::Relaxed);
}

/// Check the dirty flag and, if set, refresh every menu item's accelerator
/// from the current CONFIG.
///
/// **Must be called on the main (event-loop) thread** – the same thread that
/// called `build_menu()` – so the thread-local `MENU_ITEMS` is accessible.
pub fn update_menu_accelerators_if_dirty() {
    if MENU_ACCEL_DIRTY
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    MENU_ITEMS.with(|items| {
        for (id, item) in items.borrow().iter() {
            let accel = accelerator_for_menu(*id);
            if let Err(e) = item.set_accelerator(accel) {
                tracing::warn!("Failed to update accelerator for {:?}: {}", id, e);
            }
        }
    });
    tracing::debug!("Menu accelerators updated after config change");
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
/// Delegates to the keybinding dispatcher to ensure consistent behavior
/// between menu accelerators and keyboard shortcuts.
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

    let Some(action_str) = menu_action_for_id(id) else {
        return false;
    };
    let Ok(action) = Action::from_str(action_str) else {
        return false;
    };

    dispatch_action(&action, *state);
    true
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
            "view.toggle_sidebar",
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
