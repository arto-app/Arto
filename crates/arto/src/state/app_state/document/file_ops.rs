//! Opening a document, and the one errand that is not a document.

use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

impl AppState {
    /// Read a file in this window, at the place it was left.
    ///
    /// Opening the document already on screen is not a move, so it does
    /// nothing beyond noting the visit: the window would otherwise gain a
    /// history entry for standing still.
    ///
    /// A document that has been read before opens where the reader had got
    /// to. Coming back to a long document at the top of it is being made to
    /// find your place again every time, and the history is already keeping
    /// the answer.
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
            self.keep_reading_position();
            let resume = crate::visits::position(file).filter(|anchor| !anchor.is_top());
            self.pending_scroll_anchor.set(resume);
            self.update_document(|document| document.navigate_to(file));
        }
        // Recorded once the window is already showing it. Every window on the
        // history reads it against the document on screen — the trace leaves
        // out what is being read, the palette starts on the one before it —
        // so announcing the visit first would redraw them all around a
        // document that is not the current one yet.
        self.record_visit(file);
    }

    /// Note a visit, and tell this window's own lists in the same breath.
    ///
    /// The history announces itself over a broadcast that every window hears,
    /// this one included — but a poll of the runtime later, which is a frame
    /// in which this window is showing a document its own lists have not heard
    /// about. The signal closes that gap; the broadcast still carries it to
    /// the other windows.
    pub fn record_visit(&mut self, file: impl AsRef<Path>) {
        crate::visits::record_visit(file.as_ref());
        *self.visits_revision.write() += 1;
    }

    /// Hand the document on screen's place to the history, so that leaving it
    /// is what saves it.
    ///
    /// The position is noted in memory as the page scrolls; this is where it
    /// is put on the row it belongs to before the row stops being the current
    /// one. The write to disk comes with the visit that follows.
    pub fn keep_reading_position(&mut self) {
        let Some(file) = self.current_file() else {
            return;
        };
        crate::visits::keep_position(&file, *self.current_scroll_anchor.read());
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
        self.update_document(|document| document.navigate_to(file.clone()));
        self.record_visit(&file);
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
        crate::window::preferences::open_or_focus_preferences_window(
            crate::window::preferences::PreferencesSnapshot {
                window_id: dioxus::desktop::window().id(),
                sidebar_width,
                sidebar_zoom_level,
                directory,
            },
            *self.current_theme.read(),
        );
    }
}
