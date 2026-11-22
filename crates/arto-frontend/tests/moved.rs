//! The crate is what `arto_markdown::frontend` is, under its old name, so a
//! host that still depends on it is handed the same bundles.

#[test]
fn every_bundle_is_the_one_arto_markdown_carries() {
    use arto_markdown::frontend;

    assert_eq!(arto_frontend::STYLESHEET, frontend::STYLESHEET);
    assert_eq!(arto_frontend::RUNTIME, frontend::RUNTIME);
    assert_eq!(arto_frontend::MERMAID, frontend::MERMAID);
    assert_eq!(arto_frontend::MATH, frontend::MATH);
    assert_eq!(arto_frontend::HLJS_COMMON, frontend::HLJS_COMMON);
    assert_eq!(arto_frontend::HLJS_ALL, frontend::HLJS_ALL);
    assert_eq!(arto_frontend::HTML_EMBED_FRAME, frontend::HTML_EMBED_FRAME);
}

#[test]
fn a_fragment_is_given_the_libraries_arto_markdown_chooses() {
    let fragment =
        r#"<pre class="preprocessed-mermaid" data-original-content="graph TD;">graph TD;</pre>"#;

    let old: Vec<&str> = arto_frontend::Libraries::for_fragment(fragment)
        .scripts()
        .collect();
    let new: Vec<&str> = arto_markdown::frontend::Libraries::for_fragment(fragment)
        .scripts()
        .collect();

    assert_eq!(old, new);
    assert!(arto_frontend::draws_html_embed(
        r#"<pre class="preprocessed-html-embed" data-original-content="&lt;b&gt;">x</pre>"#
    ));
}
