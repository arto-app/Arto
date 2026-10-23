//! GitHub alerts

use super::*;

#[test]
fn alert_note_renders_title_and_body() {
    let html = render(indoc! {"
        > [!NOTE]
        > This is a note
    "});

    assert!(html.contains(r#"<div class="markdown-alert markdown-alert-note""#));
    assert!(html.contains(r#"<p class="markdown-alert-title""#));
    assert!(html.contains(r#"<span class="alert-icon" data-alert-type="note"></span>NOTE"#));
    assert!(html.contains("This is a note"));
    assert!(html.contains("</div>"));
}

#[test]
fn alert_warning_uses_its_own_kind() {
    let html = render(indoc! {"
        > [!WARNING]
        > Be careful!
    "});

    assert!(html.contains("markdown-alert-warning"));
    assert!(html.contains(r#"data-alert-type="warning""#));
    assert!(html.contains("WARNING"));
    assert!(html.contains("Be careful!"));
}

#[test]
fn alert_keeps_every_quoted_line() {
    let html = render(indoc! {"
        > [!IMPORTANT]
        > First line
        > Second line
        > Third line
    "});

    assert!(html.contains("markdown-alert-important"));
    assert!(html.contains("First line"));
    assert!(html.contains("Second line"));
    assert!(html.contains("Third line"));
}

#[test]
fn every_alert_kind_is_recognized() {
    for (name, class) in [
        ("NOTE", "note"),
        ("TIP", "tip"),
        ("IMPORTANT", "important"),
        ("WARNING", "warning"),
        ("CAUTION", "caution"),
    ] {
        let html = render(&format!("> [!{name}]\n> Test content"));
        assert!(
            html.contains(&format!("markdown-alert-{class}")),
            "{name}: {html}"
        );
        assert!(html.contains(name), "{name}: {html}");
    }
}

#[test]
fn consecutive_alerts_each_keep_their_own_lines() {
    let html = render(indoc! {"
        > [!NOTE]
        > First note

        > [!WARNING]
        > A warning

        > [!TIP]
        > A tip
    "});

    for (class, line) in [("note", 1), ("warning", 4), ("tip", 7)] {
        let alert = html
            .find(&format!("markdown-alert-{class}"))
            .unwrap_or_else(|| panic!("{class}: {html}"));
        let start_tag = &html[html[..alert].rfind('<').unwrap()..];
        let start_tag = &start_tag[..start_tag.find('>').unwrap()];
        assert!(
            start_tag.contains(&format!(r#"data-source-range="{line}:1-"#)),
            "{class} should start on line {line}: {start_tag}"
        );
    }
}

#[test]
fn plain_blockquote_is_not_an_alert() {
    let html = render("Regular paragraph\n> Regular quote");

    assert!(!html.contains("markdown-alert"), "{html}");
    assert!(
        html.contains(r#"<blockquote data-source-range="2:1-2:15">"#),
        "{html}"
    );
    assert!(html.contains("Regular quote"));
}
