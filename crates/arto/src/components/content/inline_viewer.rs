use dioxus::prelude::*;
use std::path::Path;

use crate::markdown::render_to_html;
use crate::state::AppState;

#[component]
pub fn InlineViewer(markdown: String) -> Element {
    let state = use_context::<AppState>();
    let html = use_signal(String::new);

    // Setup component hooks
    use_inline_markdown_loader(markdown, html);

    rsx! {
        div {
            class: "markdown-viewer",
            class: if *state.content_full_width.read() { "full-width" },
            article {
                class: "markdown-body",
                dangerous_inner_html: "{html}"
            }
        }
    }
}

/// Hook to render inline markdown content
fn use_inline_markdown_loader(markdown: String, html: Signal<String>) {
    use_effect(move || {
        let mut html = html;
        let markdown = markdown.clone();

        spawn(async move {
            // Render inline markdown (use a dummy path since images are already embedded)
            let rendered = render_to_html(&markdown, Path::new(".")).unwrap_or_else(|e| {
                tracing::error!("Failed to render inline markdown: {}", e);
                format!(r#"<p class="error">Error rendering markdown: {}</p>"#, e)
            });
            html.set(rendered);
        });
    });
}
