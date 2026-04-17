use dioxus::document;
use dioxus::prelude::*;

use crate::pinned_search::add_pinned_search;
use crate::state::sidebar_cursor;
use crate::state::{AppState, FocusedPanel, SidebarPanel};
use crate::theme::Theme;
use crate::window::settings::normalize_zoom_level;

use super::action::Action;

/// Execute an action by dispatching to the appropriate handler.
///
/// This is the main entry point for action execution after the engine
/// matches a keybinding. `Cancel` is handled separately in app.rs
/// (cancel chain logic) and should not reach here.
pub fn dispatch_action(action: &Action, mut state: AppState) {
    match action {
        // --- Scroll (JS eval) ---
        Action::ScrollDown => scroll_eval("scrollDown"),
        Action::ScrollUp => scroll_eval("scrollUp"),
        Action::ScrollPageDown => scroll_eval("scrollPageDown"),
        Action::ScrollPageUp => scroll_eval("scrollPageUp"),
        Action::ScrollHalfPageDown => scroll_eval("scrollHalfPageDown"),
        Action::ScrollHalfPageUp => scroll_eval("scrollHalfPageUp"),
        Action::ScrollTop => scroll_eval("scrollToTop"),
        Action::ScrollBottom => scroll_eval("scrollToBottom"),

        // --- Tab ---
        Action::TabNew => {
            state.add_empty_tab(true);
        }
        Action::TabClose => {
            let active_tab = *state.active_tab.read();
            state.close_tab(active_tab);
        }
        Action::TabCloseAll => {
            let mut tabs = state.tabs.write();
            tabs.clear();
            tabs.push(crate::state::Tab::default());
            state.active_tab.set(0);
        }
        Action::TabCloseOthers => {
            let active_tab = *state.active_tab.read();
            state.close_others(active_tab);
        }
        Action::TabTogglePin => {
            let active_tab = *state.active_tab.read();
            state.toggle_pin(active_tab);
        }
        Action::TabOpenInNewWindow => open_current_tab_in_new_window(&mut state),
        Action::TabNext => dispatch_tab_cycle(&mut state, true),
        Action::TabPrev => dispatch_tab_cycle(&mut state, false),

        // --- History ---
        Action::HistoryBack => {
            state.save_scroll_and_go_back();
        }
        Action::HistoryForward => {
            state.save_scroll_and_go_forward();
        }

        // --- Search ---
        Action::SearchOpen => search_open(&mut state),
        Action::SearchNext => search_navigate_eval("next"),
        Action::SearchPrev => search_navigate_eval("prev"),
        Action::SearchClear => search_clear_eval(),
        Action::SearchPinCurrent => search_pin_current(&mut state),

        // --- Zoom ---
        Action::ZoomIn => {
            let current = normalize_zoom_level(*state.zoom_level.read());
            state.zoom_level.set(normalize_zoom_level(current + 0.1));
        }
        Action::ZoomOut => {
            let current = normalize_zoom_level(*state.zoom_level.read());
            state.zoom_level.set(normalize_zoom_level(current - 0.1));
        }
        Action::ZoomReset => {
            state.zoom_level.set(1.0);
        }

        // --- Window ---
        Action::WindowNew => {
            crate::window::create_main_window_sync(
                &dioxus::desktop::window(),
                crate::state::Tab::default(),
                crate::window::CreateMainWindowConfigParams::default(),
            );
        }
        Action::WindowClose => {
            dioxus::desktop::window().close();
        }
        Action::WindowCloseAllChildWindows => {
            crate::window::close_child_windows_for_last_focused();
        }
        Action::WindowCloseAllWindows => {
            crate::window::close_all_main_windows();
        }
        Action::WindowToggleSidebar => {
            let closing = state.sidebar.read().pinned;
            state.toggle_sidebar();
            // Return focus to Content when closing a focused sidebar panel
            if closing {
                let panel = *state.focused_panel.read();
                if matches!(panel, FocusedPanel::LeftSidebar | FocusedPanel::QuickAccess) {
                    state.focused_panel.set(FocusedPanel::Content);
                }
            }
        }
        Action::WindowToggleRightSidebar => {
            let closing = *state.right_sidebar_pinned.read();
            state.toggle_right_sidebar();
            if closing && *state.focused_panel.read() == FocusedPanel::RightSidebar {
                state.focused_panel.set(FocusedPanel::Content);
            }
        }
        Action::WindowReload => {
            let current = *state.reload_trigger.read();
            state.reload_trigger.set(current + 1);
        }

        // --- Clipboard (path variants) ---
        Action::CopyFilePath => {
            if let Some(file) = get_current_file(&state) {
                crate::utils::clipboard::copy_text(file.to_string_lossy());
                show_action_feedback("Copied");
            }
        }
        Action::CopyFilePathWithLine | Action::CopyFilePathWithRange => {
            if let Some(file) = get_current_file(&state) {
                let is_range = matches!(action, Action::CopyFilePathWithRange);
                copy_file_path_with_line(file, is_range);
            }
        }

        // --- Clipboard (content copy) ---
        Action::CopyCode => copy_content_cursor_text("getCodeText"),
        Action::CopyCodeAsMarkdown => copy_content_cursor_text("getCodeAsMarkdown"),
        Action::CopyTableAsTsv => copy_content_cursor_text("getTableAsTsv"),
        Action::CopyTableAsCsv => copy_content_cursor_text("getTableAsCsv"),
        Action::CopyTableAsMarkdown => copy_content_cursor_text("getTableAsMarkdown"),
        Action::CopyImageAsMarkdown => copy_content_cursor_text("getImageAsMarkdown"),
        Action::CopyImage => copy_image_from_cursor(false),
        Action::CopyImageWithBackground => copy_image_from_cursor(true),
        Action::CopyImagePath => copy_image_path_from_cursor(),
        Action::CopyAsMarkdown => {
            if let Some(file) = get_current_file(&state) {
                copy_markdown_source(file);
            }
        }
        Action::CopyLinkPath => copy_link_path_from_cursor(),

        // --- Focus ---
        Action::FocusLeftSidebar => {
            // Show overlay if not pinned, then focus it
            if !state.sidebar.read().pinned {
                state.left_hover_active.set(true);
                state.right_hover_active.set(false);
            }
            state.focused_panel.set(FocusedPanel::LeftSidebar);
            initialize_sidebar_focus(&mut state, FocusedPanel::LeftSidebar);
        }
        Action::FocusRightSidebar => {
            // Show overlay if not pinned, then focus it
            if !*state.right_sidebar_pinned.read() {
                state.right_hover_active.set(true);
                state.left_hover_active.set(false);
            }
            state.focused_panel.set(FocusedPanel::RightSidebar);
            initialize_sidebar_focus(&mut state, FocusedPanel::RightSidebar);
        }
        Action::FocusQuickAccess => {
            // Show overlay if not pinned (quick access is part of sidebar)
            if !state.sidebar.read().pinned {
                state.left_hover_active.set(true);
                state.right_hover_active.set(false);
            }
            state.focused_panel.set(FocusedPanel::QuickAccess);
            // Initialize cursor to first bookmark if not set
            if state.quick_access_cursor.read().is_none()
                && !crate::bookmarks::BOOKMARKS.read().items.is_empty()
            {
                state.quick_access_cursor.set(Some(0));
            }
        }
        Action::FocusContent => {
            state.focused_panel.set(FocusedPanel::Content);
            state.left_hover_active.set(false);
            state.right_hover_active.set(false);
        }

        // --- Cursor ---
        Action::CursorDown => dispatch_cursor_move(&mut state, CursorDirection::Down),
        Action::CursorUp => dispatch_cursor_move(&mut state, CursorDirection::Up),
        Action::CursorEnter => dispatch_cursor_enter(&mut state),
        Action::CursorOpen => dispatch_cursor_open(&mut state),
        Action::CursorCollapse => dispatch_cursor_collapse(&mut state),

        // --- Content cursor (engine restricts to Content context) ---
        Action::ContentNext => content_cursor_eval("next"),
        Action::ContentPrev => content_cursor_eval("prev"),
        Action::ContentNextHeading => content_cursor_eval("nextHeading"),
        Action::ContentPrevHeading => content_cursor_eval("prevHeading"),
        Action::ContentOpenViewer => open_content_viewer_from_cursor(&state),

        // --- Directory ---
        Action::DirectoryParent => {
            state.go_to_parent_directory();
        }
        Action::DirectoryBack => {
            state.go_back_directory();
        }
        Action::DirectoryForward => {
            state.go_forward_directory();
        }

        // --- File ---
        Action::FileOpen => {
            if let Some(file) = pick_markdown_file() {
                state.open_file(file);
            }
        }
        Action::FileOpenDirectory => {
            if let Some(dir) = pick_directory() {
                state.set_root_directory(dir);
            }
        }
        Action::FileSetParentAsRoot => set_parent_of_current_file_as_root(&mut state),
        Action::FileToggleBookmark => toggle_bookmark_on_cursor_or_current(&mut state),
        Action::FileOpenLink => open_link_from_cursor(&mut state, false),
        Action::FileOpenLinkInNewTab => open_link_from_cursor(&mut state, true),
        Action::FileSaveImageAs => save_image_from_cursor(),
        Action::FilePreferences => {
            state.open_preferences();
        }
        Action::FileRevealInFinder => {
            if let Some(file) = get_current_file(&state) {
                crate::utils::file_operations::reveal_in_finder(&file);
            }
        }

        // --- App ---
        Action::AppAbout => {
            crate::components::content::set_preferences_tab_to_about();
            state.open_preferences();
        }
        Action::AppQuit => {
            crate::window::shutdown_all_windows();
        }
        Action::AppGoToHomepage => {
            let _ = open::that("https://github.com/arto-app/Arto");
        }
        Action::HelpShowKeyboardShortcuts => {}

        // --- Sidebar ---
        Action::SidebarToggleShowAllFiles => {
            let current = state.sidebar.read().show_all_files;
            state.sidebar.write().show_all_files = !current;
        }

        // --- Right sidebar ---
        Action::RightSidebarShowContents => {
            if !*state.right_sidebar_pinned.read() {
                state.right_hover_active.set(true);
                state.left_hover_active.set(false);
            }
            state.set_right_sidebar_panel(SidebarPanel::Contents);
            state.focused_panel.set(FocusedPanel::RightSidebar);
        }
        Action::RightSidebarShowSearch => {
            if !*state.right_sidebar_pinned.read() {
                state.right_hover_active.set(true);
                state.left_hover_active.set(false);
            }
            state.set_right_sidebar_panel(SidebarPanel::Search);
            state.focused_panel.set(FocusedPanel::RightSidebar);
        }

        // --- Theme ---
        Action::ThemeSetLight => state.current_theme.set(Theme::Light),
        Action::ThemeSetDark => state.current_theme.set(Theme::Dark),
        Action::ThemeSetAuto => state.current_theme.set(Theme::Auto),

        // Cancel is handled in app.rs before dispatch
        Action::Cancel => {}
    }
}

/// Clone sidebar data needed for cursor navigation, releasing the read guard.
///
/// Returns `(root_directory, expanded_dirs, show_all_files)` if a root is set.
fn extract_sidebar_data(
    state: &AppState,
) -> Option<(
    std::path::PathBuf,
    std::collections::HashSet<std::path::PathBuf>,
    bool,
)> {
    let sidebar = state.sidebar.read();
    sidebar.root_directory.as_ref().map(|root| {
        (
            root.clone(),
            sidebar.expanded_dirs.clone(),
            sidebar.show_all_files,
        )
    })
}

/// Cycle to next/previous tab, or toggle right sidebar tab when focused.
fn dispatch_tab_cycle(state: &mut AppState, forward: bool) {
    if *state.focused_panel.read() == FocusedPanel::RightSidebar {
        toggle_right_sidebar_tab(state);
    } else {
        let tabs_len = state.tabs.read().len();
        if tabs_len > 1 {
            let current = *state.active_tab.read();
            let next = if forward {
                (current + 1) % tabs_len
            } else if current == 0 {
                tabs_len - 1
            } else {
                current - 1
            };
            state.switch_to_tab(next);
        }
    }
}

/// Cycle the right sidebar between Contents and Search tabs.
fn toggle_right_sidebar_tab(state: &mut AppState) {
    let panel = match *state.right_sidebar_panel.read() {
        SidebarPanel::Directory => SidebarPanel::Contents,
        SidebarPanel::Contents => SidebarPanel::Search,
        SidebarPanel::Search => SidebarPanel::Directory,
    };
    state.set_right_sidebar_panel(panel);
}

enum CursorDirection {
    Down,
    Up,
}

fn dispatch_cursor_move(state: &mut AppState, direction: CursorDirection) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar | FocusedPanel::RightSidebar => {
            match active_sidebar_panel(state, panel) {
                SidebarPanel::Directory => {
                    if let Some((root, expanded, show_all)) = extract_sidebar_data(state) {
                        let items = sidebar_cursor::visible_items(&root, &expanded, show_all);
                        let current = state.sidebar_cursor.read().clone();
                        let next = match direction {
                            CursorDirection::Down => sidebar_cursor::move_down(&current, &items),
                            CursorDirection::Up => sidebar_cursor::move_up(&current, &items),
                        };
                        state.sidebar_cursor.set(next);
                        scroll_cursor_into_view();
                    }
                }
                SidebarPanel::Contents => {
                    let headings_len = state.right_sidebar_headings.read().len();
                    if headings_len > 0 {
                        let current = *state.toc_cursor.read();
                        state
                            .toc_cursor
                            .set(move_index_cursor(current, headings_len, &direction));
                        scroll_cursor_into_view();
                    }
                }
                SidebarPanel::Search => {}
            }
        }
        FocusedPanel::QuickAccess => {
            let bookmarks_len = crate::bookmarks::BOOKMARKS.read().items.len();
            if bookmarks_len > 0 {
                let current = *state.quick_access_cursor.read();
                state.quick_access_cursor.set(move_index_cursor(
                    current,
                    bookmarks_len,
                    &direction,
                ));
                scroll_cursor_into_view();
            }
        }
        FocusedPanel::Content => {}
    }
}

/// Move an index-based cursor up or down within a list of `len` items.
/// No-wrap: stays at boundary when reaching start/end.
fn move_index_cursor(
    current: Option<usize>,
    len: usize,
    direction: &CursorDirection,
) -> Option<usize> {
    match direction {
        CursorDirection::Down => match current {
            None => Some(0),
            Some(i) if i + 1 < len => Some(i + 1),
            Some(i) => Some(i), // stay at end
        },
        CursorDirection::Up => match current {
            None => Some(len - 1),
            Some(0) => Some(0), // stay at start
            Some(i) => Some(i - 1),
        },
    }
}

/// cursor.enter — "Enter into": directory → set as root, file → open, heading → scroll to.
fn dispatch_cursor_enter(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar | FocusedPanel::RightSidebar => {
            match active_sidebar_panel(state, panel) {
                SidebarPanel::Directory => {
                    let cursor = state.sidebar_cursor.read().clone();
                    let Some(path) = cursor else { return };
                    if path.is_dir() {
                        state.set_root_directory(&path);
                    } else {
                        state.open_file(&path);
                    }
                }
                SidebarPanel::Contents => open_right_sidebar(state),
                SidebarPanel::Search => {}
            }
        }
        FocusedPanel::QuickAccess => open_quick_access(state),
        FocusedPanel::Content => {}
    }
}

/// cursor.open — "Open/expand": directory → expand tree, file → open, heading → scroll to.
fn dispatch_cursor_open(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar | FocusedPanel::RightSidebar => {
            match active_sidebar_panel(state, panel) {
                SidebarPanel::Directory => open_sidebar(state),
                SidebarPanel::Contents => open_right_sidebar(state),
                SidebarPanel::Search => {}
            }
        }
        FocusedPanel::QuickAccess => open_quick_access(state),
        FocusedPanel::Content => {}
    }
}

fn open_sidebar(state: &mut AppState) {
    let cursor = state.sidebar_cursor.read().clone();
    let Some(path) = cursor else { return };

    if !path.is_dir() {
        state.open_file(&path);
        return;
    }

    // Open directory: expand it and move cursor to first child
    {
        let is_expanded = state.sidebar.read().expanded_dirs.contains(&path);
        if !is_expanded {
            state.toggle_directory_expansion(&path);
        }
    }
    // Recompute visible items and move to first child
    let next_cursor = {
        let Some((root, expanded, show_all)) = extract_sidebar_data(state) else {
            return;
        };
        let items = sidebar_cursor::visible_items(&root, &expanded, show_all);
        items
            .iter()
            .position(|p| p == &path)
            .and_then(|pos| items.get(pos + 1).cloned())
    };
    if let Some(next) = next_cursor {
        state.sidebar_cursor.set(Some(next));
        scroll_cursor_into_view();
    }
}

fn open_right_sidebar(state: &mut AppState) {
    let heading_id = {
        let idx = *state.toc_cursor.read();
        idx.and_then(|i| {
            state
                .right_sidebar_headings
                .read()
                .get(i)
                .map(|h| h.id.clone())
        })
    };
    let Some(id) = heading_id else { return };
    spawn(async move {
        let id_json = serde_json::to_string(&id).unwrap_or_else(|_| "null".to_string());
        let js = format!(
            r#"
            (() => {{
                const content = document.querySelector('.content');
                const el = document.getElementById({id_json});
                if (content && el) {{
                    const contentRect = content.getBoundingClientRect();
                    const elRect = el.getBoundingClientRect();
                    const top = content.scrollTop + (elRect.top - contentRect.top);
                    content.scrollTo({{ top, behavior: 'smooth' }});
                }}
            }})();
            "#,
        );
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%id, "Failed to scroll to heading: {e}");
        }
    });
}

fn open_quick_access(state: &mut AppState) {
    let bookmark_info = {
        let idx = *state.quick_access_cursor.read();
        idx.and_then(|i| {
            let bookmarks = crate::bookmarks::BOOKMARKS.read();
            bookmarks
                .items
                .get(i)
                .map(|b| (b.path.clone(), b.exists(), b.is_dir()))
        })
    };
    if let Some((path, exists, is_dir)) = bookmark_info {
        if exists {
            if is_dir {
                state.set_root_directory(&path);
            } else {
                state.open_file(&path);
            }
        }
    }
}

fn dispatch_cursor_collapse(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar | FocusedPanel::RightSidebar
            if active_sidebar_panel(state, panel) == SidebarPanel::Directory =>
        {
            let cursor = state.sidebar_cursor.read().clone();
            if let Some(path) = cursor {
                let is_expanded_dir =
                    { path.is_dir() && state.sidebar.read().expanded_dirs.contains(&path) };
                if is_expanded_dir {
                    state.toggle_directory_expansion(&path);
                } else {
                    let parent =
                        extract_sidebar_data(state).and_then(|(root, expanded, show_all)| {
                            let items = sidebar_cursor::visible_items(&root, &expanded, show_all);
                            sidebar_cursor::find_parent_dir(&path, &items)
                        });
                    if let Some(parent) = parent {
                        state.sidebar_cursor.set(Some(parent));
                        scroll_cursor_into_view();
                    }
                }
            }
        }
        // No-op for other panels
        FocusedPanel::LeftSidebar
        | FocusedPanel::RightSidebar
        | FocusedPanel::QuickAccess
        | FocusedPanel::Content => {}
    }
}

fn active_sidebar_panel(state: &AppState, focused_panel: FocusedPanel) -> SidebarPanel {
    match focused_panel {
        FocusedPanel::LeftSidebar => *state.left_sidebar_panel.read(),
        FocusedPanel::RightSidebar => *state.right_sidebar_panel.read(),
        FocusedPanel::QuickAccess | FocusedPanel::Content => SidebarPanel::Directory,
    }
}

fn initialize_sidebar_focus(state: &mut AppState, focused_panel: FocusedPanel) {
    match active_sidebar_panel(state, focused_panel) {
        SidebarPanel::Directory => {
            if state.sidebar_cursor.read().is_none() {
                if let Some((root, expanded, show_all)) = extract_sidebar_data(state) {
                    let items = sidebar_cursor::visible_items(&root, &expanded, show_all);
                    if let Some(first) = items.first() {
                        state.sidebar_cursor.set(Some(first.clone()));
                    }
                }
            }
        }
        SidebarPanel::Contents => {
            if state.toc_cursor.read().is_none() && !state.right_sidebar_headings.read().is_empty()
            {
                state.toc_cursor.set(Some(0));
            }
        }
        SidebarPanel::Search => {}
    }
}

/// Scroll the keyboard-focused element into view using JS.
fn scroll_cursor_into_view() {
    spawn(async move {
        if let Err(e) = document::eval(
            r#"
            requestAnimationFrame(() => {
                document.querySelector('.keyboard-focused')?.scrollIntoView({ block: 'nearest' });
            });
            "#,
        )
        .await
        {
            tracing::debug!("Failed to scroll cursor into view: {e}");
        }
    });
}

fn search_navigate_eval(direction: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.search.navigate('{direction}')");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%direction, "Search navigate failed: {e}");
        }
    });
}

fn search_open(state: &mut AppState) {
    let mut app_state = *state;
    spawn(async move {
        let js = r#"
            (() => {
                const s = window.getSelection();
                dioxus.send(s ? s.toString() : "");
            })()
        "#;
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(text) if !text.trim().is_empty() => {
                app_state.open_search_with_text(Some(text));
            }
            Ok(_) | Err(_) => {
                app_state.open_search_with_text(None);
            }
        }
    });
}

fn search_clear_eval() {
    spawn(async move {
        if let Err(e) = document::eval("window.Arto.search.clear();").await {
            tracing::debug!("Search clear failed: {e}");
        }
    });
}

fn search_pin_current(state: &mut AppState) {
    let mut app_state = *state;
    spawn(async move {
        #[derive(serde::Deserialize)]
        struct QueryValue {
            value: String,
        }

        let mut eval = document::eval(
            r#"
            (() => {
                const input = document.querySelector('.search-input');
                dioxus.send({ value: input?.value || '' });
            })()
            "#,
        );
        match eval.recv::<QueryValue>().await {
            Ok(result) if !result.value.is_empty() => {
                let _ = add_pinned_search(result.value);
                app_state.update_search_results(0, 0);
                let _ = document::eval(
                    r#"
                    (() => {
                        const input = document.querySelector('.search-input');
                        if (input) {
                            input.value = '';
                            input.focus();
                        }
                        window.Arto.search.clear();
                    })()
                    "#,
                )
                .await;
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Search pin current failed: {e}"),
        }
    });
}

fn scroll_eval(method: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.scroll.{method}();");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%method, "Scroll eval failed: {e}");
        }
    });
}

pub(crate) fn content_cursor_eval(method: &'static str) {
    spawn(async move {
        let js = format!("window.Arto.contentCursor.{method}()");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%method, "Content cursor eval failed: {e}");
        }
    });
}

fn copy_content_cursor_text(js_getter: &'static str) {
    spawn(async move {
        let js = format!(
            "(() => {{ const t = window.Arto?.contentCursor?.{js_getter}() ?? ''; dioxus.send(t); }})()"
        );
        let mut eval = document::eval(&js);
        match eval.recv::<String>().await {
            Ok(text) if !text.is_empty() => {
                crate::utils::clipboard::copy_text(&text);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(js_getter, "Content cursor copy failed: {e}"),
        }
    });
}

fn copy_image_from_cursor(opaque: bool) {
    spawn(async move {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum CopyImageTarget {
            Image { src: String },
            Math,
            Mermaid,
            None,
        }

        let js = r#"
            (() => {
                const cursor = window.Arto?.contentCursor;
                const el = cursor?.getCurrentElement?.();
                if (!el) { dioxus.send({ kind: 'none' }); return; }

                if (el.tagName === 'IMG') {
                    const src = cursor?.getImageSrc?.() ?? '';
                    if (!src) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'image', src });
                    return;
                }

                if (
                    el instanceof HTMLElement &&
                    (
                        el.classList.contains('preprocessed-math-display') ||
                        el.classList.contains('preprocessed-math')
                    )
                ) {
                    dioxus.send({ kind: 'math' });
                    return;
                }

                if (el instanceof HTMLElement && el.classList.contains('preprocessed-mermaid')) {
                    dioxus.send({ kind: 'mermaid' });
                    return;
                }

                dioxus.send({ kind: 'none' });
            })();
        "#;
        let mut eval = document::eval(js);
        let Ok(target) = eval.recv::<CopyImageTarget>().await else {
            return;
        };

        match target {
            CopyImageTarget::Image { src } => {
                copy_image_from_src(src, opaque).await;
            }
            CopyImageTarget::Math => {
                copy_special_block_from_cursor("mathElement", opaque).await;
            }
            CopyImageTarget::Mermaid => {
                copy_special_block_from_cursor("mermaidElement", opaque).await;
            }
            CopyImageTarget::None => {}
        }
    });
}

async fn copy_special_block_from_cursor(kind: &str, opaque: bool) {
    let opaque_str = if opaque { "true" } else { "false" };
    let js = format!(
        r#"
        (async () => {{
            const cursor = window.Arto?.contentCursor;
            const el = cursor?.getCurrentElement?.();
            if (!(el instanceof HTMLElement)) {{ dioxus.send(null); return; }}
            dioxus.send(await window.Arto.rasterize.{kind}(el, {opaque_str}));
        }})();
        "#,
    );
    let mut eval = document::eval(&js);
    if let Ok(Some(data_url)) = eval.recv::<Option<String>>().await {
        crate::utils::clipboard::copy_image_from_data_url(&data_url);
        show_action_feedback("Copied");
    }
}

pub(crate) async fn copy_image_from_src(src: String, opaque: bool) {
    let rasterize_src = if src.starts_with("http://") || src.starts_with("https://") {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn({
            let src = src.clone();
            move || {
                let _ = tx.send(crate::utils::image::download_image_as_data_url(&src));
            }
        });
        match rx.await {
            Ok(Ok(data_url)) => data_url,
            Ok(Err(e)) => {
                tracing::error!(%e, "Failed to download image for clipboard copy");
                return;
            }
            Err(_) => {
                tracing::error!("Image download thread was cancelled");
                return;
            }
        }
    } else {
        src
    };

    let Ok(src_json) = serde_json::to_string(&rasterize_src) else {
        tracing::error!("Failed to serialize image src as JSON");
        return;
    };
    let opaque_str = if opaque { "true" } else { "false" };
    let js = format!(
        "(async () => {{ dioxus.send(await window.Arto.rasterize.image({}, {})); }})();",
        src_json, opaque_str
    );
    let mut eval = document::eval(&js);
    if let Ok(Some(data_url)) = eval.recv::<Option<String>>().await {
        crate::utils::clipboard::copy_image_from_data_url(&data_url);
        show_action_feedback("Copied");
    }
}

fn copy_image_path_from_cursor() {
    spawn(async move {
        let js =
            "(() => { const src = window.Arto?.contentCursor?.getImageSrc?.() ?? ''; dioxus.send(src); })()";
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(src) if !src.is_empty() => {
                crate::utils::clipboard::copy_text(src);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Copy image path failed: {e}"),
        }
    });
}

fn copy_link_path_from_cursor() {
    spawn(async move {
        let js =
            "(() => { const href = window.Arto?.contentCursor?.getLinkHref?.() ?? ''; dioxus.send(href); })()";
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(href) if !href.is_empty() => {
                crate::utils::clipboard::copy_text(href);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Copy link path failed: {e}"),
        }
    });
}

fn copy_file_path_with_line(file: std::path::PathBuf, is_range: bool) {
    spawn(async move {
        let js =
            "(() => { dioxus.send(window.Arto?.contentCursor?.getSourceLineRange() ?? null); })()";
        let mut eval = document::eval(js);
        if let Ok(Some((start, end))) = eval.recv::<Option<(u32, u32)>>().await {
            let path_str = file.display().to_string();
            let text = if is_range && start != end {
                format!("{path_str}:{start}-{end}")
            } else {
                format!("{path_str}:{start}")
            };
            crate::utils::clipboard::copy_text(&text);
            show_action_feedback("Copied");
        }
    });
}

fn copy_markdown_source(file: std::path::PathBuf) {
    spawn(async move {
        #[derive(serde::Deserialize)]
        struct MarkdownSourceRequest {
            range: Option<(u32, u32)>,
            selected_text: String,
        }

        let js = r#"
            (() => {
                const range = window.Arto?.contentCursor?.getSourceLineRange?.() ?? null;
                const selection = window.getSelection();
                const selected_text = selection ? selection.toString() : "";
                dioxus.send({ range, selected_text });
            })()
        "#;
        let mut eval = document::eval(js);
        if let Ok(MarkdownSourceRequest {
            range: Some((start, end)),
            selected_text,
        }) = eval.recv::<MarkdownSourceRequest>().await
        {
            let handle = std::thread::spawn(move || {
                let source = crate::utils::source_extract::extract_source_lines(&file, start, end)?;
                if selected_text.trim().is_empty() {
                    return Some(source);
                }
                Some(
                    crate::utils::source_extract::extract_source_selection(&source, &selected_text)
                        .unwrap_or(source),
                )
            });
            match handle.join() {
                Ok(Some(md)) => {
                    crate::utils::clipboard::copy_text(&md);
                    show_action_feedback("Copied");
                }
                Ok(None) => tracing::debug!(%start, %end, "No source lines extracted"),
                Err(_) => tracing::debug!("Source extraction thread panicked"),
            }
        }
    });
}

pub(crate) fn show_action_feedback(message: &str) {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"Done\"".to_string());
    let js = format!("window.Arto?.feedback?.show?.({msg});");
    spawn(async move {
        let _ = document::eval(&js).await;
    });
}

fn get_current_file(state: &AppState) -> Option<std::path::PathBuf> {
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

fn pick_markdown_file() -> Option<std::path::PathBuf> {
    use rfd::FileDialog;
    FileDialog::new()
        .add_filter("Markdown", &["md", "markdown"])
        .set_directory(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")))
        .pick_file()
}

fn pick_directory() -> Option<std::path::PathBuf> {
    use rfd::FileDialog;
    FileDialog::new()
        .set_directory(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")))
        .pick_folder()
}

fn toggle_bookmark_on_cursor_or_current(state: &mut AppState) {
    let target_path = get_bookmark_target_path(state).or_else(|| get_current_file(state));
    let Some(path) = target_path else { return };

    let is_bookmarked = crate::bookmarks::toggle_bookmark(&path);
    if is_bookmarked {
        show_action_feedback("Bookmarked");
    } else {
        show_action_feedback("Bookmark removed");
    }
}

fn get_bookmark_target_path(state: &AppState) -> Option<std::path::PathBuf> {
    match *state.focused_panel.read() {
        FocusedPanel::LeftSidebar => state.sidebar_cursor.read().clone(),
        FocusedPanel::QuickAccess => {
            let cursor = *state.quick_access_cursor.read();
            cursor.and_then(|index| {
                let bookmarks = crate::bookmarks::BOOKMARKS.read();
                bookmarks.items.get(index).map(|b| b.path.clone())
            })
        }
        FocusedPanel::RightSidebar | FocusedPanel::Content => None,
    }
}

fn open_content_viewer_from_cursor(state: &AppState) {
    let theme = *state.current_theme.read();
    spawn(async move {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum ViewerTarget {
            Image { src: String, alt: Option<String> },
            Math { source: String },
            Mermaid { source: String },
            None,
        }

        let js = r#"
            (() => {
                const cursor = window.Arto?.contentCursor;
                const el = cursor?.getCurrentElement?.();
                if (!(el instanceof HTMLElement)) { dioxus.send({ kind: 'none' }); return; }

                if (el.tagName === 'IMG') {
                    const src = cursor?.getImageSrc?.() || el.getAttribute('src') || '';
                    if (!src) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({
                        kind: 'image',
                        src,
                        alt: el.getAttribute('alt'),
                    });
                    return;
                }

                if (
                    el.classList.contains('preprocessed-math-display') ||
                    el.classList.contains('preprocessed-math')
                ) {
                    const source = el.dataset.originalContent || '';
                    if (!source) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'math', source });
                    return;
                }

                if (el.classList.contains('preprocessed-mermaid')) {
                    const source = el.dataset.originalContent || '';
                    if (!source) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'mermaid', source });
                    return;
                }

                dioxus.send({ kind: 'none' });
            })();
        "#;
        let mut eval = document::eval(js);
        let Ok(target) = eval.recv::<ViewerTarget>().await else {
            return;
        };

        match target {
            ViewerTarget::Image { src, alt } => {
                crate::window::open_or_focus_image_window(src, alt, theme);
            }
            ViewerTarget::Math { source } => {
                crate::window::open_or_focus_math_window(source, theme);
            }
            ViewerTarget::Mermaid { source } => {
                crate::window::open_or_focus_mermaid_window(source, theme);
            }
            ViewerTarget::None => {}
        }
    });
}

fn open_link_from_cursor(state: &mut AppState, open_in_new_tab: bool) {
    let Some(base_dir) = get_current_file(state).and_then(|f| f.parent().map(|p| p.to_path_buf()))
    else {
        return;
    };
    let mut app_state = *state;

    spawn(async move {
        let js =
            "(() => { const href = window.Arto?.contentCursor?.getLinkHref?.() ?? ''; dioxus.send(href); })()";
        let mut eval = document::eval(js);
        let Ok(href) = eval.recv::<String>().await else {
            return;
        };
        if href.is_empty() {
            return;
        }

        if href.starts_with("http://") || href.starts_with("https://") {
            let _ = open::that(href);
            return;
        }

        let Some(current_file) = get_current_file(&app_state) else {
            return;
        };
        let Some(resolved_link) =
            crate::utils::markdown_link::resolve_markdown_link(&base_dir, &current_file, &href)
        else {
            return;
        };

        if open_in_new_tab {
            if let Some(heading_id) = resolved_link.heading_id {
                app_state.set_pending_heading_navigation(resolved_link.path.clone(), heading_id);
            } else {
                app_state.clear_pending_heading_navigation();
            }
            app_state.add_file_tab(resolved_link.path, true);
        } else {
            app_state.navigate_to_file_at_heading(resolved_link.path, resolved_link.heading_id);
        }
    });
}

fn save_image_from_cursor() {
    spawn(async move {
        #[derive(serde::Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum SaveImageTarget {
            Image { src: String },
            Math,
            Mermaid,
            None,
        }

        let js = r#"
            (() => {
                const cursor = window.Arto?.contentCursor;
                const el = cursor?.getCurrentElement?.();
                if (!el) { dioxus.send({ kind: 'none' }); return; }

                if (el.tagName === 'IMG') {
                    const src = cursor?.getImageSrc?.() ?? '';
                    if (!src) { dioxus.send({ kind: 'none' }); return; }
                    dioxus.send({ kind: 'image', src });
                    return;
                }

                if (
                    el instanceof HTMLElement &&
                    (
                        el.classList.contains('preprocessed-math-display') ||
                        el.classList.contains('preprocessed-math')
                    )
                ) {
                    dioxus.send({ kind: 'math' });
                    return;
                }

                if (el instanceof HTMLElement && el.classList.contains('preprocessed-mermaid')) {
                    dioxus.send({ kind: 'mermaid' });
                    return;
                }

                dioxus.send({ kind: 'none' });
            })()
        "#;
        let mut eval = document::eval(js);
        let Ok(target) = eval.recv::<SaveImageTarget>().await else {
            return;
        };

        match target {
            SaveImageTarget::Image { src } => {
                std::thread::spawn(move || {
                    crate::utils::image::save_image(&src);
                });
            }
            SaveImageTarget::Math => {
                save_special_block_from_cursor("mathElement").await;
            }
            SaveImageTarget::Mermaid => {
                save_special_block_from_cursor("mermaidElement").await;
            }
            SaveImageTarget::None => {}
        }
    });
}

async fn save_special_block_from_cursor(kind: &str) {
    let js = format!(
        r#"
        (async () => {{
            const cursor = window.Arto?.contentCursor;
            const el = cursor?.getCurrentElement?.();
            if (!(el instanceof HTMLElement)) {{ dioxus.send(null); return; }}
            dioxus.send(await window.Arto.rasterize.{kind}(el, true));
        }})();
        "#,
    );
    let mut eval = document::eval(&js);
    if let Ok(Some(data_url)) = eval.recv::<Option<String>>().await {
        std::thread::spawn(move || {
            crate::utils::image::save_image(&data_url);
        });
    }
}

fn set_parent_of_current_file_as_root(state: &mut AppState) {
    let Some(file) = get_current_file(state) else {
        return;
    };
    let Some(parent) = file.parent() else {
        return;
    };
    state.set_root_directory(parent.to_path_buf());
}

fn open_current_tab_in_new_window(state: &mut AppState) {
    let active = *state.active_tab.read();
    let tabs_len = state.tabs.read().len();
    if tabs_len <= 1 {
        return;
    }

    let Some(tab) = state.get_tab(active) else {
        return;
    };

    crate::window::create_main_window_sync(
        &dioxus::desktop::window(),
        tab,
        crate::window::CreateMainWindowConfigParams::default(),
    );
    let _ = state.close_tab(active);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_index_cursor_down_from_none_selects_first() {
        let next = move_index_cursor(None, 3, &CursorDirection::Down);
        assert_eq!(next, Some(0));
    }

    #[test]
    fn move_index_cursor_down_advances_and_stops_at_end() {
        let next = move_index_cursor(Some(1), 3, &CursorDirection::Down);
        assert_eq!(next, Some(2));

        let at_end = move_index_cursor(Some(2), 3, &CursorDirection::Down);
        assert_eq!(at_end, Some(2));
    }

    #[test]
    fn move_index_cursor_up_from_none_selects_last() {
        let next = move_index_cursor(None, 3, &CursorDirection::Up);
        assert_eq!(next, Some(2));
    }

    #[test]
    fn move_index_cursor_up_moves_and_stops_at_start() {
        let next = move_index_cursor(Some(2), 3, &CursorDirection::Up);
        assert_eq!(next, Some(1));

        let at_start = move_index_cursor(Some(0), 3, &CursorDirection::Up);
        assert_eq!(at_start, Some(0));
    }
}
