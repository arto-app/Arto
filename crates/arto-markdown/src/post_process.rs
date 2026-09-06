use base64::{engine::general_purpose, Engine as _};
use lol_html::{element, HtmlRewriter, Settings};
use std::io::Read;
use std::path::Path;

/// Maximum byte size of a local image that will be inlined as a data URL.
///
/// Prevents accidental misreferences (e.g. log files, device files like `/dev/zero`)
/// from freezing the UI or exhausting memory during Markdown rendering. Legitimate
/// screenshots, photos, and diagrams comfortably fit within this limit.
const MAX_INLINE_IMAGE_SIZE: u64 = 32 * 1024 * 1024;

/// Read a file for inlining, rejecting anything over `max_size` bytes.
///
/// Uses `metadata` as a fast path for regular files, then falls back to a bounded
/// `Read::take` so files whose reported length is unreliable (device files, files
/// that grow between stat and read) cannot exceed the limit.
fn read_image_bounded(path: &Path, max_size: u64) -> Option<Vec<u8>> {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > max_size {
            tracing::debug!(
                ?path,
                size = metadata.len(),
                limit = max_size,
                "Image exceeds inline size limit; skipping"
            );
            return None;
        }
    }

    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(max_size + 1).read_to_end(&mut buf).ok()?;
    if buf.len() as u64 > max_size {
        tracing::debug!(
            ?path,
            limit = max_size,
            "Image exceeded inline size limit during read; skipping"
        );
        return None;
    }
    Some(buf)
}

/// Whether the file a local link points at exists. The check is what the
/// click handler will do with the same path, so a link that cannot open is
/// shown as such before it is clicked.
fn link_target_exists(base_dir: &Path, path: &str) -> bool {
    let target = Path::new(path);
    if target.is_absolute() {
        target.is_file()
    } else {
        base_dir.join(target).is_file()
    }
}

/// Whether `href` names a URL scheme that is not `file:`.
///
/// `http:`, `mailto:`, `tel:` and the like address something outside the
/// file system, so they stay anchors instead of being resolved as document
/// paths. A one-letter prefix is not read as a scheme, which keeps a Windows
/// path such as `C:\notes\a.md` a path.
fn has_foreign_scheme(href: &str) -> bool {
    let Some(colon) = href.find(':') else {
        return false;
    };
    let scheme = &href[..colon];
    scheme.len() > 1
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        && !scheme.eq_ignore_ascii_case("file")
}

/// A `file:` URL as a filesystem path, or `None` when `href` is not one.
///
/// `file://`, `file://localhost/…` and `file:/…` all parse; percent-encoding
/// and the platform's path shape are handled by `url`.
fn file_url_to_path(href: &str) -> Option<String> {
    if !href.starts_with("file:") {
        return None;
    }
    let path = url::Url::parse(href)
        .ok()
        .and_then(|url| url.to_file_path().ok());
    if path.is_none() {
        tracing::debug!(?href, "file: URL could not be parsed; left as written");
    }
    Some(path?.to_string_lossy().into_owned())
}

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

/// Post-process HTML with lol_html.
///
/// Handles:
/// - `<img src="…">`: inline local images as data URLs. Supports `file:` URLs
///   (parsed via `url::Url`, handles percent-encoding and platform differences)
///   as well as absolute and relative filesystem paths. Relative paths resolve
///   against `base_dir`. Paths resolving outside `base_dir` are still read and
///   inlined (logged at `trace` level), matching standard Markdown viewer behavior.
///   Reads are bounded by `MAX_INLINE_IMAGE_SIZE` to prevent misreferenced huge
///   files or device files from freezing the UI.
/// - `<a href="…">`: convert local links to `<span data-md-link="…">` for in-app
///   navigation; a Markdown target that does not exist is marked `md-link-missing`
pub(super) fn post_process_html_tags(html_str: &str, base_dir: &Path) -> String {
    let canonical_base = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());
    let link_base = canonical_base.clone();
    let mut output = Vec::new();

    let mut rewriter = HtmlRewriter::new(
        Settings::new()
            // Process img tags: convert local paths to data URLs
            .append_element_content_handler(element!("img[src]", move |el| {
                if let Some(src) = el.get_attribute("src") {
                    if !src.starts_with("http://")
                        && !src.starts_with("https://")
                        && !src.starts_with("data:")
                    {
                        // Resolve the src to a filesystem path for inlining.
                        // `file:` URLs (including `file://`, `file://localhost/...`, and
                        // `file:/...`) are parsed properly (handles percent-encoding and
                        // platform differences). Other values are treated as plain
                        // filesystem paths: absolute paths are used as-is, and relative
                        // paths are joined against the markdown file's directory. After
                        // canonicalization, paths that resolve outside the base directory
                        // are still allowed and read; this is logged for debugging but not
                        // blocked.
                        let absolute_path = if src.starts_with("file:") {
                            // Try proper URL parsing first; fall back to stripping the
                            // scheme and treating the remainder as a plain path for
                            // slightly-nonconforming inputs (e.g. unencoded spaces).
                            let parsed = url::Url::parse(&src)
                                .ok()
                                .and_then(|u| u.to_file_path().ok());
                            if parsed.is_none() {
                                tracing::debug!(
                                    ?src,
                                    "file: URL could not be parsed; falling back to plain path"
                                );
                            }
                            parsed.or_else(|| {
                                // Strip the scheme prefix and use the rest as a path.
                                let raw = src
                                    .strip_prefix("file://")
                                    .or_else(|| src.strip_prefix("file:/"))
                                    .or_else(|| src.strip_prefix("file:"))
                                    .unwrap_or(&src);
                                let path = Path::new(raw);
                                if path.is_absolute() {
                                    Some(path.to_path_buf())
                                } else {
                                    Some(canonical_base.join(path))
                                }
                            })
                        } else {
                            let path = Path::new(&src);
                            if path.is_absolute() {
                                Some(path.to_path_buf())
                            } else {
                                Some(canonical_base.join(path))
                            }
                        };

                        if let Some(absolute_path) = absolute_path {
                            if let Ok(canonical_path) = absolute_path.canonicalize() {
                                if !canonical_path.starts_with(&canonical_base) {
                                    tracing::trace!(
                                        ?src,
                                        "Image path resolved outside base directory; proceeding with inline read"
                                    );
                                }
                                if let Some(image_data) =
                                    read_image_bounded(&canonical_path, MAX_INLINE_IMAGE_SIZE)
                                {
                                    let mime_type = get_mime_type(&canonical_path);
                                    let base64_data =
                                        general_purpose::STANDARD.encode(&image_data);
                                    let data_url =
                                        format!("data:{};base64,{}", mime_type, base64_data);
                                    el.set_attribute("src", &data_url)?;
                                }
                            }
                        }
                    }
                }
                Ok(())
            }))
            // Process anchor tags: convert markdown links to spans
            .append_element_content_handler(element!("a[href]", move |el| {
                let Some(href) = el.get_attribute("href") else {
                    return Ok(());
                };
                if has_foreign_scheme(&href) {
                    return Ok(());
                }
                // A fragment belongs to the target document, not to its file
                // name; a link that is only a fragment stays an in-page anchor.
                let (path, fragment) = href
                    .split_once('#')
                    .map_or((href.as_str(), None), |(path, fragment)| {
                        (path, Some(fragment))
                    });
                // The app resolves the link as a filesystem path, so a
                // `file:` URL is turned into one here rather than being
                // joined onto the base directory as a literal string.
                let path = match file_url_to_path(path) {
                    Some(path) => path,
                    None => path.to_string(),
                };
                let Some(ext) = Path::new(&path).extension().and_then(|e| e.to_str()) else {
                    return Ok(());
                };
                let class = if ext != "md" && ext != "markdown" {
                    "md-link md-link-invalid"
                } else if !link_target_exists(&link_base, &path) {
                    "md-link md-link-missing"
                } else {
                    "md-link"
                };
                let link = match fragment {
                    Some(fragment) => format!("{path}#{fragment}"),
                    None => path,
                };
                el.set_tag_name("span")?;
                el.remove_attribute("href");
                el.set_attribute("data-md-link", &link)?;
                el.set_attribute("class", class)?;
                el.set_attribute("onmousedown",
                    "if(event.button===0||event.button===1){event.preventDefault();window.handleMarkdownLinkClick(this.dataset.mdLink,event.button)}")?;
                Ok(())
            })),
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
    // These verify the current behavior (e.g. no unsafe interpolation)
    // and guard against security regressions.
    // If behavior is intentionally changed, update both the code and these tests.
    // ========================================================================

    /// Relative paths that traverse up the directory tree (e.g. `../`) are resolved
    /// relative to the markdown file's directory, matching standard Markdown viewer behavior.
    #[test]
    fn test_relative_path_traversal_img_src_resolved() {
        let temp = TempDir::new().unwrap();
        let sub = temp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        // Create image.png OUTSIDE base_dir (sub/), in a sibling directory
        let images_dir = temp.path().join("images");
        fs::create_dir(&images_dir).unwrap();
        let image = images_dir.join("image.png");
        fs::write(&image, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<img src="../images/image.png">"#;
        let result = post_process_html_tags(html, &sub);

        // Relative path traversal should be resolved and image converted to data URL
        assert!(
            result.contains("data:image/png;base64,"),
            "Relative path traversal images should be converted to data URLs: {result}"
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
        let result = post_process_html_tags(html, &sub);

        assert!(
            result.contains("data:image/png;base64,"),
            "Images within base_dir should be converted: {result}"
        );
    }

    /// Single quotes in href are safely stored in data-md-link attribute
    #[test]
    fn test_link_single_quote_in_data_attribute() {
        let html = r#"<a href="file's.md">link</a>"#;
        let result = post_process_html_tags(html, Path::new("/tmp"));
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
        let result = post_process_html_tags(html, Path::new("/tmp"));
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
        let result = post_process_html_tags(html, Path::new("/tmp"));

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

    #[test]
    fn links_with_a_scheme_of_their_own_stay_anchors() {
        // `mailto:contact@example.com` ends in something that looks like a
        // file extension; it is an address, not a document.
        let html = r#"<a href="mailto:contact@example.com">mail</a><a href="tel:+81-3-0000-0000">call</a>"#;
        let result = post_process_html_tags(html, Path::new("/tmp"));

        assert_eq!(result, html);
    }

    #[test]
    fn a_file_url_link_is_resolved_to_a_path() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("note.md");
        fs::write(&target, "# Note").unwrap();
        let url = url::Url::from_file_path(&target).unwrap();

        let html = format!(r#"<a href="{url}#section">note</a>"#);
        let result = post_process_html_tags(&html, temp_dir.path());

        // The app opens `data-md-link` as a filesystem path, so the URL must
        // not survive into it, and the target must be found rather than
        // reported missing.
        assert!(
            result.contains(&format!(
                r#"data-md-link="{}#section""#,
                target.to_string_lossy()
            )),
            "{result}"
        );
        assert!(result.contains(r#"class="md-link""#), "{result}");
        assert!(!result.contains("md-link-missing"), "{result}");
    }

    #[test]
    fn a_windows_drive_letter_is_still_a_path() {
        let html = r#"<a href="C:\notes\a.md">note</a>"#;
        let result = post_process_html_tags(html, Path::new("/tmp"));

        assert!(result.contains("data-md-link"), "{result}");
    }

    /// Characterization: HTTP URLs are not converted (this is correct behavior)
    #[test]
    fn test_http_urls_not_converted() {
        let html = r#"<img src="https://example.com/img.png">"#;
        let result = post_process_html_tags(html, Path::new("/tmp"));
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
        let result = post_process_html_tags(html, temp_dir.path());

        assert!(
            result.contains("data:image/png;base64,"),
            "Should convert img src to data URL"
        );
        assert!(
            !result.contains(r#"src="test.png""#),
            "Should not contain original path"
        );
    }

    /// A directory holding `doc.md`, for links whose target must exist.
    fn dir_with_doc() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("doc.md"), "# Doc").unwrap();
        temp
    }

    #[test]
    fn test_post_process_html_tags_anchor() {
        let temp = dir_with_doc();
        let html = r#"<a href="doc.md">Link</a>"#;
        let result = post_process_html_tags(html, temp.path());

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
    fn missing_markdown_target_is_marked() {
        let temp = TempDir::new().unwrap();
        let html = r#"<a href="./does-not-exist.md">Missing</a>"#;
        let result = post_process_html_tags(html, temp.path());

        assert!(
            result.contains(r#"class="md-link md-link-missing""#),
            "{result}"
        );
        assert!(
            result.contains(r#"data-md-link="./does-not-exist.md""#),
            "{result}"
        );
    }

    #[test]
    fn fragment_does_not_hide_the_extension() {
        let temp = dir_with_doc();
        let html = r#"<a href="./doc.md#section">Section</a>"#;
        let result = post_process_html_tags(html, temp.path());

        assert!(result.contains(r#"class="md-link""#), "{result}");
        assert!(
            result.contains(r#"data-md-link="./doc.md#section""#),
            "the fragment must reach the click handler: {result}"
        );
    }

    #[test]
    fn fragment_only_links_stay_anchors() {
        let html = r##"<a href="#section">Here</a>"##;
        let result = post_process_html_tags(html, Path::new("."));

        assert_eq!(result, html);
    }

    #[test]
    fn test_post_process_html_tags_http_urls() {
        let html =
            r#"<img src="https://example.com/image.png" /><a href="https://example.com">Link</a>"#;
        let result = post_process_html_tags(html, Path::new("."));

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
        let result = post_process_html_tags(html, Path::new("."));

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
        let temp = dir_with_doc();
        let html = r#"<a href="doc.md">MD</a><a href="file.txt">TXT</a>"#;
        let result = post_process_html_tags(html, temp.path());

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

    /// file:// URLs should be resolved to the local file and converted to data URLs.
    /// Tests the canonical `file:///absolute/path` form and `file://localhost/...` form.
    #[test]
    fn test_file_scheme_url_resolved() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().canonicalize().unwrap().join("image.png");
        fs::write(&image_path, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        // Use the url crate's from_file_path to build a correct, platform-appropriate
        // file URL (e.g., `file:///abs/path` on Unix, `file:///C:/abs/path` on Windows).
        let file_url = url::Url::from_file_path(&image_path)
            .expect("valid file path")
            .to_string();
        let html = format!(r#"<img src="{}">"#, file_url);
        let result = post_process_html_tags(&html, temp_dir.path());
        assert!(
            result.contains("data:image/png;base64,"),
            "file:///... URL should be converted to data URL: {result}"
        );

        // file://localhost/absolute/path form: replace the empty host with "localhost"
        let localhost_url = file_url.replacen("file:///", "file://localhost/", 1);
        let html2 = format!(r#"<img src="{}">"#, localhost_url);
        let result2 = post_process_html_tags(&html2, temp_dir.path());
        assert!(
            result2.contains("data:image/png;base64,"),
            "file://localhost/... URL should be converted to data URL: {result2}"
        );
    }

    /// `read_image_bounded` must reject files larger than the configured limit,
    /// both via the metadata fast path and via the bounded-read fallback.
    #[test]
    fn test_read_image_bounded_rejects_oversized() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("big.png");
        // 2 KiB file; limit 1 KiB.
        fs::write(&path, vec![0u8; 2048]).unwrap();

        assert!(
            read_image_bounded(&path, 1024).is_none(),
            "file larger than limit must be rejected"
        );
    }

    /// `read_image_bounded` must accept files at or below the limit.
    #[test]
    fn test_read_image_bounded_accepts_within_limit() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("ok.png");
        fs::write(&path, vec![0u8; 1024]).unwrap();

        let data = read_image_bounded(&path, 1024).expect("exactly-limit file must be accepted");
        assert_eq!(data.len(), 1024);
    }

    /// A `./../` style relative path should be resolved correctly
    #[test]
    fn test_dot_slash_relative_path_resolved() {
        let temp = TempDir::new().unwrap();
        let docs = temp.path().join("docs");
        fs::create_dir(&docs).unwrap();
        let images = temp.path().join("images");
        fs::create_dir(&images).unwrap();
        let image = images.join("diagram.png");
        fs::write(&image, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        // ./../images/diagram.png resolves from docs/ to images/
        let html = r#"<img src="./../images/diagram.png">"#;
        let result = post_process_html_tags(html, &docs);

        assert!(
            result.contains("data:image/png;base64,"),
            "./../ relative path should be converted to data URL: {result}"
        );
    }
}
