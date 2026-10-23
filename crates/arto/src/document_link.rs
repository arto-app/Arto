//! Opening a link to another Markdown document, as written in the source
//! (`./other.md#section`), from any entry point: a click on the rendered
//! link, or the keyboard cursor's open-link action.

use std::path::Path;

use dioxus::prelude::*;

use crate::link_target::{resolve_document_link, split_link_fragment};
use crate::scroll_anchor::ScrollAnchor;
use crate::state::AppState;

/// Where a document link opens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkOpen {
    /// Follow it in this window, remembering `scroll_anchor` in the
    /// document's history so that going back lands where the reader was.
    Here { scroll_anchor: ScrollAnchor },
    /// Open it in a window of its own, leaving this one where it is.
    NewWindow,
}

/// Open `link`, relative to `current_file`, and scroll to its fragment once
/// the target has rendered. Returns `false` when the target cannot be
/// resolved, in which case nothing changes.
pub fn open_document_link(
    state: &mut AppState,
    current_file: &Path,
    link: &str,
    how: LinkOpen,
) -> bool {
    let (path, fragment) = split_link_fragment(link);

    let Some(canonical_path) = resolve_document_link(current_file, path) else {
        tracing::warn!(?current_file, path, "Failed to resolve document link");
        return false;
    };

    // A link into the document that is already open does not reload it,
    // so the scroll happens right away instead of after the next render.
    // The open file may have been recorded through a symlink, so compare
    // canonical paths.
    let current_canonical = current_file
        .canonicalize()
        .unwrap_or_else(|_| current_file.to_path_buf());
    if matches!(how, LinkOpen::Here { .. }) && canonical_path == current_canonical {
        if let Some(fragment) = fragment {
            let _ = document::eval(&scroll_to_heading_js(&fragment));
        }
        return true;
    }

    tracing::debug!(path = ?canonical_path, "Opening document link");
    state.pending_scroll_fragment.set(fragment);
    match how {
        LinkOpen::NewWindow => {
            crate::window::create_main_window_sync(
                &dioxus::desktop::window(),
                crate::state::Document::new(canonical_path),
                crate::window::CreateMainWindowConfigParams::default(),
            );
        }
        LinkOpen::Here { scroll_anchor } => {
            state.save_current_scroll_anchor(scroll_anchor);
            state.navigate_to_file(canonical_path);
        }
    }
    true
}

/// JavaScript that scrolls the heading with `id` into view, if it exists.
pub fn scroll_to_heading_js(id: &str) -> String {
    let id_json = serde_json::to_string(id).unwrap_or_else(|_| "null".to_string());
    format!("window.Arto?.scroll?.toHeading?.({id_json});")
}
