//! Files and folders: picking one to open, following a link, revealing and
//! printing the document.

use dioxus::document;

use super::*;
use crate::document_link::{open_document_link, LinkOpen};

pub(super) fn open_picked_file(state: &mut AppState) {
    if let Some(file) = pick_markdown_file() {
        state.open_file(file);
    }
}

pub(super) fn add_picked_directory(state: &mut AppState) {
    if let Some(dir) = pick_directory() {
        state.add_root(dir);
    }
}

pub(super) fn reveal_current_in_finder(state: &AppState) {
    if let Some(file) = get_current_file(state) {
        crate::utils::file_operations::reveal_in_finder(&file);
    }
}

pub(super) fn print_current(state: &AppState) {
    crate::utils::print::print_window(get_current_file(state));
}

pub(crate) fn pick_markdown_file() -> Option<std::path::PathBuf> {
    use rfd::FileDialog;
    FileDialog::new()
        .add_filter("Markdown", crate::utils::file::MARKDOWN_EXTENSIONS)
        .set_directory(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")))
        .pick_file()
}

pub(crate) fn pick_directory() -> Option<std::path::PathBuf> {
    use rfd::FileDialog;
    FileDialog::new()
        .set_directory(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")))
        .pick_folder()
}

/// Preview the link under the keyboard cursor the way a rest of the pointer
/// on it would; the page decides what the link shows.
pub(super) fn preview_link_at_cursor() {
    let _ = document::eval("window.Arto?.linkPreview?.showAtCursor?.();");
}

pub(super) fn open_link_from_cursor(state: &mut AppState, open_in_new_window: bool) {
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
