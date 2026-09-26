//! What changed in the document since it was last read.

use crate::baselines::{self, Moment};
use crate::config::CONFIG;
use crate::state::AppState;
use crate::utils::task::spawn_detached;
use dioxus::prelude::*;

impl AppState {
    /// Take the version on screen as read, at `moment`.
    ///
    /// The version is the one the page was drawn from, not what the file
    /// holds by now: what changed on disk since the last draw has not been
    /// seen.
    pub fn keep_read_version(&self, moment: Moment) {
        if !CONFIG.read().reading.show_changes || !baselines::advances(moment, true) {
            return;
        }
        let Some(rendered) = self.rendered_source.peek().clone() else {
            return;
        };
        baselines::advance(&rendered.path, &rendered.source);
    }

    /// Take everything on screen as read, and clear the marks.
    pub fn mark_changes_read(&mut self) {
        self.keep_read_version(Moment::MarkedRead);
        self.changes.set(Vec::new());
        spawn_detached(async {
            let _ = document::eval("window.Arto?.changes?.clear?.()").await;
        });
    }
}
