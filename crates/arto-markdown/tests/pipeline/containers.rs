//! Mermaid and math containers

use super::*;

#[test]
fn mermaid_block_becomes_a_preprocessed_pre() {
    let html = render(indoc! {"
        ```mermaid
        graph TD
            A-->B
        ```
    "});

    assert!(html.contains(r#"class="preprocessed-mermaid""#), "{html}");
    // The attribute holds the diagram source; the newlines in it are
    // escaped, so the value survives as one attribute.
    assert!(
        html.contains(r#"data-original-content="graph TD&#10;    A--&gt;B&#10;""#),
        "{html}"
    );
    assert!(html.contains("</pre>"));
    assert!(!html.contains("language-mermaid"), "{html}");
}

#[test]
fn inline_math_becomes_a_preprocessed_span() {
    let html = render("This is inline math: $x = y + z$");

    assert!(
        html.contains(r#"<span class="preprocessed-math-inline" data-original-content="x = y + z">x = y + z</span>"#),
        "{html}"
    );
}

#[test]
fn display_math_becomes_a_preprocessed_div() {
    let html = render(indoc! {"
        Display math:

        $$
        x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}
        $$
    "});

    assert!(
        html.contains(r#"class="preprocessed-math-display""#),
        "{html}"
    );
    assert!(html.contains("data-original-content"), "{html}");
    assert!(html.contains("frac"), "{html}");
}

#[test]
fn display_math_in_the_middle_of_a_line_stays_inline() {
    // `$$…$$` is a block construct: written inside a paragraph it is read
    // as inline math, so both formulas here land in the inline container.
    let html = render("Inline $a + b$ and display $$c = d$$");

    assert_eq!(
        html.matches(r#"class="preprocessed-math-inline""#).count(),
        2,
        "{html}"
    );
    assert!(
        !html.contains(r#"class="preprocessed-math-display""#),
        "{html}"
    );
}

#[test]
fn empty_mermaid_block_still_becomes_a_container() {
    let html = render("```mermaid\n```");

    assert!(html.contains(r#"class="preprocessed-mermaid""#), "{html}");
    assert!(html.contains(r#"data-original-content="""#), "{html}");
}

#[test]
fn other_languages_stay_code_blocks() {
    let html = render("```python\nprint('hello')\n```");

    assert!(!html.contains("preprocessed"), "{html}");
    assert!(
        html.contains(r#"<code data-source-range="2:1-2:14" class="language-python">"#),
        "{html}"
    );
}

#[test]
fn mermaid_source_lines_cover_both_fences() {
    let html = render("```mermaid\ngraph TD\n```");

    assert!(
        has_element(
            &html,
            "pre",
            &[
                ("class", "preprocessed-mermaid"),
                ("data-source-range", "1:1-3:3"),
            ]
        ),
        "{html}"
    );
}

#[test]
fn empty_display_math_does_not_break_rendering() {
    let html = render("$$$$");

    // Whether the engine reports an empty display formula is its call; the
    // pipeline must survive either way.
    if html.contains("preprocessed") {
        assert!(html.contains("preprocessed-math-display"), "{html}");
    }
}
