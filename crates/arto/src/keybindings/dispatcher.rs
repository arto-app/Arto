use dioxus::document;
use dioxus::prelude::*;

use crate::pinned_search::add_pinned_search;
use crate::state::sidebar_cursor;
use crate::state::{AppState, FocusedPanel};
use crate::theme::Theme;
use crate::utils::task::spawn_detached;

use super::Action;

mod clipboard;
mod contents;
mod file;
mod highlight;
mod palette;
mod panel;
mod reveal;
mod search;
mod window;

use clipboard::*;
use contents::*;
use file::*;
use highlight::*;
use palette::*;
use panel::*;
use reveal::*;
use search::*;
use window::*;

// What the rest of the app reaches for by name, which is what the menus and
// the content's own context menu need.
pub(crate) use clipboard::{copy_image_from_src, copy_rasterized_image};
#[cfg(target_os = "macos")]
pub(crate) use file::{pick_directory, pick_markdown_file};
pub(crate) use highlight::highlight_selection;
pub(crate) use palette::activate_palette_row;
pub(crate) use reveal::content_cursor_eval;

/// Where Help → Go to Homepage leads.
const HOMEPAGE_URL: &str = env!("CARGO_PKG_REPOSITORY");

pub(crate) fn open_homepage() {
    let _ = open::that(HOMEPAGE_URL);
}

/// A new window is a fresh start: the places, the welcome page, and nothing
/// the window it was asked from happened to have wandered into.
fn open_new_window() {
    crate::window::create_main_window_sync(
        &dioxus::desktop::window(),
        crate::state::Document::default(),
        crate::window::CreateMainWindowConfigParams::default(),
    );
}

/// What `action` does when it acts on no window's state, or [`None`] when it
/// needs one.
///
/// The native menu runs these even with no window open, where there is no
/// state to hand [`dispatch_action`].
pub fn stateless_action(action: &Action) -> Option<fn()> {
    Some(match action {
        Action::WindowNew => open_new_window,
        Action::WindowCloseAllChildWindows => crate::window::close_child_windows_for_last_focused,
        Action::WindowCloseAllWindows => crate::window::close_all_main_windows,
        Action::AppGoToHomepage => open_homepage,
        _ => return None,
    })
}

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
    if let Some(run) = stateless_action(action) {
        return run();
    }
    match action {
        Action::ScrollDown => scroll_eval("down"),
        Action::ScrollUp => scroll_eval("up"),
        Action::ScrollPageDown => scroll_eval("pageDown"),
        Action::ScrollPageUp => scroll_eval("pageUp"),
        Action::ScrollHalfPageDown => scroll_eval("halfPageDown"),
        Action::ScrollHalfPageUp => scroll_eval("halfPageUp"),
        Action::ScrollTop => scroll_eval("toTop"),
        Action::ScrollBottom => scroll_eval("toBottom"),

        Action::HistoryBack => {
            state.save_scroll_and_go_back();
        }
        Action::HistoryForward => {
            state.save_scroll_and_go_forward();
        }

        Action::SearchOpen => search_open(&mut state),
        Action::SearchNext => search_navigate_eval("next"),
        Action::SearchPrev => search_navigate_eval("prev"),
        Action::SearchClear => state.close_search(),
        Action::SearchPinCurrent => search_pin_current(&mut state),

        Action::HighlightAdd => highlight_selection(&state, crate::highlights::last_color(), false),
        Action::HighlightRemove => remove_highlights_at_selection(&state),
        Action::HighlightNote => highlight_with_note(&state),

        Action::ZoomIn => state.zoom_in(),
        Action::ZoomOut => state.zoom_out(),
        Action::ZoomReset => state.zoom_reset(),

        Action::WindowDuplicate => duplicate_window(&mut state),
        // Putting the document down is what "new" means for a window that
        // reads one: the welcome page takes its place, offering the next.
        Action::WindowNewDocument => new_document(&mut state),
        Action::WindowClose => close_window(),
        // Closing the panel hands the keyboard back to the document; that
        // belongs to `hide_panel`, so both the rail and this go through it.
        Action::WindowToggleSidebar => state.toggle_sidebar(),
        Action::WindowToggleFocusMode => state.toggle_focus_mode(),
        Action::WindowReload => reload(&mut state),

        Action::CopyFilePath => copy_current_file_path(&state),
        Action::CopyFilePathWithLine => copy_file_path_with_line(&state, false),
        Action::CopyFilePathWithRange => copy_file_path_with_line(&state, true),
        Action::CopyCode => copy_content_cursor_text("getCodeText"),
        Action::CopyCodeAsMarkdown => copy_content_cursor_text("getCodeAsMarkdown"),
        Action::CopyTableAsTsv => copy_content_cursor_text("getTableAsTsv"),
        Action::CopyTableAsCsv => copy_content_cursor_text("getTableAsCsv"),
        Action::CopyTableAsMarkdown => copy_content_cursor_text("getTableAsMarkdown"),
        Action::CopyImageAsMarkdown => copy_content_cursor_text("getImageAsMarkdown"),
        Action::CopyImage => copy_image_from_cursor(false),
        Action::CopyImageWithBackground => copy_image_from_cursor(true),
        Action::CopyImagePath => copy_image_path_from_cursor(),
        Action::CopyAsMarkdown => copy_markdown_source(&state),
        Action::CopyLinkPath => copy_link_path_from_cursor(),

        // One per face: the keyboard goes to the list that was asked for,
        // rather than to whichever the panel happened to be showing.
        Action::FocusPlaces => face_to(&mut state, crate::state::Face::Places),
        Action::FocusRecent => face_to(&mut state, crate::state::Face::Recent),
        Action::FocusStarred => face_to(&mut state, crate::state::Face::Starred),
        Action::FocusLinks => face_to(&mut state, crate::state::Face::Links),
        Action::FocusContent => state.focus_content(),

        Action::CursorDown => dispatch_cursor_move(&mut state, CursorDirection::Down),
        Action::CursorUp => dispatch_cursor_move(&mut state, CursorDirection::Up),
        Action::CursorEnter => dispatch_cursor_enter(&mut state),
        Action::CursorOpen => dispatch_cursor_open(&mut state),
        Action::CursorCollapse => dispatch_cursor_collapse(&mut state),

        // The engine only lets these through in the Content context.
        Action::ContentNext => content_cursor_eval("next"),
        Action::ContentPrev => content_cursor_eval("prev"),
        Action::ContentNextHeading => content_cursor_eval("nextHeading"),
        Action::ContentPrevHeading => content_cursor_eval("prevHeading"),
        Action::ContentOpenViewer => open_content_viewer_from_cursor(&state),

        Action::DirectoryParent => add_parent_of_primary_root(&mut state),

        Action::PaletteOpen => state.toggle_palette(),
        Action::PaletteNext => step_palette(&mut state, true),
        Action::PalettePrev => step_palette(&mut state, false),
        Action::PaletteConfirm => confirm_palette(state),
        Action::PaletteClose => state.close_palette(),

        Action::ContentsToggle => state.toggle_contents(),
        Action::LensStop => crate::lenses::stop(state),
        Action::LensHide => crate::lenses::hide(state),
        Action::Lens(place) => crate::lenses::toggle(state, usize::from(*place)),
        Action::ContentsNext => step_contents(&mut state, true),
        Action::ContentsPrev => step_contents(&mut state, false),
        Action::ContentsConfirm => confirm_contents(&mut state),
        Action::ContentsClose => state.close_contents(),

        Action::ChangesNext => changes_eval("next"),
        Action::ChangesPrev => changes_eval("prev"),
        Action::ChangesMarkRead => state.mark_changes_read(),

        Action::FileOpen => open_picked_file(&mut state),
        Action::FileOpenDirectory => add_picked_directory(&mut state),
        Action::FileSetParentAsRoot => set_parent_of_current_file_as_root(&mut state),
        Action::FileToggleBookmark => toggle_bookmark_on_cursor_or_current(&mut state),
        Action::FileOpenLink => open_link_from_cursor(&mut state, false),
        Action::FileOpenLinkInNewWindow => open_link_from_cursor(&mut state, true),
        Action::FilePreviewLink => preview_link_at_cursor(),
        Action::FileSaveImageAs => save_image_from_cursor(),
        Action::FilePreferences => state.open_preferences(),
        Action::FileRevealInFinder => reveal_current_in_finder(&state),
        Action::FilePrint => print_current(&state),

        Action::AppAbout => open_about(&mut state),
        Action::AppQuit => close_window(),
        // Both are answered before dispatch, in `keybinding_engine`: they act
        // on the overlay, which is the window's own state and not the
        // document's.
        Action::HelpShowKeyboardShortcuts => {}

        Action::SidebarFacePlaces => face_to(&mut state, crate::state::Face::Places),
        Action::SidebarFaceRecent => face_to(&mut state, crate::state::Face::Recent),
        Action::SidebarFaceStarred => face_to(&mut state, crate::state::Face::Starred),
        Action::SidebarFaceLinks => face_to(&mut state, crate::state::Face::Links),
        Action::SidebarFaceNext => step_face(&mut state, true),
        Action::SidebarFacePrev => step_face(&mut state, false),
        Action::SidebarToggleShowAllFiles => toggle_show_all_files(&mut state),

        Action::ThemeSetLight => state.current_theme.set(Theme::Light),
        Action::ThemeSetDark => state.current_theme.set(Theme::Dark),
        Action::ThemeSetAuto => state.current_theme.set(Theme::Auto),

        Action::Cancel => {}

        // Run before the match, by `stateless_action`.
        Action::WindowNew
        | Action::WindowCloseAllChildWindows
        | Action::WindowCloseAllWindows
        | Action::AppGoToHomepage => {}
    }
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
