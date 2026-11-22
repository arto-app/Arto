//! What the renderer Arto depends on makes of the sample documents.
//!
//! `arto-markdown` pins its own output in snapshots of its own, but those
//! travel with the renderer wherever it is developed. These are the app's
//! record: the HTML each numbered sample under `samples/` renders to, and the
//! library bundles a page showing it loads, as the version of the renderer
//! this workspace builds against produces them. A change of that version
//! that changes either fails here with a diff, so the app learns of it before
//! a reader does. Review and accept intended changes with `cargo insta
//! review`.

use arto_markdown::frontend::{Highlighting, Libraries};
use arto_markdown::{render_to_html, RenderOptions};

/// The libraries a page loads, one name per line, so a change reads as a
/// change of name rather than of a struct's debug output.
fn libraries(fragment: &str) -> String {
    let Libraries {
        mermaid,
        math,
        highlighting,
    } = Libraries::for_fragment(fragment);
    let highlighting = match highlighting {
        Highlighting::None => None,
        Highlighting::Common => Some("highlight.js (common)"),
        Highlighting::Every => Some("highlight.js (all)"),
    };
    [
        mermaid.then_some("mermaid"),
        math.then_some("katex"),
        highlighting,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn the_renderer_draws_the_samples_as_recorded() {
    insta::glob!("../../../samples", "[0-9]*.md", |path| {
        let markdown = std::fs::read_to_string(path).expect("sample is readable");
        let html = render_to_html(&markdown, path, &RenderOptions::default())
            .unwrap_or_else(|err| panic!("{}: {err:#}", path.display()))
            .html;
        insta::assert_snapshot!(format!("{}\n---\n{html}", libraries(&html)));
    });
}
