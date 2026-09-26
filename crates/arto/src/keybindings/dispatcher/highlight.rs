//! Highlighting the selection, and taking highlights off it.
//!
//! The page names what is selected (`frontend/src/user-highlights.ts`);
//! what is kept is `crate::highlights`, which tells every window showing
//! the document to draw it again.

use dioxus::document;

use super::*;
use crate::highlights::page::doc_name;
use crate::highlights::{HighlightColor, HighlightId, TextAnchor};

/// A place the page says is selected, in the document it has drawn.
#[derive(serde::Deserialize)]
struct Selected {
    doc: Option<String>,
    anchor: TextAnchor,
}

/// Highlight the selection in `color`.
///
/// From the context menu, picking the item may already have taken the
/// selection away, so the one the menu opened on stands in for it; from the
/// keyboard only a selection still there counts.
pub(crate) fn highlight_selection(state: &AppState, color: HighlightColor, from_menu: bool) {
    // Plain text is one code block, and a code block is not marked.
    if state.rendered_source.peek().is_none() {
        return;
    }
    let Some(file) = state.current_file() else {
        return;
    };
    let describe = if from_menu {
        "describeMenuSelection"
    } else {
        "describeSelection"
    };
    spawn_detached(async move {
        let js = format!("dioxus.send(window.Arto?.highlights?.{describe}?.() ?? null)");
        match document::eval(&js).recv::<Option<Selected>>().await {
            // Taken from a page still showing the document before this one:
            // its words belong there, not here.
            Ok(Some(selected)) if selected.doc.as_deref() != Some(doc_name(&file).as_str()) => {
                tracing::debug!("The selection was made in a document no longer shown");
            }
            Ok(Some(Selected { anchor, .. })) => {
                crate::highlights::add(&file, anchor, color);
                // The highlight is what shows the words were taken; a
                // selection left over them would hide it.
                let _ = document::eval("window.getSelection()?.removeAllRanges();").await;
            }
            Ok(None) => show_action_feedback("Select text to highlight"),
            Err(error) => tracing::debug!(%error, "The selection was not described"),
        }
    });
}

/// Take away the highlights the selection touches.
pub(super) fn remove_highlights_at_selection(state: &AppState) {
    let Some(file) = state.current_file() else {
        return;
    };
    spawn_detached(async move {
        let js = "dioxus.send(window.Arto?.highlights?.idsAtSelection?.() ?? [])";
        match document::eval(js).recv::<Vec<HighlightId>>().await {
            Ok(ids) if !ids.is_empty() => crate::highlights::remove_highlights(&file, &ids),
            Ok(_) => {}
            Err(error) => tracing::debug!(%error, "The highlights selected were not found"),
        }
    });
}
