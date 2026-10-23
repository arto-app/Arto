//! Wiki links

use super::*;

#[test]
fn a_wiki_link_becomes_a_document_link() {
    // The href gets `.md`, so the post-processing pass turns it into an
    // in-app link like any other document link.
    let html = render("See [[README]] and [[README|the index]].");

    assert!(html.contains(r#"data-md-link="README.md""#), "{html}");
    assert!(html.contains(">README</span>"), "{html}");
    assert!(html.contains(">the index</span>"), "{html}");
    assert!(!html.contains("[["), "{html}");
}

#[test]
fn a_wiki_link_inside_code_is_left_alone() {
    let html = render("Inline `[[README]]` and:\n\n```\n[[README]]\n```\n");

    assert_eq!(html.matches("[[README]]").count(), 2, "{html}");
    assert!(!html.contains("<a href"), "{html}");
}

#[test]
fn a_wiki_target_with_spaces_stays_a_path() {
    // The app opens the target as a file name, so it must not reach it
    // percent-encoded the way an ordinary link's href would.
    let html = render("[[My Note]]");

    assert!(html.contains(r#"data-md-link="My Note.md""#), "{html}");
}

#[test]
fn a_wiki_label_is_rendered_as_markup() {
    let html = render("[[Guide|**the** guide]]");

    assert!(html.contains("<strong>the</strong> guide</span>"), "{html}");
}

#[test]
fn a_url_in_a_wiki_label_does_not_become_a_second_link() {
    // The label is already inside the anchor the wiki link opened, so an
    // autolinked URL there would nest one `<a>` in another and the browser
    // would close the outer one early, dropping the rest of the label out
    // of the link.
    let html = render("[[https://example.com|see https://other.com now]]");

    assert_eq!(html.matches("<a ").count(), 1, "{html}");
    assert!(html.contains("see https://other.com now</a>"), "{html}");
}
