use dioxus::prelude::*;

use crate::config::{ReadingConfig, CONFIG};
use crate::markdown::ReadingProfile;
use crate::reading_time::{estimate, Estimate};
use crate::scroll_anchor::ScrollAnchor;
use crate::state::AppState;

/// How long the document takes to read, or how long is left, in the header.
///
/// Text rather than a control: there is nothing to do with it but read it.
/// This is the one component that reads the scroll anchor, which changes on
/// every frame of a scroll; the text is worked out in a memo, so the page
/// only changes when the minutes do, and the header around it not at all.
#[component]
pub fn ReadingTime() -> Element {
    let state = use_context::<AppState>();
    let shown = use_memo(move || {
        // Subscribes to configuration changes; the value itself says nothing.
        let _ = state.config_revision.read();
        let config = CONFIG.read().reading.clone();
        let profile = state.reading_profile.read().clone();
        let page_lens_applied = state.page_lenses.read().iter().any(|run| run.applied);
        visible_estimate(
            profile.as_deref(),
            page_lens_applied,
            *state.current_scroll_anchor.read(),
            *state.scrolled_to_end.read(),
            &config,
        )
    });

    let Some(shown) = shown() else {
        return rsx! {};
    };
    rsx! {
        span {
            class: "reading-time",
            title: "{shown.title}",
            "{shown.label}"
        }
    }
}

/// The estimate to show, if any.
///
/// A page lens puts something else in the document's place — a translation
/// reads at its own length — so while one is applied the document's own
/// estimate would describe text that is not on screen.
fn visible_estimate(
    profile: Option<&ReadingProfile>,
    page_lens_applied: bool,
    anchor: ScrollAnchor,
    at_end: bool,
    config: &ReadingConfig,
) -> Option<Estimate> {
    if page_lens_applied {
        return None;
    }
    estimate(profile?, anchor, at_end, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::ReadingBlock;

    fn long_document() -> ReadingProfile {
        ReadingProfile {
            blocks: vec![ReadingBlock {
                line: 1,
                words: 10_000,
                ..Default::default()
            }],
        }
    }

    #[test]
    fn a_long_document_shows_its_time() {
        let profile = long_document();
        let shown = visible_estimate(
            Some(&profile),
            false,
            ScrollAnchor::TOP,
            false,
            &ReadingConfig::default(),
        );
        assert_eq!(
            shown.map(|shown| shown.label).as_deref(),
            Some("43 min read")
        );
    }

    #[test]
    fn nothing_rendered_as_markdown_shows_nothing() {
        let shown = visible_estimate(
            None,
            false,
            ScrollAnchor::TOP,
            false,
            &ReadingConfig::default(),
        );
        assert_eq!(shown, None);
    }

    #[test]
    fn an_applied_page_lens_shows_nothing() {
        let profile = long_document();
        let shown = visible_estimate(
            Some(&profile),
            true,
            ScrollAnchor::TOP,
            false,
            &ReadingConfig::default(),
        );
        assert_eq!(shown, None);
    }
}
