//! Per-tab AI view overlay.
//!
//! When an AI provider with `AiAction::View` is invoked, its streaming output
//! is captured here and rendered in place of the tab's normal content. The
//! original content is preserved so the user can revert at any time.

/// One AI view override entry, keyed by tab index in [`AppState::ai_overlays`].
#[derive(Debug, Clone, PartialEq)]
pub struct AiOverlay {
    /// Provider id that produced this override (used to display attribution).
    pub provider_id: String,
    /// Display name of the provider at invocation time. Stored alongside the
    /// id so the badge stays correct even if the provider is renamed mid-run.
    pub provider_name: String,
    /// Accumulated raw Markdown produced by the AI so far.
    pub markdown: String,
    /// Whether the stream is still receiving deltas. Toggles to `false` on
    /// `StreamEvent::Done` or `StreamEvent::Error`.
    pub streaming: bool,
    /// Error message attached when the stream ended with [`StreamEvent::Error`].
    pub error: Option<String>,
}

impl AiOverlay {
    pub fn new(provider_id: impl Into<String>, provider_name: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            provider_name: provider_name.into(),
            markdown: String::new(),
            streaming: true,
            error: None,
        }
    }

    pub fn append_delta(&mut self, delta: &str) {
        self.markdown.push_str(delta);
    }

    pub fn finish_ok(&mut self) {
        self.streaming = false;
    }

    pub fn finish_err(&mut self, message: impl Into<String>) {
        self.streaming = false;
        self.error = Some(message.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_overlay_starts_streaming_with_empty_markdown() {
        let o = AiOverlay::new("p1", "Translate");
        assert_eq!(o.provider_id, "p1");
        assert_eq!(o.provider_name, "Translate");
        assert!(o.markdown.is_empty());
        assert!(o.streaming);
        assert!(o.error.is_none());
    }

    #[test]
    fn append_delta_concatenates() {
        let mut o = AiOverlay::new("p", "n");
        o.append_delta("hello ");
        o.append_delta("world");
        assert_eq!(o.markdown, "hello world");
    }

    #[test]
    fn finish_ok_clears_streaming_without_error() {
        let mut o = AiOverlay::new("p", "n");
        o.finish_ok();
        assert!(!o.streaming);
        assert!(o.error.is_none());
    }

    #[test]
    fn finish_err_records_message() {
        let mut o = AiOverlay::new("p", "n");
        o.finish_err("boom");
        assert!(!o.streaming);
        assert_eq!(o.error.as_deref(), Some("boom"));
    }
}
