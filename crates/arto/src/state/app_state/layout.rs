//! What the window is wide enough to draw beside the document.
//!
//! The rule and its thresholds live in [`crate::hooks::layout_budget`], which
//! is pure and tested against the specification's table. This is where the
//! window's own numbers are collected and handed to it.

use dioxus::prelude::*;

use crate::config::{RecentTrace, CONFIG};
use crate::hooks::layout_budget::{budget, Visible};
use crate::state::AppState;

impl AppState {
    /// What fits beside the document right now.
    ///
    /// The width is measured after zoom, because magnifying the page is the
    /// same as narrowing the window.
    pub fn visible_chrome(&self) -> Visible {
        let zoom = *self.zoom_level.read();
        let width = self.size.read().width as f64;
        let effective_width = if zoom > 0.0 { width / zoom } else { width };
        let (min_content, panel_width) = {
            let config = CONFIG.read();
            (config.sidebar.min_content_width, self.sidebar.read().width)
        };
        budget(effective_width, min_content, panel_width)
    }

    /// Whether the margin trace is drawn.
    ///
    /// The setting decides first and the width has the last word: a window
    /// too narrow to keep the document readable hides the trace whatever was
    /// chosen, and widening it brings the trace back unchanged.
    pub fn trace_visible(&self) -> bool {
        let chosen = match CONFIG.read().sidebar.recent_trace {
            RecentTrace::Never => false,
            RecentTrace::HiddenWhenSidebar => !self.sidebar.read().pinned,
            RecentTrace::HiddenWhenFullWidth => !*self.content_full_width.read(),
            RecentTrace::Always => true,
        };
        chosen && self.visible_chrome().trace
    }
}
