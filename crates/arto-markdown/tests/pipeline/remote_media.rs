//! Remote media

use arto_markdown::without_remote_media;

#[test]
fn a_data_url_in_a_srcset_stays_whatever_its_commas_are_followed_by() {
    // A `srcset` URL runs to the next whitespace, so everything after the
    // commas here is the data URL's payload, not an address of its own.
    let html = r#"<img alt="dot" srcset="data:image/gif;base64,R0lG,mailto:x 2x">"#;

    assert_eq!(without_remote_media(html), html);
}

#[test]
fn a_relative_srcset_url_holding_a_comma_is_one_local_url() {
    // A comma with no whitespace after it does not start a candidate, so the
    // browser requests `a.png,https://…` relative to the page.
    let html = r#"<img alt="a" srcset="a.png,https://example.com/b.png 2x">"#;

    assert_eq!(without_remote_media(html), html);
}

#[test]
fn a_remote_candidate_after_a_data_url_in_a_srcset_is_taken_out() {
    let html = without_remote_media(
        r#"<img alt="dot" srcset="data:image/gif;base64,AAA= 1x, https://attacker.example/b.png 2x">"#,
    );

    assert!(!html.contains("attacker.example"), "{html}");
    assert_eq!(html, "dot");
}

#[test]
fn a_srcset_address_written_with_backslashes_is_taken_out() {
    let html = without_remote_media(
        r#"<img alt="b" srcset="./local.png 1x, \\attacker.example/b.png 2x">"#,
    );

    assert!(!html.contains("attacker.example"), "{html}");
}

#[test]
fn a_file_url_counts_as_fetched_from_elsewhere() {
    // A page that may not fetch from elsewhere has nothing to serve a file
    // from either, so a `file:` image goes the way a remote one does.
    let html = without_remote_media(r#"<img alt="secret" src="file:///etc/secret.png">"#);

    assert_eq!(html, "secret");
}
