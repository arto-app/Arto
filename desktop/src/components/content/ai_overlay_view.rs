//! Renders an [`AiOverlay`] in place of normal tab content.
//!
//! Shows a header bar with the provider name, a Close button to revert to
//! the original document, and a markdown body that streams the assistant
//! response. Errors are rendered as a callout below the partial output.

use dioxus::prelude::*;
use std::path::Path;

use crate::components::icon::{Icon, IconName};
use crate::markdown::render_to_html;
use crate::state::{AiOverlay, AppState, TabId};

#[component]
pub fn AiOverlayView(tab_id: TabId, overlay: AiOverlay) -> Element {
    let mut state = use_context::<AppState>();
    let html = use_signal(String::new);

    use_streaming_renderer(overlay.markdown.clone(), html);

    let close = move |_| {
        state.ai_overlays.write().remove(&tab_id);
    };

    rsx! {
        div {
            class: "ai-overlay",

            div {
                class: "ai-overlay-bar",
                Icon { name: IconName::Sparkles, size: 16 }
                span { class: "ai-overlay-title", "{overlay.provider_name}" }
                if overlay.streaming {
                    span { class: "ai-overlay-status", "Streaming…" }
                }
                button {
                    class: "ai-overlay-close",
                    title: "Restore original",
                    onclick: close,
                    Icon { name: IconName::Close, size: 16 }
                }
            }

            if let Some(error) = overlay.error.as_ref() {
                div { class: "ai-overlay-error", "{error}" }
            }

            div {
                class: "markdown-viewer",
                article {
                    class: "markdown-body",
                    dangerous_inner_html: "{html}"
                }
            }
        }
    }
}

/// Re-render the markdown body whenever new deltas arrive.
fn use_streaming_renderer(markdown: String, html: Signal<String>) {
    use_effect(move || {
        let mut html = html;
        let markdown = markdown.clone();
        spawn(async move {
            // Use a dummy base path — AI output rarely contains relative
            // image references, and even when it does, those resolve against
            // the assistant's view rather than the source file.
            let rendered = render_to_html(&markdown, Path::new(".")).unwrap_or_else(|e| {
                tracing::error!(?e, "failed to render AI overlay markdown");
                format!(r#"<p class="error">Render error: {e}</p>"#)
            });
            html.set(rendered);
        });
    });
}
