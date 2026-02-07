use dioxus::document;
use dioxus::prelude::*;

use crate::components::content::set_preferences_tab_to_about;
use crate::components::right_sidebar::RightSidebarTab;
use crate::keybindings::action::Action;
use crate::state::{AppState, FocusedPanel};
use crate::window::{self, settings::normalize_zoom_level, CreateMainWindowConfigParams};

/// Scroll amount in pixels for scroll.up/scroll.down actions.
const SCROLL_LINE_PX: u32 = 40;

/// Dispatch an action from the keybinding engine.
///
/// This handles all Engine-dispatched actions. Menu-dispatched actions
/// (via Cmd+Key accelerators) are handled separately by menu.rs.
/// Some actions overlap (e.g., tab.close can come from both Engine and menu),
/// but the logic is shared through AppState methods.
pub fn dispatch_action(action: &Action, state: &mut AppState) {
    match action {
        // Scroll actions — executed via JS eval
        Action::ScrollDown => scroll_by(SCROLL_LINE_PX as i32),
        Action::ScrollUp => scroll_by(-(SCROLL_LINE_PX as i32)),
        Action::ScrollPageDown => scroll_page("down"),
        Action::ScrollPageUp => scroll_page("up"),
        Action::ScrollHalfPageDown => scroll_half_page("down"),
        Action::ScrollHalfPageUp => scroll_half_page("up"),
        Action::ScrollTop => scroll_to_edge("top"),
        Action::ScrollBottom => scroll_to_edge("bottom"),

        // Tab actions (context-aware: cycle right sidebar tabs when focused)
        Action::TabNext => {
            if *state.focused_panel.read() == FocusedPanel::RightSidebar {
                let next = match *state.right_sidebar_tab.read() {
                    RightSidebarTab::Contents => RightSidebarTab::Search,
                    RightSidebarTab::Search => RightSidebarTab::Contents,
                };
                state.set_right_sidebar_tab(next);
            } else {
                let active = *state.active_tab.read();
                let count = state.tabs.read().len();
                if count > 0 {
                    state.switch_to_tab((active + 1) % count);
                }
            }
        }
        Action::TabPrev => {
            if *state.focused_panel.read() == FocusedPanel::RightSidebar {
                let next = match *state.right_sidebar_tab.read() {
                    RightSidebarTab::Contents => RightSidebarTab::Search,
                    RightSidebarTab::Search => RightSidebarTab::Contents,
                };
                state.set_right_sidebar_tab(next);
            } else {
                let active = *state.active_tab.read();
                let count = state.tabs.read().len();
                if count > 0 {
                    state.switch_to_tab((active + count - 1) % count);
                }
            }
        }
        Action::TabClose => {
            let active = *state.active_tab.read();
            state.close_tab(active);
        }
        Action::TabCloseAll => {
            state.close_all_unpinned();
        }
        Action::TabNew => {
            state.add_empty_tab(true);
        }
        Action::TabUndoClose => {
            // Phase 3: undo close stack (not yet implemented)
            tracing::debug!("tab.undo_close not yet implemented");
        }
        Action::TabGo(n) => {
            let count = state.tabs.read().len();
            if count == 0 {
                return;
            }
            // 1-indexed, 9 = last tab
            let index = if *n == 9 {
                count - 1
            } else {
                (*n).saturating_sub(1).min(count - 1)
            };
            state.switch_to_tab(index);
        }

        // History actions
        Action::HistoryBack => {
            state.save_scroll_and_go_back();
        }
        Action::HistoryForward => {
            state.save_scroll_and_go_forward();
        }

        // Search actions
        Action::SearchOpen => {
            state.open_search_with_text(None);
        }
        Action::SearchNext => {
            spawn(async {
                let _ = document::eval("window.Arto.search.navigate('next')").await;
            });
        }
        Action::SearchPrev => {
            spawn(async {
                let _ = document::eval("window.Arto.search.navigate('prev')").await;
            });
        }
        Action::SearchClose => {
            if *state.search_open.read() {
                state.toggle_search();
            }
        }

        // Zoom actions
        Action::ZoomIn => {
            let current = normalize_zoom_level(*state.zoom_level.read());
            let next = normalize_zoom_level(current + 0.1);
            state.zoom_level.set(next);
        }
        Action::ZoomOut => {
            let current = normalize_zoom_level(*state.zoom_level.read());
            let next = normalize_zoom_level(current - 0.1);
            state.zoom_level.set(next);
        }
        Action::ZoomReset => {
            state.zoom_level.set(1.0);
        }

        // Clipboard actions
        Action::ClipboardCopyUrl => {
            if let Some(path) = get_current_file(state) {
                crate::utils::clipboard::copy_text(path.to_string_lossy());
            }
        }
        Action::ClipboardCopySelection => {
            // Copy current selection via JS
            spawn(async {
                let _ = document::eval(
                    "navigator.clipboard.writeText(window.getSelection()?.toString() || '')",
                )
                .await;
            });
        }

        // Window actions
        Action::WindowNew => {
            window::create_main_window_sync(
                &dioxus::desktop::window(),
                crate::state::Tab::default(),
                CreateMainWindowConfigParams::default(),
            );
        }
        Action::WindowClose => {
            dioxus::desktop::window().close();
        }
        Action::WindowCloseAllChildren => {
            window::close_child_windows_for_last_focused();
        }
        Action::WindowCloseAll => {
            window::close_all_main_windows();
        }
        Action::WindowToggleSidebar => {
            let was_open = state.sidebar.read().open;
            state.toggle_sidebar();
            if !was_open {
                // Opening sidebar: auto-focus it
                state.focused_panel.set(FocusedPanel::LeftSidebar);
                // Initialize cursor to current active file if not set
                if state.sidebar_cursor.read().is_none() {
                    let cursor = get_current_file(state);
                    state.sidebar_cursor.set(cursor);
                }
            } else {
                // Closing sidebar: return focus to content
                state.focused_panel.set(FocusedPanel::Content);
            }
        }
        Action::WindowToggleRightSidebar => {
            let was_open = *state.right_sidebar_open.read();
            state.toggle_right_sidebar();
            if !was_open {
                // Opening right sidebar: auto-focus it
                state.focused_panel.set(FocusedPanel::RightSidebar);
                // Initialize cursor to first heading if not set
                if state.toc_cursor.read().is_none() && !state.toc_headings.read().is_empty() {
                    state.toc_cursor.set(Some(0));
                }
            } else {
                // Closing right sidebar: return focus to content
                state.focused_panel.set(FocusedPanel::Content);
            }
        }

        // Focus actions
        Action::FocusLeftSidebar => {
            if state.sidebar.read().open {
                state.focused_panel.set(FocusedPanel::LeftSidebar);
                // Initialize cursor if not set
                if state.sidebar_cursor.read().is_none() {
                    let cursor = get_current_file(state);
                    state.sidebar_cursor.set(cursor);
                }
            }
        }
        Action::FocusRightSidebar => {
            if *state.right_sidebar_open.read() {
                state.focused_panel.set(FocusedPanel::RightSidebar);
                // Initialize cursor if not set
                if state.toc_cursor.read().is_none() && !state.toc_headings.read().is_empty() {
                    state.toc_cursor.set(Some(0));
                }
            }
        }
        Action::FocusQuickAccess => {
            // Open sidebar if closed
            if !state.sidebar.read().open {
                state.toggle_sidebar();
            }
            state.focused_panel.set(FocusedPanel::QuickAccess);
            // Initialize cursor if not set and bookmarks exist
            if state.quick_access_cursor.read().is_none() {
                let count = crate::bookmarks::BOOKMARKS.read().items.len();
                if count > 0 {
                    state.quick_access_cursor.set(Some(0));
                }
            }
        }
        Action::FocusContent => {
            state.focused_panel.set(FocusedPanel::Content);
        }

        // Cursor actions (panel navigation)
        Action::CursorDown => {
            let panel = *state.focused_panel.read();
            match panel {
                FocusedPanel::LeftSidebar => move_sidebar_cursor_down(state),
                FocusedPanel::RightSidebar => move_toc_cursor_down(state),
                FocusedPanel::QuickAccess => move_quick_access_cursor_down(state),
                FocusedPanel::Content => {}
            }
        }
        Action::CursorUp => {
            let panel = *state.focused_panel.read();
            match panel {
                FocusedPanel::LeftSidebar => move_sidebar_cursor_up(state),
                FocusedPanel::RightSidebar => move_toc_cursor_up(state),
                FocusedPanel::QuickAccess => move_quick_access_cursor_up(state),
                FocusedPanel::Content => {}
            }
        }
        Action::CursorEnter => {
            let panel = *state.focused_panel.read();
            match panel {
                FocusedPanel::LeftSidebar => sidebar_enter_action(state),
                FocusedPanel::RightSidebar => toc_jump_to_heading(state),
                FocusedPanel::QuickAccess => quick_access_enter_action(state),
                FocusedPanel::Content => {}
            }
        }
        Action::CursorCollapse => {
            let panel = *state.focused_panel.read();
            if panel == FocusedPanel::LeftSidebar {
                sidebar_collapse_or_parent(state);
            }
        }

        // Directory actions (sidebar navigation)
        Action::DirectoryParent => {
            let panel = *state.focused_panel.read();
            if panel == FocusedPanel::LeftSidebar {
                state.go_to_parent_directory();
                state.sidebar_cursor.set(None);
            }
        }
        Action::DirectoryBack => {
            let panel = *state.focused_panel.read();
            if panel == FocusedPanel::LeftSidebar {
                state.go_back_directory();
                state.sidebar_cursor.set(None);
            }
        }
        Action::DirectoryForward => {
            let panel = *state.focused_panel.read();
            if panel == FocusedPanel::LeftSidebar {
                state.go_forward_directory();
                state.sidebar_cursor.set(None);
            }
        }

        // File actions
        Action::FileOpen => {
            // Phase 3: file picker (requires rfd async)
            tracing::debug!("file.open via keybinding not yet implemented");
        }
        Action::FileOpenDirectory => {
            tracing::debug!("file.open_directory via keybinding not yet implemented");
        }
        Action::FileRevealInFinder => {
            if let Some(file) = get_current_file(state) {
                crate::utils::file_operations::reveal_in_finder(&file);
            }
        }
        Action::FileCopyPath => {
            if let Some(file) = get_current_file(state) {
                crate::utils::clipboard::copy_text(file.to_string_lossy());
            }
        }

        // App actions
        Action::AppAbout => {
            set_preferences_tab_to_about();
            state.open_preferences();
        }
        Action::AppPreferences => {
            state.open_preferences();
        }
        Action::AppHomepage => {
            let _ = open::that("https://github.com/arto-app/Arto");
        }

        // Cancel chain (see spec §3)
        Action::Cancel => {
            // Priority chain is handled in app.rs before calling dispatch_action
        }
    }
}

/// Get the current file path from state.
fn get_current_file(state: &AppState) -> Option<std::path::PathBuf> {
    let tabs = state.tabs.read();
    let active = *state.active_tab.read();
    tabs.get(active).and_then(|tab| {
        if let crate::state::TabContent::File(path) = &tab.content {
            Some(path.clone())
        } else {
            None
        }
    })
}

// ============================================================================
// Scroll Helpers
// ============================================================================

/// Scroll the content area by a pixel amount.
fn scroll_by(pixels: i32) {
    spawn(async move {
        let js = format!("window.Arto.scroll.scrollBy({pixels})");
        let _ = document::eval(&js).await;
    });
}

/// Scroll by one page (viewport height).
fn scroll_page(direction: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.scroll.scrollPage('{direction}')");
        let _ = document::eval(&js).await;
    });
}

/// Scroll by half a page.
fn scroll_half_page(direction: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.scroll.scrollHalfPage('{direction}')");
        let _ = document::eval(&js).await;
    });
}

/// Scroll to the top or bottom edge.
fn scroll_to_edge(edge: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.scroll.scrollToEdge('{edge}')");
        let _ = document::eval(&js).await;
    });
}

// ============================================================================
// Left Sidebar Cursor Helpers
// ============================================================================

fn move_sidebar_cursor_down(state: &mut AppState) {
    let sidebar = state.sidebar.read();
    let Some(root) = sidebar.root_directory.as_ref() else {
        return;
    };
    let visible =
        crate::state::compute_visible_items(root, &sidebar.expanded_dirs, sidebar.show_all_files);
    let current = state.sidebar_cursor.read().clone();
    drop(sidebar);

    let new_cursor = crate::state::move_cursor_down(current.as_deref(), &visible);
    state.sidebar_cursor.set(new_cursor);
    scroll_keyboard_focused_into_view(".left-sidebar-tree");
}

fn move_sidebar_cursor_up(state: &mut AppState) {
    let sidebar = state.sidebar.read();
    let Some(root) = sidebar.root_directory.as_ref() else {
        return;
    };
    let visible =
        crate::state::compute_visible_items(root, &sidebar.expanded_dirs, sidebar.show_all_files);
    let current = state.sidebar_cursor.read().clone();
    drop(sidebar);

    let new_cursor = crate::state::move_cursor_up(current.as_deref(), &visible);
    state.sidebar_cursor.set(new_cursor);
    scroll_keyboard_focused_into_view(".left-sidebar-tree");
}

fn sidebar_enter_action(state: &mut AppState) {
    let cursor = state.sidebar_cursor.read().clone();
    let Some(path) = cursor else {
        return;
    };

    if path.is_dir() {
        // Expand/toggle the directory
        state.toggle_directory_expansion(&path);
    } else {
        // Open the file and return focus to content
        state.open_file(&path);
        state.focused_panel.set(FocusedPanel::Content);
    }
}

fn sidebar_collapse_or_parent(state: &mut AppState) {
    let cursor = state.sidebar_cursor.read().clone();
    let Some(path) = cursor else {
        return;
    };

    if path.is_dir() && state.sidebar.read().expanded_dirs.contains(&path) {
        // Collapse expanded directory
        state.toggle_directory_expansion(&path);
    } else {
        // Move cursor to parent item
        let root = state.sidebar.read().root_directory.clone();
        if let Some(root) = root {
            if let Some(parent) = crate::state::find_parent_item(&path, &root) {
                state.sidebar_cursor.set(Some(parent));
            }
        }
    }
}

// ============================================================================
// Right Sidebar (TOC) Cursor Helpers
// ============================================================================

fn move_toc_cursor_down(state: &mut AppState) {
    let count = state.toc_headings.read().len();
    if count == 0 {
        return;
    }
    let current = *state.toc_cursor.read();
    let next = match current {
        None => Some(0),
        Some(idx) if idx + 1 < count => Some(idx + 1),
        Some(idx) => Some(idx), // at bottom, stay
    };
    state.toc_cursor.set(next);
    scroll_keyboard_focused_into_view(".right-sidebar-contents-list");
}

fn move_toc_cursor_up(state: &mut AppState) {
    let count = state.toc_headings.read().len();
    if count == 0 {
        return;
    }
    let current = *state.toc_cursor.read();
    let next = match current {
        None => Some(count - 1),
        Some(0) => Some(0), // at top, stay
        Some(idx) => Some(idx - 1),
    };
    state.toc_cursor.set(next);
    scroll_keyboard_focused_into_view(".right-sidebar-contents-list");
}

fn toc_jump_to_heading(state: &mut AppState) {
    let idx = *state.toc_cursor.read();
    let Some(idx) = idx else {
        return;
    };
    let headings = state.toc_headings.read();
    let Some(heading) = headings.get(idx) else {
        return;
    };
    let id = heading.id.clone();
    drop(headings);

    // Scroll to heading via JS
    spawn(async move {
        let js = format!(
            r#"
            (() => {{
                const el = document.getElementById('{}');
                if (el) el.scrollIntoView({{ behavior: 'smooth', block: 'start' }});
            }})();
            "#,
            id
        );
        let _ = document::eval(&js).await;
    });

    // Return focus to content
    state.focused_panel.set(FocusedPanel::Content);
}

// ============================================================================
// Quick Access Cursor Helpers
// ============================================================================

fn move_quick_access_cursor_down(state: &mut AppState) {
    let count = crate::bookmarks::BOOKMARKS.read().items.len();
    if count == 0 {
        return;
    }
    let current = *state.quick_access_cursor.read();
    let next = match current {
        None => Some(0),
        Some(idx) if idx + 1 < count => Some(idx + 1),
        Some(idx) => Some(idx), // at bottom, stay
    };
    state.quick_access_cursor.set(next);
    scroll_keyboard_focused_into_view(".left-sidebar-quick-access-list");
}

fn move_quick_access_cursor_up(state: &mut AppState) {
    let count = crate::bookmarks::BOOKMARKS.read().items.len();
    if count == 0 {
        return;
    }
    let current = *state.quick_access_cursor.read();
    let next = match current {
        None => Some(count - 1),
        Some(0) => Some(0), // at top, stay
        Some(idx) => Some(idx - 1),
    };
    state.quick_access_cursor.set(next);
    scroll_keyboard_focused_into_view(".left-sidebar-quick-access-list");
}

fn quick_access_enter_action(state: &mut AppState) {
    let cursor = *state.quick_access_cursor.read();
    let Some(idx) = cursor else {
        return;
    };
    let bookmarks = crate::bookmarks::BOOKMARKS.read();
    let Some(bookmark) = bookmarks.items.get(idx) else {
        return;
    };
    let path = bookmark.path.clone();
    let is_dir = bookmark.is_dir();
    drop(bookmarks);

    if is_dir {
        state.set_root_directory(&path);
    } else {
        state.open_file(&path);
        state.focused_panel.set(FocusedPanel::Content);
    }
}

// ============================================================================
// Shared Helpers
// ============================================================================

/// Scroll the `.keyboard-focused` element within `container_selector` into view.
///
/// Uses `requestAnimationFrame` to wait for the DOM update after the cursor signal
/// changes, then scrolls the focused element into view if needed.
pub fn scroll_keyboard_focused_into_view(container_selector: &'static str) {
    spawn(async move {
        let js = format!(
            r#"
            requestAnimationFrame(() => {{
                const el = document.querySelector('{container_selector} .keyboard-focused');
                if (el) el.scrollIntoView({{ block: 'nearest' }});
            }});
            "#
        );
        let _ = document::eval(&js).await;
    });
}
