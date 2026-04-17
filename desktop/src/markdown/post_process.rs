use base64::{engine::general_purpose, Engine as _};
use lol_html::{element, HtmlRewriter, Settings};
use std::path::Path;

use super::headings::HeadingInfo;

/// Infer MIME type from file extension
pub(super) fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        _ => "image/png", // Default
    }
}

/// Post-process HTML to handle img, anchor, and table tags using lol_html
pub(super) fn post_process_html_tags(
    html_str: &str,
    base_dir: &Path,
    table_source_lines: &[(usize, usize)],
) -> String {
    post_process_html_impl(html_str, base_dir, table_source_lines, None)
}

/// Post-process HTML to handle img, anchor, table tags, and add heading IDs using lol_html
pub(super) fn post_process_html_with_headings(
    html_str: &str,
    base_dir: &Path,
    headings: &[HeadingInfo],
    table_source_lines: &[(usize, usize)],
) -> String {
    post_process_html_impl(html_str, base_dir, table_source_lines, Some(headings))
}

/// Common HTML post-processing implementation.
///
/// Handles:
/// - `<table>`: inject `data-source-line` attributes for source mapping
/// - `<h1>`–`<h6>`: inject `id` attributes for TOC navigation (when `headings` is Some)
/// - `<img src="…">`: convert relative paths to data URLs with path traversal prevention
/// - `<a href="…">`: convert local links to `<span data-md-link="…">` for in-app navigation
fn post_process_html_impl(
    html_str: &str,
    base_dir: &Path,
    table_source_lines: &[(usize, usize)],
    headings: Option<&[HeadingInfo]>,
) -> String {
    let canonical_base = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());
    let mut output = Vec::new();
    let table_index = std::cell::RefCell::new(0usize);
    let table_source_lines = table_source_lines.to_vec();
    let heading_index = std::cell::RefCell::new(0usize);
    let headings = headings.map(|h| h.to_vec()).unwrap_or_default();

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                // Process table tags: inject source line attributes
                element!("table", |el| {
                    let mut idx = table_index.borrow_mut();
                    if let Some(&(start, end)) = table_source_lines.get(*idx) {
                        el.set_attribute("data-source-line", &start.to_string())?;
                        el.set_attribute("data-source-line-end", &end.to_string())?;
                    }
                    *idx += 1;
                    Ok(())
                }),
                // Process heading tags: add IDs for TOC navigation
                // No-op when headings is empty (called from post_process_html_tags)
                element!("h1, h2, h3, h4, h5, h6", |el| {
                    if headings.is_empty() {
                        return Ok(());
                    }
                    let mut idx = heading_index.borrow_mut();
                    if let Some(heading) = headings.get(*idx) {
                        el.set_attribute("id", &heading.id)?;
                    }
                    *idx += 1;
                    Ok(())
                }),
                // Process img tags: convert relative paths to data URLs
                element!("img[src]", move |el| {
                    if let Some(src) = el.get_attribute("src") {
                        if !src.starts_with("http://")
                            && !src.starts_with("https://")
                            && !src.starts_with("data:")
                        {
                            let absolute_path = canonical_base.join(&src);
                            if let Ok(canonical_path) = absolute_path.canonicalize() {
                                // Path traversal prevention: only allow files within base_dir
                                if !canonical_path.starts_with(&canonical_base) {
                                    tracing::warn!(
                                        ?src,
                                        "Image path escapes base directory, skipping"
                                    );
                                    return Ok(());
                                }
                                if let Ok(image_data) = std::fs::read(&canonical_path) {
                                    let mime_type = get_mime_type(&canonical_path);
                                    let base64_data = general_purpose::STANDARD.encode(&image_data);
                                    let data_url =
                                        format!("data:{};base64,{}", mime_type, base64_data);
                                    el.set_attribute("src", &data_url)?;
                                }
                            }
                        }
                    }
                    Ok(())
                }),
                // Process anchor tags: convert markdown links to spans
                element!("a[href]", |el| {
                    if let Some(href) = el.get_attribute("href") {
                        if !href.starts_with("http://") && !href.starts_with("https://") {
                            el.set_tag_name("span")?;
                            el.remove_attribute("href");
                            el.set_attribute("data-md-link", &href)?;
                            if crate::utils::markdown_link::is_internal_markdown_href(&href) {
                                el.set_attribute("class", "md-link")?;
                            } else {
                                el.set_attribute("class", "md-link md-link-invalid")?;
                            }
                            el.set_attribute("onmousedown",
                                "if(event.button===0||event.button===1){event.preventDefault();window.handleMarkdownLinkClick(this.dataset.mdLink,event.button)}")?;
                        }
                    }
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
        |chunk: &[u8]| {
            output.extend_from_slice(chunk);
        },
    );

    let _ = rewriter.write(html_str.as_bytes());
    let _ = rewriter.end();
    String::from_utf8(output).unwrap_or_else(|_| html_str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ========================================================================
    // Security regression tests
    // These verify the current hardened behavior (e.g. path traversal blocked,
    // no unsafe interpolation) and guard against security regressions.
    // If behavior is intentionally changed, update both the code and these tests.
    // ========================================================================

    /// Path traversal images must NOT be converted to data URLs.
    /// The base_dir boundary check prevents reading files outside base_dir.
    #[test]
    fn test_path_traversal_img_src_blocked() {
        let temp = TempDir::new().unwrap();
        let sub = temp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        // Create secret.png OUTSIDE base_dir (sub/)
        let secret = temp.path().join("secret.png");
        fs::write(&secret, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<img src="../secret.png">"#;
        let result = post_process_html_tags(html, &sub, &[]);

        // Path traversal should be blocked: image NOT converted to data URL
        assert!(
            !result.contains("data:image/png;base64,"),
            "Path-traversal images must not be converted to data URLs: {result}"
        );
        // Original src should remain unchanged
        assert!(
            result.contains(r#"src="../secret.png""#),
            "Original src attribute should be preserved: {result}"
        );
    }

    /// Images within base_dir should still be converted normally
    #[test]
    fn test_path_within_base_dir_still_converted() {
        let temp = TempDir::new().unwrap();
        let sub = temp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let image = sub.join("image.png");
        fs::write(&image, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<img src="image.png">"#;
        let result = post_process_html_tags(html, &sub, &[]);

        assert!(
            result.contains("data:image/png;base64,"),
            "Images within base_dir should be converted: {result}"
        );
    }

    /// Single quotes in href are safely stored in data-md-link attribute
    #[test]
    fn test_link_single_quote_in_data_attribute() {
        let html = r#"<a href="file's.md">link</a>"#;
        let result = post_process_html_tags(html, Path::new("/tmp"), &[]);
        // href is stored in data-md-link, not interpolated into JS
        assert!(
            result.contains("data-md-link"),
            "Should use data-md-link attribute: {result}"
        );
        assert!(
            result.contains("this.dataset.mdLink"),
            "Should read href from dataset: {result}"
        );
    }

    /// Special chars in href cannot cause XSS with data-* attribute pattern
    #[test]
    fn test_link_special_chars_safe_with_data_attribute() {
        let html = r#"<a href="test'-alert('xss').md">link</a>"#;
        let result = post_process_html_tags(html, Path::new("/tmp"), &[]);
        // href is stored in data attribute, never interpolated into JS string
        assert!(
            result.contains("data-md-link"),
            "Should use data-md-link attribute: {result}"
        );
        // The onmousedown handler reads from dataset, not from interpolated string
        assert!(
            !result.contains("handleMarkdownLinkClick('"),
            "Should NOT contain interpolated href in JS string: {result}"
        );
        assert!(
            result.contains("this.dataset.mdLink"),
            "Should read href safely from dataset: {result}"
        );
    }

    /// Verify that crafted href payloads cannot inject executable JavaScript.
    /// The `data-md-link` + `dataset.mdLink` pattern ensures the value is never
    /// interpolated into a JS string literal, so quote escapes are harmless.
    #[test]
    fn test_xss_injection_via_href_payload() {
        // Payload with .md extension so the anchor handler converts the link,
        // plus quotes and JS that would be dangerous if interpolated into JS.
        let html = r#"<a href="evil');alert(1).md">link</a>"#;
        let result = post_process_html_tags(html, Path::new("/tmp"), &[]);

        // The href must be stored in data-md-link, NOT spliced into inline JS
        assert!(
            result.contains("data-md-link"),
            "Href should be stored in data-md-link: {result}"
        );
        // The onmousedown handler must read from dataset, never from interpolation
        assert!(
            result.contains("this.dataset.mdLink"),
            "Should read href safely from dataset: {result}"
        );
        assert!(
            !result.contains("handleMarkdownLinkClick('"),
            "Href must NOT be interpolated into JS string: {result}"
        );
    }

    /// Characterization: HTTP URLs are not converted (this is correct behavior)
    #[test]
    fn test_http_urls_not_converted() {
        let html = r#"<img src="https://example.com/img.png">"#;
        let result = post_process_html_tags(html, Path::new("/tmp"), &[]);
        assert!(result.contains("https://example.com/img.png"));
    }

    #[test]
    fn test_get_mime_type() {
        assert_eq!(get_mime_type(Path::new("test.png")), "image/png");
        assert_eq!(get_mime_type(Path::new("test.jpg")), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.jpeg")), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.gif")), "image/gif");
        assert_eq!(get_mime_type(Path::new("test.svg")), "image/svg+xml");
        assert_eq!(get_mime_type(Path::new("test.webp")), "image/webp");
        assert_eq!(get_mime_type(Path::new("test.bmp")), "image/bmp");
        assert_eq!(get_mime_type(Path::new("test.ico")), "image/x-icon");
        assert_eq!(get_mime_type(Path::new("test.unknown")), "image/png");
    }

    #[test]
    fn test_post_process_html_tags_img() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.png");
        let png_data = vec![0x89, 0x50, 0x4E, 0x47];
        fs::write(&image_path, png_data).unwrap();

        let html = r#"<p><img src="test.png" alt="test" /></p>"#;
        let result = post_process_html_tags(html, temp_dir.path(), &[]);

        assert!(
            result.contains("data:image/png;base64,"),
            "Should convert img src to data URL"
        );
        assert!(
            !result.contains(r#"src="test.png""#),
            "Should not contain original path"
        );
    }

    #[test]
    fn test_post_process_html_tags_anchor() {
        let html = r#"<a href="doc.md">Link</a>"#;
        let result = post_process_html_tags(html, Path::new("."), &[]);

        assert!(
            result.contains("<span ") && result.contains(r#"class="md-link""#),
            "Should convert to span with md-link class: {result}"
        );
        assert!(
            result.contains(r#"data-md-link="doc.md""#),
            "Should store href in data attribute: {result}"
        );
        assert!(
            result.contains("handleMarkdownLinkClick"),
            "Should add click handler: {result}"
        );
        assert!(!result.contains("<a "), "Should not contain anchor tag");
    }

    #[test]
    fn test_post_process_html_tags_anchor_with_fragment() {
        let html = r#"<a href="doc.md#section-1">Link</a>"#;
        let result = post_process_html_tags(html, Path::new("."), &[]);

        assert!(
            result.contains(r#"data-md-link="doc.md#section-1""#),
            "Should preserve href fragment in data attribute: {result}"
        );
        assert!(
            result.contains(r#"class="md-link""#),
            "Should be a markdown link: {result}"
        );
        assert!(!result.contains("<a "), "Should not contain anchor tag");
    }

    #[test]
    fn test_post_process_html_tags_same_file_anchor() {
        let html = r##"<a href="#section-1">Link</a>"##;
        let result = post_process_html_tags(html, Path::new("."), &[]);

        assert!(
            result.contains(r##"data-md-link="#section-1""##),
            "Should preserve same-file anchor in data attribute: {result}"
        );
        assert!(
            result.contains(r#"class="md-link""#),
            "Should be a markdown link: {result}"
        );
        assert!(!result.contains("<a "), "Should not contain anchor tag");
    }

    #[test]
    fn test_post_process_html_tags_http_urls() {
        let html =
            r#"<img src="https://example.com/image.png" /><a href="https://example.com">Link</a>"#;
        let result = post_process_html_tags(html, Path::new("."), &[]);

        assert!(
            result.contains(r#"src="https://example.com/image.png""#),
            "Should keep HTTP img"
        );
        assert!(
            result.contains(r#"<a href="https://example.com""#),
            "Should keep HTTP link"
        );
    }

    #[test]
    fn test_post_process_html_tags_non_md_local_file() {
        let html = r#"<a href="file.txt">Text File</a>"#;
        let result = post_process_html_tags(html, Path::new("."), &[]);

        assert!(
            result.contains("<span ") && result.contains(r#"class="md-link md-link-invalid""#),
            "Should convert to span with md-link and md-link-invalid class: {result}"
        );
        assert!(
            result.contains("handleMarkdownLinkClick"),
            "Should add click handler for local files: {result}"
        );
        assert!(!result.contains("<a "), "Should not contain anchor tag");
    }

    #[test]
    fn test_post_process_html_tags_md_vs_other_files() {
        let html = r#"<a href="doc.md">MD</a><a href="file.txt">TXT</a>"#;
        let result = post_process_html_tags(html, Path::new("."), &[]);

        // MD file should have only md-link class
        assert!(
            result.contains(r#"class="md-link""#),
            "Should have md-link for .md file"
        );

        // TXT file should have both md-link and md-link-invalid classes
        assert!(
            result.contains(r#"class="md-link md-link-invalid""#),
            "Should have md-link and md-link-invalid for .txt file"
        );

        // Both should have click handlers
        let click_handler_count = result.matches("handleMarkdownLinkClick").count();
        assert_eq!(
            click_handler_count, 2,
            "Should have click handlers for both links"
        );
    }

    #[test]
    fn test_post_process_html_with_headings_injects_ids() {
        let html = r#"<h1 data-source-line="1">Title</h1><h2 data-source-line="3">Section</h2>"#;
        let headings = vec![
            HeadingInfo {
                level: 1,
                text: "Title".to_string(),
                id: "title".to_string(),
            },
            HeadingInfo {
                level: 2,
                text: "Section".to_string(),
                id: "section".to_string(),
            },
        ];

        let result = post_process_html_with_headings(html, Path::new("."), &headings, &[]);

        assert!(
            result.contains(r#"id="title""#),
            "H1 should get id from headings: {result}"
        );
        assert!(
            result.contains(r#"id="section""#),
            "H2 should get id from headings: {result}"
        );
    }

    #[test]
    fn test_post_process_html_with_headings_more_html_headings_than_info() {
        // When HTML has more headings than HeadingInfo entries, extra headings are skipped
        let html = r#"<h1>A</h1><h2>B</h2><h3>C</h3>"#;
        let headings = vec![HeadingInfo {
            level: 1,
            text: "A".to_string(),
            id: "a".to_string(),
        }];

        let result = post_process_html_with_headings(html, Path::new("."), &headings, &[]);

        assert!(
            result.contains(r#"id="a""#),
            "First heading should get id: {result}"
        );
        // Remaining headings should still render without error
        assert!(
            result.contains("<h2>B</h2>") || result.contains("<h2 >B</h2>"),
            "Extra headings should render without id: {result}"
        );
    }

    #[test]
    fn test_post_process_html_with_headings_empty_headings() {
        let html = r#"<h1>Title</h1>"#;
        let headings: Vec<HeadingInfo> = vec![];

        let result = post_process_html_with_headings(html, Path::new("."), &headings, &[]);

        // Should not crash, heading renders without id
        assert!(
            result.contains("Title"),
            "Should still render heading text: {result}"
        );
    }
}
