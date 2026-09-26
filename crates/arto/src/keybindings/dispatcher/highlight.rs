//! Highlighting the selection, and taking highlights off it.
//!
//! The page names what is selected (`frontend/src/user-highlights.ts`);
//! what is kept is `crate::highlights`, which tells every window showing
//! the document to draw it again.

use dioxus::document;

use super::*;
use crate::highlights::card::{self, Rect};
use crate::highlights::page::doc_name;
use crate::highlights::{HighlightColor, HighlightId, TextAnchor};

/// A place the page says is selected, in the document it has drawn.
#[derive(serde::Deserialize)]
struct Selected {
    doc: Option<String>,
    anchor: TextAnchor,
    /// The highlight the whole selection already lies in, if any.
    #[serde(default)]
    within: Option<HighlightId>,
    /// Where that highlight, or else the selection, is drawn.
    #[serde(default)]
    rect: Option<Rect>,
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
                if crate::highlights::add(&file, anchor, color).is_none() {
                    show_action_feedback(NOT_KEPT);
                    return;
                }
                // The highlight is what shows the words were taken; a
                // selection left over them would hide it.
                let _ = document::eval("window.getSelection()?.removeAllRanges();").await;
            }
            Ok(None) => show_action_feedback("Select text to highlight"),
            Err(error) => tracing::debug!(%error, "The selection was not described"),
        }
    });
}

/// What the reader is told when a highlight could not be written down.
const NOT_KEPT: &str = "The highlight could not be kept";

/// Highlight the selection in the colour picked last and open the card of
/// the new highlight, to write a note on it. A selection already inside a
/// highlight opens that one's card instead: marking the same words twice is
/// not what a reader asking for a note on them means.
///
/// The card is placed by where the selection was: the new highlight is drawn
/// only once the store has announced it, after the card is already open.
pub(super) fn highlight_with_note(state: &AppState) {
    if state.rendered_source.peek().is_none() {
        return;
    }
    let Some(file) = state.current_file() else {
        return;
    };
    let state = *state;
    spawn_detached(async move {
        let js = "dioxus.send(window.Arto?.highlights?.describeSelection?.() ?? null)";
        match document::eval(js).recv::<Option<Selected>>().await {
            Ok(Some(selected)) if selected.doc.as_deref() != Some(doc_name(&file).as_str()) => {
                tracing::debug!("The selection was made in a document no longer shown");
            }
            Ok(Some(selected)) => {
                let id = match selected.within {
                    Some(id) => id,
                    None => {
                        let color = crate::highlights::last_color();
                        // A card for a highlight that was not kept would take
                        // a note with nowhere to go.
                        let Some(id) = crate::highlights::add(&file, selected.anchor, color) else {
                            show_action_feedback(NOT_KEPT);
                            return;
                        };
                        id
                    }
                };
                let _ = document::eval("window.getSelection()?.removeAllRanges();").await;
                if let Some(rect) = selected.rect {
                    card::open(state, file, id, rect);
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_says_which_highlight_it_is_already_in_and_where() {
        let json = r#"{
            "doc": "/a.md",
            "anchor": {"exact": "x", "prefix": "", "suffix": "", "start": 0, "line": 1},
            "within": "hl_1",
            "rect": {"left": 1, "top": 2, "right": 3, "bottom": 4}
        }"#;
        let selected: Selected = serde_json::from_str(json).unwrap();
        assert_eq!(selected.within, Some(HighlightId::from("hl_1".to_string())));
        assert_eq!(selected.rect.map(|rect| rect.bottom), Some(4.0));

        let bare = r#"{
            "doc": null,
            "anchor": {"exact": "x", "prefix": "", "suffix": "", "start": 0, "line": 1},
            "within": null,
            "rect": null
        }"#;
        let selected: Selected = serde_json::from_str(bare).unwrap();
        assert_eq!(selected.within, None);
        assert!(selected.rect.is_none());
    }
}
