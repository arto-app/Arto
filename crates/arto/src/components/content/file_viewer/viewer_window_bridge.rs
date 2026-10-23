//! The page asking for a diagram, a formula or an image in a window of its own.

use dioxus::prelude::*;
use serde::Deserialize;

use crate::state::AppState;

/// What the page's `handle*WindowOpen` globals send.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "type")]
enum ViewerWindowRequest {
    #[serde(rename = "open_mermaid_window")]
    Mermaid { source: String },
    #[serde(rename = "open_math_window")]
    Math { source: String },
    #[serde(rename = "open_image_window")]
    Image { src: String, alt: Option<String> },
}

/// A message the page sent, or `None` when it is not one of the requests.
///
/// Received as a `Value` and decoded here rather than by `recv`, because a
/// message `recv` cannot decode ends the receive loop for every later one.
fn decode(message: serde_json::Value) -> Option<ViewerWindowRequest> {
    serde_json::from_value(message)
        .inspect_err(|err| tracing::debug!(%err, "Ignoring viewer window message"))
        .ok()
}

/// Register the globals the page calls to open a viewer window, and open the
/// window it asks for in the current theme.
pub(super) fn use_viewer_window_bridge(state: AppState) {
    use_effect(move || {
        let mut eval_provider = document::eval(indoc::indoc! {r#"
            window.handleMermaidWindowOpen = (source) => {
                dioxus.send({ type: "open_mermaid_window", source: source });
            };
            window.handleMathWindowOpen = (source) => {
                dioxus.send({ type: "open_math_window", source: source });
            };
            window.handleImageWindowOpen = (src, alt) => {
                dioxus.send({ type: "open_image_window", src: src, alt: alt });
            };
        "#});

        spawn(async move {
            while let Ok(message) = eval_provider.recv::<serde_json::Value>().await {
                let Some(request) = decode(message) else {
                    continue;
                };
                let theme = *state.current_theme.peek();
                match request {
                    ViewerWindowRequest::Mermaid { source } => {
                        tracing::info!("Opening mermaid window for diagram");
                        crate::window::open_or_focus_mermaid_window(source, theme);
                    }
                    ViewerWindowRequest::Math { source } => {
                        tracing::info!("Opening math window for LaTeX");
                        crate::window::open_or_focus_math_window(source, theme);
                    }
                    ViewerWindowRequest::Image { src, alt } => {
                        tracing::info!("Opening image window");
                        crate::window::open_or_focus_image_window(src, alt, theme);
                    }
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_mermaid_request_carries_its_source() {
        let request = decode(json!({ "type": "open_mermaid_window", "source": "graph TD; A-->B" }));

        assert_eq!(
            request,
            Some(ViewerWindowRequest::Mermaid {
                source: "graph TD; A-->B".to_string(),
            })
        );
    }

    #[test]
    fn a_math_request_carries_its_source() {
        let request = decode(json!({ "type": "open_math_window", "source": "e^{i\\pi}" }));

        assert_eq!(
            request,
            Some(ViewerWindowRequest::Math {
                source: "e^{i\\pi}".to_string(),
            })
        );
    }

    #[test]
    fn an_image_request_carries_its_src_and_alt() {
        let request = decode(json!({ "type": "open_image_window", "src": "a.png", "alt": "A" }));

        assert_eq!(
            request,
            Some(ViewerWindowRequest::Image {
                src: "a.png".to_string(),
                alt: Some("A".to_string()),
            })
        );
    }

    #[test]
    fn an_image_without_alt_text_has_none() {
        let null_alt = decode(json!({ "type": "open_image_window", "src": "a.png", "alt": null }));
        let missing_alt = decode(json!({ "type": "open_image_window", "src": "a.png" }));

        let expected = Some(ViewerWindowRequest::Image {
            src: "a.png".to_string(),
            alt: None,
        });
        assert_eq!(null_alt, expected);
        assert_eq!(missing_alt, expected);
    }

    #[test]
    fn a_request_without_its_source_is_ignored() {
        assert_eq!(decode(json!({ "type": "open_mermaid_window" })), None);
        assert_eq!(
            decode(json!({ "type": "open_image_window", "alt": "A" })),
            None
        );
    }

    #[test]
    fn an_unknown_request_is_ignored() {
        assert_eq!(
            decode(json!({ "type": "open_elsewhere", "source": "x" })),
            None
        );
        assert_eq!(decode(json!("open_mermaid_window")), None);
    }
}
