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
use std::path::Path;

/// `samples/stress.md` is a megabyte of generated Markdown, kept out of the
/// numbered set so it is not snapshotted — the snapshot would be larger than
/// the rest of the repository put together. It is here to render, and to be
/// the document to open when a change is meant to be felt rather than read.
#[test]
fn the_stress_sample_renders() {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../samples/stress.md"
    ));
    let markdown = std::fs::read_to_string(path).expect("sample is readable");

    let (html, headings) = render_to_html_with_toc(&markdown, path, &RenderOptions::default())
        .unwrap_or_else(|err| panic!("{}: {err:#}", path.display()));

    assert!(headings.len() > 100, "{} headings", headings.len());
    assert!(
        headings.iter().all(|heading| !heading.id.is_empty()),
        "every heading needs an anchor for the table of contents"
    );
    // Nothing from the engine's own vocabulary may reach the output.
    assert!(!html.contains("data-source-span"), "spans survived");
    assert!(!html.contains("ox-callout"), "callout classes survived");
}

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
