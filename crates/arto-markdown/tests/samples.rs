//! Rendering snapshots of the sample documents.
//!
//! Every numbered file under `samples/` is rendered through the public
//! pipeline and compared with the stored HTML under `tests/snapshots/`. A
//! change in output fails here with a diff, so a parser upgrade or an engine
//! swap shows exactly which constructs moved. Review and accept intended
//! changes with `cargo insta review` (the devShell ships `cargo-insta`).

use arto_markdown::{render_to_html, RenderOptions};

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
