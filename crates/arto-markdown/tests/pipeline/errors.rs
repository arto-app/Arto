//! When rendering fails

use super::*;
use arto_markdown::RenderError;
use std::error::Error as _;

#[test]
fn a_document_nested_deeper_than_the_parser_goes_is_a_parse_error() {
    let markdown = format!("{} too deep", ">".repeat(200));

    let error = render_to_html(
        &markdown,
        Path::new("/nonexistent/test.md"),
        &RenderOptions::default(),
    )
    .expect_err("the parser gives up");

    assert!(matches!(error, RenderError::Parse(_)), "{error:?}");
    assert!(
        error.source().is_some(),
        "the parser's own error says where it gave up"
    );
}
