//! Focus mode: the window keeps the document and puts everything else away.
//!
//! What goes is the same as what a narrow window folds — the rail, the panel,
//! the contents ruler, the margin trace — so it goes the same way, through
//! [`AppState::visible_chrome`]. None of the reader's choices is written, and
//! leaving brings every one of them back as it was.

use std::path::Path;

use dioxus::prelude::*;

use crate::state::{AppState, FocusedPanel};

impl AppState {
    /// Enter focus mode, or leave it.
    ///
    /// A window with nothing to read has nothing to focus on, so the welcome
    /// page ignores it, as it does the full-width toggle.
    ///
    /// Entering gives the keyboard back to the page and lets a peeking panel
    /// go, along with any menu it opened: the panel is about to be put away,
    /// and keys or a menu left in it would act on rows no one can see.
    pub fn toggle_focus_mode(&mut self) {
        if self.document.read().is_empty() {
            return;
        }
        let on = !*self.focus_mode.read();
        if on {
            self.focus_content();
            self.close_sidebar_context_menu();
            let document = self.document.read().file().map(Path::to_path_buf);
            self.focus_document.set(document);
        }
        self.focus_mode.set(on);
    }

    /// Leave focus mode, if the window is in it.
    pub fn exit_focus_mode(&mut self) {
        if *self.focus_mode.peek() {
            self.focus_mode.set(false);
        }
    }

    /// Leave focus mode once the document it was entered on is no longer the
    /// one being read.
    ///
    /// Focus mode is for reading one document through. Following a link or
    /// going back to another ends that reading, as does putting the document
    /// down or a document failing to load — left on, focus mode would come
    /// back uninvited with the next document, from a welcome page that cannot
    /// turn it off. The same document read again from disk is still that
    /// reading.
    pub fn settle_focus_mode(&mut self) {
        let document = self.document.read();
        let same = !document.is_empty() && document.file() == self.focus_document.peek().as_deref();
        drop(document);
        if !same {
            self.exit_focus_mode();
        }
    }

    /// Whether the document being read is the one focus mode was entered on.
    ///
    /// The reader's place is kept across focus mode changing the layout only
    /// then: when focus mode ends because another document was opened, the
    /// place still held is the last document's, and the new one has its own.
    pub fn focus_mode_document_is_current(&self) -> bool {
        self.document.peek().file() == self.focus_document.peek().as_deref()
    }

    /// Whether the chrome beside the document is put away.
    ///
    /// Only while there is a document: a window that put its document down
    /// shows the welcome page with everything around it, rather than a page
    /// of links with no way to see what the window is in. The frame between
    /// the document going and [`Self::settle_focus_mode`] is covered by this.
    pub fn focusing(&self) -> bool {
        *self.focus_mode.read() && !self.document.read().is_empty()
    }

    /// Whether the header is folded and the page dimmed around the block
    /// being read.
    ///
    /// A search pauses both: the field lives in the header, and a dimmed
    /// page would dim the matches the reader asked to see. The rail and the
    /// panel stay away meanwhile — bringing the panel back would reflow the
    /// page under the highlights being stepped through.
    pub fn focus_dims(&self) -> bool {
        self.focusing() && !*self.search_open.read()
    }

    /// Whether anything is open over the document for Escape to put away.
    ///
    /// The content cursor counts too, but it lives in the page; the key
    /// handler asks the page about it separately.
    pub fn has_overlays(&self) -> bool {
        *self.palette_open.read()
            || *self.contents_open.read()
            || *self.search_open.read()
            || *self.left_hover_active.read()
            || *self.focused_panel.read() != FocusedPanel::Content
    }
}

#[cfg(test)]
mod tests {
    use dioxus::desktop::tao::dpi::LogicalSize;
    use dioxus::prelude::*;

    use crate::state::{AppState, Document, FocusedPanel};

    /// Run `f` against a fresh window state, inside a runtime that can own
    /// its signals.
    fn with_state(f: impl FnOnce(AppState)) {
        fn app() -> Element {
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::APP, || {
            let mut state = AppState::default();
            state.document.set(Document::new("/tmp/focus.md"));
            // Wide enough for everything beside the page at any configured
            // minimum width.
            state.size.set(LogicalSize::new(4000, 1000));
            f(state);
        });
    }

    #[test]
    fn toggling_twice_leaves_the_pinned_panel_as_it_was() {
        with_state(|mut state| {
            state.sidebar.write().pinned = true;

            state.toggle_focus_mode();
            assert!(state.focusing());
            state.toggle_focus_mode();

            assert!(!state.focusing());
            assert!(state.sidebar.read().pinned);
            assert!(state.panel_is_showing());
        });
    }

    #[test]
    fn focusing_puts_away_everything_beside_the_page() {
        with_state(|mut state| {
            state.sidebar.write().pinned = true;
            let before = state.visible_chrome();
            assert!(before.rail && before.panel && before.gutter && before.trace);

            state.toggle_focus_mode();

            let during = state.visible_chrome();
            assert!(!during.rail && !during.panel && !during.gutter && !during.trace);
            assert!(!state.panel_is_showing());
        });
    }

    #[test]
    fn asking_for_the_panel_leaves_focus_mode() {
        with_state(|mut state| {
            state.sidebar.write().pinned = true;
            state.toggle_focus_mode();

            state.toggle_sidebar();

            assert!(!state.focusing());
            assert!(state.panel_is_showing());
        });
    }

    #[test]
    fn entering_takes_the_keyboard_and_a_peek_out_of_the_panel() {
        with_state(|mut state| {
            state.left_hover_active.set(true);
            state.focused_panel.set(FocusedPanel::Panel);

            state.toggle_focus_mode();

            assert!(!*state.left_hover_active.read());
            assert_eq!(*state.focused_panel.read(), FocusedPanel::Content);
            assert!(!state.panel_is_showing());
        });
    }

    #[test]
    fn entering_closes_a_menu_the_panel_opened() {
        use crate::components::sidebar::context_menu::{
            SidebarContextMenuData, SidebarItemKind, SidebarRowRole,
        };
        with_state(|mut state| {
            state
                .sidebar_context_menu
                .set(Some(SidebarContextMenuData::new(
                    (10, 10),
                    (1000, 800),
                    "/tmp/focus.md".into(),
                    SidebarItemKind::File,
                    SidebarRowRole::Entry,
                )));

            state.toggle_focus_mode();

            assert!(state.sidebar_context_menu.read().is_none());
        });
    }

    #[test]
    fn nothing_to_read_ends_focus_mode_rather_than_pausing_it() {
        with_state(|mut state| {
            state.toggle_focus_mode();
            state.document.set(Document::default());

            state.settle_focus_mode();
            state.document.set(Document::new("/tmp/next.md"));

            assert!(!state.focusing());
        });
    }

    #[test]
    fn going_to_another_document_ends_focus_mode() {
        with_state(|mut state| {
            state.toggle_focus_mode();
            state.document.set(Document::new("/tmp/next.md"));

            state.settle_focus_mode();

            assert!(!state.focusing());
        });
    }

    #[test]
    fn the_same_document_read_again_keeps_focus_mode() {
        with_state(|mut state| {
            state.toggle_focus_mode();
            // A reload from disk sets the same document again.
            state.document.set(Document::new("/tmp/focus.md"));

            state.settle_focus_mode();

            assert!(state.focusing());
        });
    }

    #[test]
    fn the_place_to_keep_is_only_in_the_document_focus_mode_was_on() {
        with_state(|mut state| {
            state.toggle_focus_mode();
            assert!(state.focus_mode_document_is_current());

            state.document.set(Document::new("/tmp/next.md"));
            state.settle_focus_mode();

            // The anchor still held is the last document's.
            assert!(!state.focus_mode_document_is_current());
        });
    }

    #[test]
    fn settling_keeps_focus_mode_while_there_is_a_document() {
        with_state(|mut state| {
            state.toggle_focus_mode();

            state.settle_focus_mode();

            assert!(state.focusing());
        });
    }

    #[test]
    fn the_welcome_page_has_nothing_to_focus_on() {
        with_state(|mut state| {
            state.document.set(Document::default());

            state.toggle_focus_mode();

            assert!(!*state.focus_mode.read());
            assert!(!state.focusing());
        });
    }

    #[test]
    fn putting_the_document_down_brings_the_chrome_back() {
        with_state(|mut state| {
            state.toggle_focus_mode();

            state.document.set(Document::default());

            assert!(!state.focusing());
            assert!(state.visible_chrome().rail);
        });
    }

    #[test]
    fn a_search_brings_the_header_back_but_not_the_panel() {
        with_state(|mut state| {
            state.sidebar.write().pinned = true;
            state.toggle_focus_mode();
            assert!(state.focus_dims());

            state.search_open.set(true);

            assert!(!state.focus_dims());
            assert!(state.focusing());
            assert!(!state.visible_chrome().panel);
        });
    }

    #[test]
    fn leaving_when_not_focusing_changes_nothing() {
        with_state(|mut state| {
            state.exit_focus_mode();
            assert!(!state.focusing());
        });
    }

    #[test]
    fn overlays_are_whatever_escape_would_put_away() {
        with_state(|mut state| {
            assert!(!state.has_overlays());

            state.palette_open.set(true);
            assert!(state.has_overlays());
            state.palette_open.set(false);

            state.contents_open.set(true);
            assert!(state.has_overlays());
            state.contents_open.set(false);

            state.search_open.set(true);
            assert!(state.has_overlays());
            state.search_open.set(false);

            state.left_hover_active.set(true);
            assert!(state.has_overlays());
            state.left_hover_active.set(false);

            state.focused_panel.set(FocusedPanel::Panel);
            assert!(state.has_overlays());
        });
    }
}
