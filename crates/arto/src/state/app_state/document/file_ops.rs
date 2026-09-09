//! Opening a document, and the one errand that is not a document.

use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

impl AppState {
    /// Read a file in this window.
    ///
    /// Opening the document already on screen is not a move, so it does
    /// nothing: the window would otherwise gain a history entry for standing
    /// still.
    pub fn open_file(&mut self, file: impl AsRef<Path>) {
        // Folded here rather than at each store's own door, so that the
        // window and the lists beside it are all talking about the same
        // document. A path typed or dropped in the wrong case opens the right
        // file on a case-insensitive disk, and every list that asks "is this
        // the one on screen?" compares strings.
        let file = crate::utils::paths::true_spelling(file.as_ref());
        let file = file.as_path();
        self.reveal_in_roots(file);
        if self.current_file().as_deref() != Some(file) {
            self.update_document(|document| document.navigate_to(file));
        }
    }

    /// Follow a link inside the document being read.
    ///
    /// The same act as [`Self::open_file`] now that a window reads one
    /// document; it stays a separate name because the callers mean different
    /// things by it, and the scroll position it leaves behind is saved by
    /// the history rather than here.
    pub fn navigate_to_file(&mut self, file: impl Into<PathBuf>) {
        let file = file.into();
        self.reveal_in_roots(&file);
        self.update_document(|document| document.navigate_to(file));
    }

    /// Open the preferences window, or focus it if it is already open.
    ///
    /// Preferences used to be a tab, which gave a short errand the lifetime of
    /// a document and left it sitting open. It is a window now; what the
    /// "Current Settings" section needs from this window is handed over as a
    /// snapshot, since the preferences window has no `AppState` of its own.
    pub fn open_preferences(&mut self) {
        let (sidebar_width, sidebar_zoom_level, directory) = {
            let sidebar = self.sidebar.read();
            (
                sidebar.width,
                sidebar.zoom_level,
                sidebar.primary_root().cloned(),
            )
        };
        let (right_sidebar_width, right_sidebar_zoom_level) = {
            let right = self.right_sidebar.read();
            (right.width, right.zoom_level)
        };
        crate::window::preferences::open_or_focus_preferences_window(
            crate::window::preferences::PreferencesSnapshot {
                window_id: dioxus::desktop::window().id(),
                sidebar_width,
                sidebar_zoom_level,
                right_sidebar_width,
                right_sidebar_zoom_level,
                directory,
            },
            *self.current_theme.read(),
        );
    }
}
