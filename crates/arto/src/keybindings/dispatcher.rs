use dioxus::document;
use dioxus::prelude::*;

use crate::document_link::{open_document_link, LinkOpen};
use crate::pinned_search::add_pinned_search;
use crate::state::sidebar_cursor;
use crate::state::{AppState, FocusedPanel};
use crate::theme::Theme;
use crate::utils::task::spawn_detached;

use super::Action;

/// Execute an action by dispatching to the appropriate handler.
///
/// This is the main entry point for action execution after the engine
/// matches a keybinding. `Cancel` is handled separately in app.rs
/// (cancel chain logic) and should not reach here.
///
/// Actions are dispatched from menu items as well as from the keyboard, and
/// a menu closes as part of the click that picks an item. Everything async
/// here is therefore spawned with [`spawn_detached`], so an action outlives
/// the widget that asked for it.
pub fn dispatch_action(action: &Action, mut state: AppState) {
    match action {
        // --- Scroll (JS eval) ---
        Action::ScrollDown => scroll_eval("down"),
        Action::ScrollUp => scroll_eval("up"),
        Action::ScrollPageDown => scroll_eval("pageDown"),
        Action::ScrollPageUp => scroll_eval("pageUp"),
        Action::ScrollHalfPageDown => scroll_eval("halfPageDown"),
        Action::ScrollHalfPageUp => scroll_eval("halfPageUp"),
        Action::ScrollTop => scroll_eval("toTop"),
        Action::ScrollBottom => scroll_eval("toBottom"),

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
        Action::ZoomIn => state.zoom_in(),
        Action::ZoomOut => state.zoom_out(),
        Action::ZoomReset => state.zoom_reset(),

        // --- Window ---
        Action::WindowNew => {
            crate::window::create_main_window_sync(
                &dioxus::desktop::window(),
                crate::state::Document::default(),
                crate::window::CreateMainWindowConfigParams::default(),
            );
        }
        // Putting the document down is what "new" means for a window that
        // reads one: the library takes its place, offering the next.
        Action::WindowNewDocument => {
            state.update_document(|document| *document = crate::state::Document::default());
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
            }
            state.focused_panel.set(FocusedPanel::LeftSidebar);
            // Initialize cursor to first item if not set
            if state.sidebar_cursor.read().is_none() {
                if let Some((root, expanded, show_all)) = extract_sidebar_data(&state) {
                    let items = sidebar_cursor::visible_items_in_roots(&root, &expanded, show_all);
                    if let Some(first) = items.first() {
                        state.sidebar_cursor.set(Some(first.clone()));
                    }
                }
            }
        }
        Action::FocusQuickAccess => {
            // Show overlay if not pinned (quick access is part of sidebar)
            if !state.sidebar.read().pinned {
                state.left_hover_active.set(true);
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
        // Walking the root itself is gone: a root's parent is added alongside
        // it now rather than replacing it, and the tree holds several roots at
        // once, so there is no single "current directory" to step through.
        Action::DirectoryParent => {
            let parent = state
                .sidebar
                .read()
                .primary_root()
                .and_then(|root| root.parent().map(|p| p.to_path_buf()));
            if let Some(parent) = parent {
                state.add_root(parent);
            }
        }

        // --- Palette ---
        Action::PaletteOpen => state.toggle_palette(),

        // --- File ---
        Action::SidebarFaceFiles => state.show_face(crate::state::Face::Files),
        Action::SidebarFaceRecent => state.show_face(crate::state::Face::Recent),
        Action::SidebarFaceStarred => state.show_face(crate::state::Face::Starred),

        Action::FileOpen => {
            if let Some(file) = pick_markdown_file() {
                state.open_file(file);
            }
        }
        Action::FileOpenDirectory => {
            if let Some(dir) = pick_directory() {
                state.add_root(dir);
            }
        }
        Action::FileSetParentAsRoot => set_parent_of_current_file_as_root(&mut state),
        Action::FileToggleBookmark => toggle_bookmark_on_cursor_or_current(&mut state),
        Action::FileOpenLink => open_link_from_cursor(&mut state, false),
        Action::FileOpenLinkInNewWindow => open_link_from_cursor(&mut state, true),
        Action::FileSaveImageAs => save_image_from_cursor(),
        Action::FilePreferences => {
            state.open_preferences();
        }
        Action::FileRevealInFinder => {
            if let Some(file) = get_current_file(&state) {
                crate::utils::file_operations::reveal_in_finder(&file);
            }
        }
        Action::FilePrint => {
            crate::utils::print::print_window(get_current_file(&state));
        }

        // --- App ---
        Action::AppAbout => {
            crate::components::content::set_preferences_tab_to_about();
            state.open_preferences();
        }
        Action::AppQuit => {
            dioxus::desktop::window().close();
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
/// Returns `(roots, expanded_dirs, show_all_files)` if the tree has any root.
fn extract_sidebar_data(
    state: &AppState,
) -> Option<(
    Vec<std::path::PathBuf>,
    std::collections::HashSet<std::path::PathBuf>,
    bool,
)> {
    let sidebar = state.sidebar.read();
    let roots: Vec<_> = sidebar.roots.all().cloned().collect();
    (!roots.is_empty()).then(|| (roots, sidebar.expanded_dirs.clone(), sidebar.show_all_files))
}

enum CursorDirection {
    Down,
    Up,
}

fn dispatch_cursor_move(state: &mut AppState, direction: CursorDirection) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar => {
            if let Some((root, expanded, show_all)) = extract_sidebar_data(state) {
                let items = sidebar_cursor::visible_items_in_roots(&root, &expanded, show_all);
                let current = state.sidebar_cursor.read().clone();
                let next = match direction {
                    CursorDirection::Down => sidebar_cursor::move_down(&current, &items),
                    CursorDirection::Up => sidebar_cursor::move_up(&current, &items),
                };
                state.sidebar_cursor.set(next);
                scroll_cursor_into_view();
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
        FocusedPanel::LeftSidebar => {
            let cursor = state.sidebar_cursor.read().clone();
            let Some(path) = cursor else { return };
            if path.is_dir() {
                state.add_root(&path);
            } else {
                state.open_file(&path);
            }
        }
        // Quick access: same as cursor.open (open the bookmark)
        FocusedPanel::QuickAccess => open_quick_access(state),
        FocusedPanel::Content => {}
    }
}

/// cursor.open — "Open/expand": directory → expand tree, file → open, heading → scroll to.
fn dispatch_cursor_open(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar => open_sidebar(state),
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
        let items = sidebar_cursor::visible_items_in_roots(&root, &expanded, show_all);
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
                state.add_root(&path);
            } else {
                state.open_file(&path);
            }
        }
    }
}

fn dispatch_cursor_collapse(state: &mut AppState) {
    let panel = *state.focused_panel.read();
    match panel {
        FocusedPanel::LeftSidebar => {
            let cursor = state.sidebar_cursor.read().clone();
            if let Some(path) = cursor {
                let is_expanded_dir =
                    { path.is_dir() && state.sidebar.read().expanded_dirs.contains(&path) };
                if is_expanded_dir {
                    // Collapse this directory
                    state.toggle_directory_expansion(&path);
                } else {
                    // Move cursor to parent directory in the visible list
                    let parent =
                        extract_sidebar_data(state).and_then(|(root, expanded, show_all)| {
                            let items =
                                sidebar_cursor::visible_items_in_roots(&root, &expanded, show_all);
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
        FocusedPanel::QuickAccess | FocusedPanel::Content => {}
    }
}

/// Scroll the keyboard-focused element into view using JS.
fn scroll_cursor_into_view() {
    spawn_detached(async move {
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
    spawn_detached(async move {
        let js = format!("window.Arto.search.navigate('{direction}')");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%direction, "Search navigate failed: {e}");
        }
    });
}

fn search_open(state: &mut AppState) {
    let mut app_state = *state;
    spawn_detached(async move {
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
    spawn_detached(async move {
        if let Err(e) = document::eval("window.Arto.search.clear();").await {
            tracing::debug!("Search clear failed: {e}");
        }
    });
}

fn search_pin_current(state: &mut AppState) {
    let mut app_state = *state;
    spawn_detached(async move {
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
    spawn_detached(async move {
        let js = format!("window.Arto.scroll.{method}();");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%method, "Scroll eval failed: {e}");
        }
    });
}

pub(crate) fn content_cursor_eval(method: &'static str) {
    spawn_detached(async move {
        let js = format!("window.Arto.contentCursor.{method}()");
        if let Err(e) = document::eval(&js).await {
            tracing::debug!(%method, "Content cursor eval failed: {e}");
        }
    });
}

fn copy_content_cursor_text(js_getter: &'static str) {
    spawn_detached(async move {
        let js = format!(
            "(() => {{ const t = window.Arto?.contentCursor?.{js_getter}() ?? ''; dioxus.send(t); }})()"
        );
        let mut eval = document::eval(&js);
        match eval.recv::<String>().await {
            Ok(text) if !text.is_empty() => {
                // What the document shows for an image is a URL only this app
                // can resolve, so anything leaving it says where the file is.
                let text = crate::assets::images::with_paths_for_urls(&text);
                crate::utils::clipboard::copy_text(&text);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(js_getter, "Content cursor copy failed: {e}"),
        }
    });
}

fn copy_image_from_cursor(opaque: bool) {
    spawn_detached(async move {
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

/// Put the PNG a rasterization returns on the clipboard, and say so when
/// there is none.
///
/// `subject` names what was being rasterized, for the log line. Every way
/// this can fail is reported: silence is what let a Copy Image that copied
/// nothing look like a menu item nobody had clicked.
pub(crate) async fn copy_rasterized_image(mut eval: document::Eval, subject: &str) {
    match eval.recv::<Option<String>>().await {
        Ok(Some(data_url)) => {
            crate::utils::clipboard::copy_image_from_data_url(&data_url);
            show_action_feedback("Copied");
        }
        Ok(None) => {
            tracing::warn!(%subject, "Rasterizing for clipboard copy produced no image")
        }
        Err(e) => {
            tracing::warn!(%e, %subject, "Rasterizing for clipboard copy failed")
        }
    }
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
    copy_rasterized_image(document::eval(&js), kind).await;
}

pub(crate) async fn copy_image_from_src(src: String, opaque: bool) {
    // An image the app serves for the document is read here rather than in the
    // WebView. Drawing it to a canvas there would taint the canvas — it comes
    // from the app's own origin, not the document's — and asking for it as a
    // CORS request is worse still, because a custom scheme does not take part
    // in CORS and the load simply fails. Reading it costs one encode of an
    // image somebody deliberately asked to copy.
    let rasterize_src = if let Some(data_url) = crate::assets::images::data_url_for(&src) {
        data_url
    } else if src.starts_with("http://") || src.starts_with("https://") {
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
    copy_rasterized_image(document::eval(&js), "image").await;
}

fn copy_image_path_from_cursor() {
    spawn_detached(async move {
        let js =
            "(() => { const src = window.Arto?.contentCursor?.getImageSrc?.() ?? ''; dioxus.send(src); })()";
        let mut eval = document::eval(js);
        match eval.recv::<String>().await {
            Ok(src) if !src.is_empty() => {
                // The path the reader means, not the URL the WebView was given.
                let src = crate::assets::images::with_paths_for_urls(&src);
                crate::utils::clipboard::copy_text(&src);
                show_action_feedback("Copied");
            }
            Ok(_) => {}
            Err(e) => tracing::debug!("Copy image path failed: {e}"),
        }
    });
}

fn copy_link_path_from_cursor() {
    spawn_detached(async move {
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
    spawn_detached(async move {
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
    spawn_detached(async move {
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
                let source = crate::utils::source_lines::extract_source_lines(&file, start, end)?;
                if selected_text.trim().is_empty() {
                    return Some(source);
                }
                Some(
                    crate::markdown::extract_source_selection(&source, &selected_text)
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
    spawn_detached(async move {
        let _ = document::eval(&js).await;
    });
}

fn get_current_file(state: &AppState) -> Option<std::path::PathBuf> {
    match &state.document.read().content {
        crate::state::DocumentContent::File(path) => Some(path.clone()),
        _ => None,
    }
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
        FocusedPanel::Content => None,
    }
}

fn open_content_viewer_from_cursor(state: &AppState) {
    let theme = *state.current_theme.read();
    spawn_detached(async move {
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

fn open_link_from_cursor(state: &mut AppState, open_in_new_window: bool) {
    let Some(current_file) = get_current_file(state) else {
        return;
    };
    let mut app_state = *state;

    spawn_detached(async move {
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

        let how = if open_in_new_window {
            LinkOpen::NewWindow
        } else {
            LinkOpen::Here {
                scroll_anchor: *app_state.current_scroll_anchor.read(),
            }
        };
        open_document_link(&mut app_state, &current_file, &href, how);
    });
}

fn save_image_from_cursor() {
    spawn_detached(async move {
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
                    // `save_image` knows `data:` and `http(s)`; the app's own
                    // protocol resolves nowhere outside a WebView, so an image
                    // the app serves is read here and handed over as bytes.
                    let src = crate::assets::images::data_url_for(&src).unwrap_or(src);
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
    match eval.recv::<Option<String>>().await {
        Ok(Some(data_url)) => {
            std::thread::spawn(move || {
                crate::utils::image::save_image(&data_url);
            });
        }
        Ok(None) => tracing::warn!(%kind, "Rasterizing for save produced no image"),
        Err(e) => tracing::warn!(%e, %kind, "Rasterizing for save failed"),
    }
}

fn set_parent_of_current_file_as_root(state: &mut AppState) {
    let Some(file) = get_current_file(state) else {
        return;
    };
    let Some(parent) = file.parent() else {
        return;
    };
    state.add_root(parent);
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
