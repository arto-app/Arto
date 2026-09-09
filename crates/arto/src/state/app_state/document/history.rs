//! Back, forward, and where on the page the reader was.

use super::DocumentContent;
use crate::scroll_anchor::ScrollAnchor;
use crate::state::AppState;
use dioxus::prelude::*;
use std::path::PathBuf;

impl AppState {
    /// Save current scroll position and go back in history.
    /// Returns true if navigation occurred.
    ///
    /// This is the main entry point for back navigation from UI (menu/header buttons).
    /// It saves the current scroll position before navigating so forward can restore it.
    pub fn save_scroll_and_go_back(&mut self) -> bool {
        let position = *self.current_scroll_anchor.read();
        self.save_current_scroll_anchor(position);
        self.go_back_in_history()
    }

    /// Save current scroll position and go forward in history.
    /// Returns true if navigation occurred.
    ///
    /// This is the main entry point for forward navigation from UI (menu/header buttons).
    pub fn save_scroll_and_go_forward(&mut self) -> bool {
        let position = *self.current_scroll_anchor.read();
        self.save_current_scroll_anchor(position);
        self.go_forward_in_history()
    }

    /// Go back in the document's own history.
    /// Returns true if navigation occurred.
    ///
    /// Note: prefer `save_scroll_and_go_back` for UI-triggered navigation.
    pub fn go_back_in_history(&mut self) -> bool {
        let target = self
            .document
            .write()
            .history
            .go_back()
            .map(|entry| (entry.path.clone(), entry.scroll_anchor));
        self.restore(target, "back")
    }

    /// Go forward in the document's own history.
    /// Returns true if navigation occurred.
    ///
    /// Note: prefer `save_scroll_and_go_forward` for UI-triggered navigation.
    pub fn go_forward_in_history(&mut self) -> bool {
        let target = self
            .document
            .write()
            .history
            .go_forward()
            .map(|entry| (entry.path.clone(), entry.scroll_anchor));
        self.restore(target, "forward")
    }

    /// Show a history entry again, at the place in it the reader had reached.
    ///
    /// The pending anchor is set before the content changes, so the viewer
    /// already knows where to land by the time it loads the file.
    fn restore(&mut self, target: Option<(PathBuf, ScrollAnchor)>, direction: &str) -> bool {
        let Some((path, scroll)) = target else {
            return false;
        };
        tracing::debug!(?path, ?scroll, direction, "Restoring a history entry");
        self.pending_scroll_anchor.set(Some(scroll));
        self.document.write().content = DocumentContent::File(path);
        true
    }

    /// Save the current scroll position to the current history entry.
    ///
    /// Call this before navigating away to preserve scroll position for back/forward.
    pub fn save_current_scroll_anchor(&mut self, scroll: ScrollAnchor) {
        self.update_document(|document| {
            let current_path = document.history.current_path().map(|p| p.to_path_buf());
            tracing::debug!(
                ?current_path,
                ?scroll,
                "Saving scroll position to history entry"
            );
            document.history.save_scroll_anchor(scroll);
        });
    }
}
