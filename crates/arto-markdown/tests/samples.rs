//! Rendering snapshots of the sample documents.
//!
//! Every numbered file under `samples/` is rendered through the public
//! pipeline and compared with the stored HTML under `tests/snapshots/`. A
//! change in output fails here with a diff, so a parser upgrade or an engine
//! swap shows exactly which constructs moved. Review and accept intended
//! changes with `cargo insta review` (the devShell ships `cargo-insta`).
//!
//! The HTML contract those snapshots pin down is documented in the crate
//! docs of `arto-markdown`.

use arto_markdown::{render_to_html, render_to_html_with_toc, HeadingInfo, RenderOptions};

#[test]
fn samples_render_as_before() {
    // The base is resolved relative to this file's directory.
    insta::glob!("../../../samples", "[0-9]*.md", |path| {
        let markdown = std::fs::read_to_string(path).expect("sample is readable");
        let html = render_to_html(&markdown, path, &RenderOptions::default())
            .unwrap_or_else(|err| panic!("{}: {err:#}", path.display()));
        insta::assert_snapshot!(html);
    });
}

/// The table of contents: heading levels, texts and the ids that the
/// rendered headings carry. Rendering with a TOC differs from plain
/// rendering only by `id` attributes, which is asserted here rather than
/// stored as a second copy of the HTML.
#[test]
fn samples_headings_as_before() {
    insta::glob!("../../../samples", "[0-9]*.md", |path| {
        let markdown = std::fs::read_to_string(path).expect("sample is readable");
        let options = RenderOptions::default();
        let plain = render_to_html(&markdown, path, &options)
            .unwrap_or_else(|err| panic!("{}: {err:#}", path.display()));
        let (with_toc, headings) = render_to_html_with_toc(&markdown, path, &options)
            .unwrap_or_else(|err| panic!("{}: {err:#}", path.display()));

        assert_eq!(
            strip_ids(&with_toc),
            strip_ids(&plain),
            "{}: TOC rendering must differ only by id attributes",
            path.display()
        );
        for heading in &headings {
            assert!(
                heading.id.is_empty() || with_toc.contains(&format!(r#" id="{}""#, heading.id)),
                "{}: heading id {:?} is missing from the rendered HTML",
                path.display(),
                heading.id
            );
        }

        insta::assert_snapshot!(outline(&headings));
    });
}

fn outline(headings: &[HeadingInfo]) -> String {
    headings
        .iter()
        .map(|h| format!("{} {:?} #{}", h.level, h.text, h.id))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove every ` id="…"` attribute.
fn strip_ids(html: &str) -> String {
    let mut rest = html;
    let mut out = String::with_capacity(html.len());
    while let Some(pos) = rest.find(" id=\"") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 5..];
        let end = after.find('"').expect("id attribute is closed");
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
