use dioxus::document;
use dioxus::prelude::*;

use crate::document_link::{open_document_link, LinkOpen};
use crate::pinned_search::add_pinned_search;
use crate::state::sidebar_cursor;
use crate::state::{AppState, FocusedPanel};
use crate::theme::Theme;
use crate::utils::task::spawn_detached;

use super::Action;

mod clipboard;
mod contents;
mod palette;
mod panel;
mod reveal;
mod search;

use clipboard::*;
use contents::*;
use palette::*;
use panel::*;
use reveal::*;
use search::*;

// What the rest of the app reaches for by name, which is what the menus and
// the content's own context menu need.
pub(crate) use clipboard::{copy_image_from_src, copy_rasterized_image};
pub(crate) use palette::activate_palette_row;
pub(crate) use reveal::content_cursor_eval;

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
        Action::SearchClear => state.close_search(),
        Action::SearchPinCurrent => search_pin_current(&mut state),

        // --- Zoom ---
        Action::ZoomIn => state.zoom_in(),
        Action::ZoomOut => state.zoom_out(),
        Action::ZoomReset => state.zoom_reset(),

        // --- Window ---
        // A new window is a fresh start: the places, the welcome page, and nothing
        // this window happened to have wandered into.
        Action::WindowNew => {
            crate::window::create_main_window_sync(
                &dioxus::desktop::window(),
                crate::state::Document::default(),
                crate::window::CreateMainWindowConfigParams::default(),
            );
        }
        Action::WindowDuplicate => duplicate_window(&mut state),
        // Putting the document down is what "new" means for a window that
        // reads one: the welcome page takes its place, offering the next.
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
        // Closing the panel hands the keyboard back to the document; that
        // belongs to `hide_panel`, so both the rail and this go through it.
        Action::WindowToggleSidebar => state.toggle_sidebar(),
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
        // One per face: the keyboard goes to the list that was asked for,
        // rather than to whichever the panel happened to be showing.
        Action::FocusPlaces => face_to(&mut state, crate::state::Face::Places),
        Action::FocusRecent => face_to(&mut state, crate::state::Face::Recent),
        Action::FocusStarred => face_to(&mut state, crate::state::Face::Starred),
        Action::FocusContent => state.focus_content(),

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
        Action::PaletteNext => step_palette(&mut state, true),
        Action::PalettePrev => step_palette(&mut state, false),
        Action::PaletteConfirm => {
            let at = crate::components::palette::cursor_row(&state, *state.palette_rows.read());
            activate_palette_row(state, at);
        }
        Action::PaletteClose => state.close_palette(),

        // --- Contents ---
        Action::ContentsToggle => state.toggle_contents(),
        Action::ContentsNext => step_contents(&mut state, true),
        Action::ContentsPrev => step_contents(&mut state, false),
        Action::ContentsConfirm => confirm_contents(&mut state),
        Action::ContentsClose => state.close_contents(),

        // --- File ---
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
        // Both are answered before dispatch, in `keybinding_engine`: they act
        // on the overlay, which is the window's own state and not the
        // document's.
        Action::HelpShowKeyboardShortcuts => {}

        // --- Sidebar ---
        Action::SidebarFacePlaces => face_to(&mut state, crate::state::Face::Places),
        Action::SidebarFaceRecent => face_to(&mut state, crate::state::Face::Recent),
        Action::SidebarFaceStarred => face_to(&mut state, crate::state::Face::Starred),
        Action::SidebarFaceNext => step_face(&mut state, true),
        Action::SidebarFacePrev => step_face(&mut state, false),
        Action::SidebarToggleShowAllFiles => {
            let current = state.sidebar.read().show_all_files;
            state.sidebar.write().show_all_files = !current;
        }

        // --- Theme ---
        Action::ThemeSetLight => state.current_theme.set(Theme::Light),
        Action::ThemeSetDark => state.current_theme.set(Theme::Dark),
        Action::ThemeSetAuto => state.current_theme.set(Theme::Auto),

        Action::Cancel => {}
    }
}

pub(crate) fn show_action_feedback(message: &str) {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"Done\"".to_string());
    let js = format!("window.Arto?.feedback?.show?.({msg});");
    spawn_detached(async move {
        let _ = document::eval(&js).await;
    });
}

/// Open a copy of this window beside it.
///
/// A duplicate is for reading the same thing two ways — the document, the
/// place in it, and the folders that were open to reach it — so it carries
/// all three, and the window's own look with them. Its own scroll position
/// is saved into the history first, which is what the copy then restores.
fn duplicate_window(state: &mut AppState) {
    let anchor = *state.current_scroll_anchor.read();
    state.save_current_scroll_anchor(anchor);

    let document = state.document();
    let (temps, sidebar_pinned, sidebar_width, sidebar_show_all_files, sidebar_zoom_level) = {
        let sidebar = state.sidebar.read();
        (
            sidebar.roots.temps().to_vec(),
            sidebar.pinned,
            sidebar.width,
            sidebar.show_all_files,
            sidebar.zoom_level,
        )
    };

    let params = crate::window::CreateMainWindowConfigParams {
        directory: None,
        temps,
        theme: *state.current_theme.read(),
        content_full_width: *state.content_full_width.read(),
        sidebar_pinned,
        sidebar_width,
        sidebar_show_all_files,
        sidebar_zoom_level,
        zoom_level: *state.zoom_level.read(),
        ..crate::window::CreateMainWindowConfigParams::default()
    };
    crate::window::create_main_window_sync(&dioxus::desktop::window(), document, params);
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
