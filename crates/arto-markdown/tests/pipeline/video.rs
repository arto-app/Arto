//! GitHub video attachments

use super::*;

pub(super) const ATTACHMENT: &str =
    "https://github.com/user-attachments/assets/2f1c9e0a-6b1d-4c4e-9a59-0d7c1e3b8f42";

#[test]
fn a_video_attachment_on_a_line_of_its_own_plays_where_it_stands() {
    // A video dropped into GitHub's editor is written as its bare URL, and
    // GitHub shows a player there rather than a link.
    let html = render(&format!("Before.\n\n{ATTACHMENT}\n\nAfter.\n"));

    assert!(
        has_element(&html, "video", &[("src", ATTACHMENT)]),
        "{html}"
    );
    assert!(html.contains(" controls"), "{html}");
    assert_eq!(html.matches("<a ").count(), 1, "{html}");
}

#[test]
fn a_video_attachment_holds_its_link_for_a_page_that_cannot_play_it() {
    let html = render(&format!("{ATTACHMENT}\n"));

    assert!(
        html.contains(&format!(
            r#"<a href="{ATTACHMENT}">{ATTACHMENT}</a></video>"#
        )),
        "{html}"
    );
}

#[test]
fn a_video_attachment_keeps_the_range_of_its_line() {
    let html = render(&format!("Before.\n\n{ATTACHMENT}\n"));

    assert!(
        html.contains(&format!(
            r#"<p data-source-range="3:1-3:{}"><video "#,
            ATTACHMENT.len()
        )),
        "{html}"
    );
}

#[test]
fn a_video_attachment_in_a_quote_plays_too() {
    let html = render(&format!("> {ATTACHMENT}\n"));

    assert!(
        has_element(&html, "video", &[("src", ATTACHMENT)]),
        "{html}"
    );
}

#[test]
fn a_video_url_from_the_older_upload_host_plays_too() {
    let url = "https://user-images.githubusercontent.com/1234567/2f1c9e0a-6b1d-4c4e-9a59-0d7c1e3b8f42.mp4";
    let html = render(&format!("{url}\n"));

    assert!(has_element(&html, "video", &[("src", url)]), "{html}");
}

#[test]
fn an_image_from_the_older_upload_host_stays_a_link() {
    // That host serves images as well, and only the extension tells them
    // apart; an image uploaded there was written as image syntax.
    let url = "https://user-images.githubusercontent.com/1234567/2f1c9e0a.png";
    let html = render(&format!("{url}\n"));

    assert!(!html.contains("<video"), "{html}");
}

#[test]
fn a_video_attachment_inside_a_sentence_stays_a_link() {
    let html = render(&format!("See {ATTACHMENT} for the demo.\n"));

    assert!(!html.contains("<video"), "{html}");
    assert!(has_element(&html, "a", &[("href", ATTACHMENT)]), "{html}");
}

#[test]
fn a_labelled_link_to_a_video_attachment_stays_a_link() {
    let html = render(&format!("[the demo]({ATTACHMENT})\n"));

    assert!(!html.contains("<video"), "{html}");
}

#[test]
fn a_video_attachment_in_a_code_block_stays_code() {
    let html = render(&format!("```\n{ATTACHMENT}\n```\n"));

    assert!(!html.contains("<video"), "{html}");
}
