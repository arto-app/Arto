//! Rendering options

use super::*;

fn render_with(markdown: &str, options: RenderOptions) -> String {
    render_to_html(markdown, Path::new("/nonexistent/test.md"), &options)
        .expect("renders")
        .html
}

#[test]
fn math_off_leaves_the_dollars_to_the_prose() {
    // Two shell variables in one line are what the option is for: the parser
    // pairs their `$`s into a formula spanning the text between them.
    let source = "run echo $HOME/$USER now";
    assert!(
        render(source).contains("preprocessed-math"),
        "the default reads this as a formula"
    );

    let html = render_with(
        source,
        RenderOptions {
            math: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("preprocessed-math"), "{html}");
    assert!(html.contains("$HOME/$USER"), "{html}");
}

#[test]
fn a_bare_url_becomes_a_link_unless_turned_off() {
    let source = "Visit https://example.com for info";
    assert!(
        render(source).contains(r#"<a href="https://example.com">https://example.com</a>"#),
        "the default links a bare URL"
    );

    let html = render_with(
        source,
        RenderOptions {
            auto_link_urls: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<a href"), "{html}");
    assert!(html.contains("https://example.com"), "{html}");
}

#[test]
fn a_bare_url_in_a_code_block_stays_code() {
    let html = render("```\nhttps://example.com\n```\n");

    assert!(!html.contains("<a href"), "{html}");
}

#[test]
fn a_linked_bare_url_leaves_the_lines_around_it_where_they_were() {
    let html = render(indoc! {"
        # Title

        https://example.com

        After URL
    "});

    assert!(html.contains(r#"<h1 data-source-range="1:1-1:7">"#), "{html}");
    assert!(html.contains(r#"<p data-source-range="3:1-3:19">"#), "{html}");
    assert!(html.contains(r#"<p data-source-range="5:1-5:9">"#), "{html}");
}

#[test]
fn wiki_links_off_leave_the_brackets_in_the_text() {
    let source = "see [[Page]] for more";
    assert!(render(source).contains("md-link"), "the default links this");

    let html = render_with(
        source,
        RenderOptions {
            wiki_links: false,
            ..Default::default()
        },
    );
    assert!(html.contains("[[Page]]"), "{html}");
}

#[test]
fn subscript_off_leaves_a_lone_tilde_alone() {
    let source = "about ~5 minutes~ long";
    assert!(render(source).contains("<sub>"), "the default reads this");

    let html = render_with(
        source,
        RenderOptions {
            subscript: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<sub>"), "{html}");
}

#[test]
fn superscript_off_leaves_a_lone_caret_alone() {
    let source = "the 2^nd^ time";
    assert!(render(source).contains("<sup>"), "the default reads this");

    let html = render_with(
        source,
        RenderOptions {
            superscript: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<sup>"), "{html}");
}

#[test]
fn smart_punctuation_off_keeps_the_quotes_as_typed() {
    let source = r#"He said "hello" -- loudly"#;
    assert!(
        render(source).contains('\u{201c}'),
        "the default curls this"
    );

    let html = render_with(
        source,
        RenderOptions {
            smart_punctuation: false,
            ..Default::default()
        },
    );
    assert!(html.contains("&quot;hello&quot;"), "{html}");
}

#[test]
fn cjk_emphasis_off_renders_the_way_github_does() {
    let source = "これは**「重要」**です。";
    assert!(
        render(source).contains("<strong>"),
        "the default pairs this"
    );

    let html = render_with(
        source,
        RenderOptions {
            cjk_emphasis: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<strong>"), "{html}");
}

#[test]
fn definition_lists_off_leave_the_colon_in_the_paragraph() {
    let source = "Term\n\n: The definition\n";
    assert!(render(source).contains("<dl "), "the default reads this");

    let html = render_with(
        source,
        RenderOptions {
            definition_lists: false,
            ..Default::default()
        },
    );
    assert!(!html.contains("<dl>"), "{html}");
}

#[test]
fn heading_attributes_off_leave_the_block_in_the_heading_text() {
    let source = "# Title {#custom}\n";
    assert_eq!(
        headings(source)[0].id,
        "custom",
        "the default takes the id from the block"
    );

    let html = render_with(
        source,
        RenderOptions {
            heading_attributes: false,
            ..Default::default()
        },
    );
    assert!(html.contains("{#custom}"), "{html}");
}

#[test]
fn heading_permalinks_appear_only_when_asked_for() {
    let source = "# Title\n";
    assert!(!render(source).contains("header-anchor"), "off by default");

    let html = render_with(
        source,
        RenderOptions {
            heading_permalinks: true,
            ..Default::default()
        },
    );
    assert!(html.contains(r#"class="header-anchor""#), "{html}");
}

#[test]
fn raw_html_is_filtered_by_default() {
    // The tags that would restyle or script the page around the document are
    // escaped; everything else keeps working.
    let source = "<kbd>Esc</kbd> and <style>p{color:red}</style>\n";

    let filtered = render(source);
    assert!(filtered.contains("<kbd>Esc</kbd>"), "{filtered}");
    assert!(!filtered.contains("<style>"), "{filtered}");

    let allowed = render_with(
        source,
        RenderOptions {
            raw_html: RawHtml::Allow,
            ..Default::default()
        },
    );
    assert!(allowed.contains("<style>"), "{allowed}");

    let escaped = render_with(
        source,
        RenderOptions {
            raw_html: RawHtml::Escape,
            ..Default::default()
        },
    );
    assert!(!escaped.contains("<kbd>"), "{escaped}");
    assert!(escaped.contains("&lt;kbd&gt;"), "{escaped}");
}

#[test]
fn filtering_raw_html_takes_the_script_out_of_it() {
    // The tag filter escapes `<script>`, so a document that wants to run
    // anyway writes it as an attribute instead. Neither spelling may reach the
    // reader under the default setting, which is the one that says the
    // document is not trusted.
    let source = indoc! {r#"
        <img src="nothing.png" onerror="alert(1)">

        <a href="javascript:alert(2)">follow me</a>

        <p ontoggle="alert(3)">text</p>
    "#};

    let filtered = render(source);
    assert!(!filtered.contains("onerror"), "{filtered}");
    assert!(!filtered.contains("ontoggle"), "{filtered}");
    assert!(!filtered.contains("javascript:"), "{filtered}");
    assert!(!filtered.contains("alert"), "{filtered}");

    // What the markup said, minus the script, is still there to read.
    assert!(filtered.contains("follow me"), "{filtered}");
    assert!(filtered.contains("text"), "{filtered}");
}

#[test]
fn a_script_url_spelled_in_entities_is_still_a_script_url() {
    // The attribute reaches the filter as the document spelled it, and the
    // browser is what resolves `&#106;` back into a `j`. A filter reading only
    // what was written would hand the reader a link that runs on click.
    let source = indoc! {r#"
        <a href="&#106;avascript:alert(1)">follow me</a>
    "#};

    let filtered = render(source);
    assert!(!filtered.contains("avascript:"), "{filtered}");
    assert!(filtered.contains("follow me"), "{filtered}");
}

#[test]
fn allowing_raw_html_allows_all_of_it() {
    // `Allow` is the setting for a document the reader trusts, and it has
    // always meant every raw node through untouched. Filtering it would be a
    // different setting wearing its name.
    let source = indoc! {r#"
        <p onclick="alert(1)">text</p>
    "#};

    let allowed = render_with(
        source,
        RenderOptions {
            raw_html: RawHtml::Allow,
            ..Default::default()
        },
    );
    assert!(allowed.contains("onclick"), "{allowed}");
}

#[test]
fn filtering_leaves_an_ordinary_document_whole() {
    // The filter reads every element of every document, so the cost of getting
    // it wrong is paid by markup that was never dangerous.
    let source = indoc! {r#"
        <kbd id="key" class="mod">Cmd</kbd>

        <details><summary>More</summary>Hidden</details>

        [a link](https://example.com)
    "#};

    let filtered = render(source);
    assert!(filtered.contains(r#"id="key""#), "{filtered}");
    assert!(filtered.contains(r#"class="mod""#), "{filtered}");
    assert!(filtered.contains("<details>"), "{filtered}");
    assert!(filtered.contains("https://example.com"), "{filtered}");
}

#[test]
fn the_gfm_baseline_survives_every_option_being_turned_off() {
    // Tables, task lists, strikethrough and footnotes are what a document
    // written for GitHub contains, so nothing a reader can switch may take
    // them away.
    let everything_off = RenderOptions {
        auto_link_urls: false,
        math: false,
        wiki_links: false,
        superscript: false,
        subscript: false,
        definition_lists: false,
        heading_attributes: false,
        smart_punctuation: false,
        cjk_emphasis: false,
        heading_permalinks: false,
        raw_html: RawHtml::Escape,
        ..Default::default()
    };

    let html = render_with(
        indoc! {"
            | a | b |
            | - | - |
            | c | d |

            - [x] done

            ~~gone~~ and a note[^1]

            [^1]: the note
        "},
        everything_off,
    );

    assert!(html.contains("<table"), "{html}");
    assert!(html.contains(r#"type="checkbox""#), "{html}");
    assert!(html.contains("<del>gone</del>"), "{html}");
    assert!(html.contains(r#"class="footnotes""#), "{html}");
}
