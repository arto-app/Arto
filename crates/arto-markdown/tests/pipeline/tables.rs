//! Tables

use super::*;

#[test]
fn paragraphs_without_tables_keep_their_lines() {
    let html = render("Just a paragraph\n\nAnother one");

    assert!(
        html.contains(r#"<p data-source-range="1:1-1:16">Just a paragraph</p>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<p data-source-range="3:1-3:11">Another one</p>"#),
        "{html}"
    );
    assert!(!html.contains("<table"), "{html}");
}

#[test]
fn table_range_extends_to_its_last_row() {
    let html = render("| A | B |\n|---|---|\n| 1 | 2 |");

    assert!(
        has_element(&html, "table", &[("data-source-range", "1:1-3:9")]),
        "{html}"
    );
}

#[test]
fn each_table_gets_its_own_range() {
    let html = render("| A |\n|---|\n| 1 |\n\n| X |\n|---|\n| Y |\n\n| P |\n|---|\n| Q |");

    for range in ["1:1-3:5", "5:1-7:5", "9:1-11:5"] {
        assert!(
            has_element(&html, "table", &[("data-source-range", range)]),
            "table {range}: {html}"
        );
    }
}

#[test]
fn header_only_table_has_a_range() {
    let html = render("| A | B |\n|---|---|");

    assert!(
        has_element(&html, "table", &[("data-source-range", "1:1-2:9")]),
        "{html}"
    );
    assert!(html.contains("<thead>"), "{html}");
}
