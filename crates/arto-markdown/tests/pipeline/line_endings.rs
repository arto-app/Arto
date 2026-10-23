//! Line endings

use super::*;

#[test]
fn a_crlf_document_renders_like_the_same_document_in_lf() {
    // A Windows checkout hands the reader CRLF. Every construct here is one
    // whose parse depends on where a line ends.
    let lf = indoc! {"
        Paragraph before the rule.

        ---

        # Heading

        - item one
        - item two

        ```rust
        fn main() {}
        ```

        > [!NOTE]
        > A note body.
    "};
    let crlf = lf.replace('\n', "\r\n");
    let cr = lf.replace('\n', "\r");

    assert_eq!(render(&crlf), render(lf));
    assert_eq!(render(&cr), render(lf));
    // The rule must stay a rule: read as a setext underline it would close
    // the paragraph above it as a heading instead.
    assert!(render(&crlf).contains("<hr data-source-range=\"3:1-3:3\">"));
}

#[test]
fn a_crlf_math_block_hands_the_frontend_the_same_source_as_lf() {
    // `data-original-content` is what the client-side renderer is given,
    // and the parser reports a math block's value with the `\r` still in it.
    let lf = "$$\nx = 1\n$$\n";

    assert_eq!(render(&lf.replace('\n', "\r\n")), render(lf));
}

#[test]
fn a_crlf_selection_maps_back_to_its_source() {
    use arto_markdown::extract_source_selection;

    assert_eq!(
        extract_source_selection(
            "intro\r\n\r\nsome **word** here\r\n",
            "word",
            &RenderOptions::default()
        ),
        Some("**word**".to_string())
    );
}

#[test]
fn a_crlf_selection_inside_a_code_block_maps_back_too() {
    use arto_markdown::extract_source_selection;

    // The parser reports a code block's value with the `\r` taken out, so a
    // map built over the CRLF source would not find it and would lose the
    // whole block.
    assert_eq!(
        extract_source_selection(
            "```rust\r\nlet x = 1;\r\n```\r\n",
            "let x = 1;",
            &RenderOptions::default()
        ),
        Some("let x = 1;".to_string())
    );
}
