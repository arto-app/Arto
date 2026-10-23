use lol_html::{element, Settings};
use std::cell::RefCell;
use std::path::Path;

use crate::images::{inline_srcset, resolved_image_src, DeferredImages};
use crate::links::{link_target_exists, local_link, make_document_link};
use crate::rewrite::{rewrite, Interrupted};
use crate::sanitize::is_unsafe_attribute;
use crate::{DeferredImage, ImageResolution, RawHtml};

/// Post-process HTML with lol_html.
///
/// Handles:
/// - `<img src="…">`, `<img srcset="…">` and `<source srcset="…">`: resolve
///   local images the way `resolution` asks for, so a `<picture>` renders
///   whichever candidate the browser picks. See [`crate::images::resolve_local_image`] for
///   how a reference becomes a file.
/// - `<a href="…">`: convert local links to `<span data-md-link="…">` for in-app
///   navigation; a Markdown target that does not exist is marked `md-link-missing`
/// - every element, unless `raw_html` is [`RawHtml::Allow`]: drop the
///   attributes [`crate::sanitize`] names as script the document brought with
///   it
///
/// Returns the rewritten HTML and, under [`ImageResolution::Deferred`], the
/// images the host is now expected to serve — or the pass that stopped part
/// way, whose output only went through the filter as far as it got.
pub(super) fn post_process_html_tags(
    html_str: &str,
    base_dir: &Path,
    resolution: &ImageResolution,
    raw_html: RawHtml,
) -> Result<(String, Vec<DeferredImage>), Interrupted> {
    let canonical_base = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());
    let link_base = canonical_base.clone();

    // Both image handlers write into the one list. They borrow it rather than
    // share ownership of it, so that reading it back after the rewriter is
    // done cannot fail — and a change that kept a handler alive too long
    // would be a compile error rather than a silently empty list.
    let collected = RefCell::new(DeferredImages::default());

    // Appended before every other handler because handlers run in the order
    // they were appended, and the anchor handler below adds the one `on*`
    // attribute Arto itself relies on. A filter running after it would take
    // that attribute straight back off again.
    let mut settings = Settings::new();
    if raw_html != RawHtml::Allow {
        settings = settings.append_element_content_handler(element!("*", |el| {
            // Every name is read before any of them is removed: the attribute
            // list borrows the element, and removing one needs it back.
            let unsafe_attributes: Vec<String> = el
                .attributes()
                .iter()
                .filter(|attribute| is_unsafe_attribute(&attribute.name(), &attribute.value()))
                .map(|attribute| attribute.name())
                .collect();
            for name in unsafe_attributes {
                el.remove_attribute(&name);
            }
            Ok(())
        }));
    }

    let settings = settings
        .append_element_content_handler(element!("img[src]", |el| {
            if let Some(src) = el.get_attribute("src") {
                if let Some(resolved) =
                    resolved_image_src(&src, &canonical_base, resolution, &collected)
                {
                    el.set_attribute("src", &resolved)?;
                }
            }
            Ok(())
        }))
        // A `<source>` inside a `<picture>` is what the browser picks in
        // the theme it matches, so it needs the same inlining as `<img>`
        // or that theme shows nothing.
        .append_element_content_handler(element!("img[srcset], source[srcset]", |el| {
            if let Some(srcset) = el.get_attribute("srcset") {
                match inline_srcset(&srcset, &canonical_base, resolution, &collected) {
                    // Nothing is left to pick from, so the attribute
                    // has to go rather than stay empty: an `<img>`
                    // then falls back to its `src` and a `<source>`
                    // is ignored in favor of the `<picture>`'s `<img>`.
                    Some(inlined) if inlined.is_empty() => {
                        el.remove_attribute("srcset");
                    }
                    Some(inlined) => el.set_attribute("srcset", &inlined)?,
                    None => {}
                }
            }
            Ok(())
        }))
        // Process anchor tags: convert markdown links to spans
        .append_element_content_handler(element!("a[href]", move |el| {
            let Some(href) = el.get_attribute("href") else {
                return Ok(());
            };
            // A link that is only a fragment, or that addresses something
            // other than a file, stays an anchor.
            let Some(local) = local_link(&href) else {
                return Ok(());
            };
            let class = if !local.is_markdown() {
                "md-link md-link-invalid"
            } else if !link_target_exists(&link_base, &local.path) {
                "md-link md-link-missing"
            } else {
                "md-link"
            };
            let link = match local.fragment {
                Some(fragment) => format!("{}#{fragment}", local.path),
                None => local.path,
            };
            make_document_link(el, &link, class)?;
            Ok(())
        }));

    let html = rewrite(html_str, settings)?;
    Ok((html, collected.into_inner().entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// The rewritten HTML alone, filtered as a document is by default, for the
    /// tests that only look at the markup.
    fn post_process(html: &str, base_dir: &Path) -> String {
        post_process_with(html, base_dir, RawHtml::Filter)
    }

    /// The same, for the tests that are about which raw-HTML setting is in
    /// force.
    fn post_process_with(html: &str, base_dir: &Path, raw_html: RawHtml) -> String {
        post_process_html_tags(html, base_dir, &ImageResolution::DataUrl, raw_html)
            .expect("post-processes")
            .0
    }

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
        let result = post_process(html, &sub);

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
        let result = post_process(html, &sub);

        assert!(
            result.contains("data:image/png;base64,"),
            "Images within base_dir should be converted: {result}"
        );
    }

    /// Single quotes in href are safely stored in data-md-link attribute
    #[test]
    fn test_link_single_quote_in_data_attribute() {
        let html = r#"<a href="file's.md">link</a>"#;
        let result = post_process(html, Path::new("/tmp"));
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
        let result = post_process(html, Path::new("/tmp"));
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
        let result = post_process(html, Path::new("/tmp"));

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
        let result = post_process(html, Path::new("/tmp"));

        assert_eq!(result, html);
    }

    #[test]
    fn a_file_url_link_is_resolved_to_a_path() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("note.md");
        fs::write(&target, "# Note").unwrap();
        let url = url::Url::from_file_path(&target).unwrap();

        let html = format!(r#"<a href="{url}#section">note</a>"#);
        let result = post_process(&html, temp_dir.path());

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
        let result = post_process(html, Path::new("/tmp"));

        assert!(result.contains("data-md-link"), "{result}");
    }

    /// Characterization: HTTP URLs are not converted (this is correct behavior)
    #[test]
    fn test_http_urls_not_converted() {
        let html = r#"<img src="https://example.com/img.png">"#;
        let result = post_process(html, Path::new("/tmp"));
        assert!(result.contains("https://example.com/img.png"));
    }

    #[test]
    fn test_post_process_html_tags_img() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.png");
        let png_data = vec![0x89, 0x50, 0x4E, 0x47];
        fs::write(&image_path, png_data).unwrap();

        let html = r#"<p><img src="test.png" alt="test" /></p>"#;
        let result = post_process(html, temp_dir.path());

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
        let result = post_process(html, temp.path());

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
        let result = post_process(html, temp.path());

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
        let result = post_process(html, temp.path());

        assert!(result.contains(r#"class="md-link""#), "{result}");
        assert!(
            result.contains(r#"data-md-link="./doc.md#section""#),
            "the fragment must reach the click handler: {result}"
        );
    }

    #[test]
    fn fragment_only_links_stay_anchors() {
        let html = r##"<a href="#section">Here</a>"##;
        let result = post_process(html, Path::new("."));

        assert_eq!(result, html);
    }

    #[test]
    fn test_post_process_html_tags_http_urls() {
        let html =
            r#"<img src="https://example.com/image.png" /><a href="https://example.com">Link</a>"#;
        let result = post_process(html, Path::new("."));

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
        let result = post_process(html, Path::new("."));

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
        let result = post_process(html, temp.path());

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
        let result = post_process(&html, temp_dir.path());
        assert!(
            result.contains("data:image/png;base64,"),
            "file:///... URL should be converted to data URL: {result}"
        );

        // file://localhost/absolute/path form: replace the empty host with "localhost"
        let localhost_url = file_url.replacen("file:///", "file://localhost/", 1);
        let html2 = format!(r#"<img src="{}">"#, localhost_url);
        let result2 = post_process(&html2, temp_dir.path());
        assert!(
            result2.contains("data:image/png;base64,"),
            "file://localhost/... URL should be converted to data URL: {result2}"
        );
    }

    /// The `<picture>` shape GitHub documents for theme-aware images: the
    /// `<source>` the browser picks in dark mode must be inlined too, or that
    /// theme renders nothing.
    #[test]
    fn source_srcset_is_inlined() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("dark.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
        fs::write(temp.path().join("light.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<picture><source media="(prefers-color-scheme: dark)" srcset="./dark.png"><img src="./light.png" alt="hero"></picture>"#;
        let result = post_process(html, temp.path());

        assert_eq!(
            result.matches("data:image/png;base64,").count(),
            2,
            "both the source and the img must be inlined: {result}"
        );
        assert!(!result.contains("./dark.png"), "{result}");
    }

    #[test]
    fn img_srcset_candidates_are_inlined_with_their_descriptors() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("a.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
        fs::write(temp.path().join("a@2x.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<img src="a.png" srcset="a.png 1x, a@2x.png 2x">"#;
        let result = post_process(html, temp.path());

        assert!(result.contains("base64,iVBORw== 1x,"), "{result}");
        assert!(result.contains("base64,iVBORw== 2x\""), "{result}");
    }

    #[test]
    fn remote_srcset_candidates_are_left_alone() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("a.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<source srcset="https://example.com/a.png 1x, a.png 2x">"#;
        let result = post_process(html, temp.path());

        assert!(
            result.contains("https://example.com/a.png 1x, data:image/png;base64,"),
            "{result}"
        );
    }

    /// A local candidate whose file cannot be read is dropped: it could never
    /// load in a page that has no base URL, and left in place the browser
    /// would pick it — a `2x` candidate wins on every Retina display — over
    /// the candidate that did inline.
    #[test]
    fn an_unreadable_local_srcset_candidate_is_dropped() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("there.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<source srcset="there.png 1x, missing.png 2x">"#;
        let result = post_process(html, temp.path());

        assert!(!result.contains("missing.png"), "{result}");
        assert!(
            result.contains("data:image/png;base64,iVBORw== 1x"),
            "{result}"
        );
    }

    /// An empty file is dropped like an unreadable one. Inlined it would be
    /// `data:image/png;base64,`, and `srcset` parsing strips that trailing
    /// comma — leaving a malformed URL that still outranks, at `2x`, the
    /// candidate that did inline.
    #[test]
    fn an_empty_image_file_is_not_inlined() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("empty.png"), []).unwrap();
        fs::write(temp.path().join("there.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<img src="empty.png" srcset="there.png 1x, empty.png 2x">"#;
        let result = post_process(html, temp.path());

        assert!(!result.contains("base64,\""), "{result}");
        assert!(!result.contains("base64, "), "{result}");
        assert_eq!(
            result,
            r#"<img src="empty.png" srcset="data:image/png;base64,iVBORw== 1x">"#
        );
    }

    /// With every candidate gone the attribute goes too, so the `<img src>`
    /// that did inline is what renders instead of a broken candidate.
    #[test]
    fn a_srcset_of_only_unreadable_candidates_is_removed() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("there.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let html = r#"<img src="there.png" srcset="missing.png 1x,  other.png 2x">"#;
        let result = post_process(html, temp.path());

        assert!(!result.contains("srcset"), "{result}");
        assert!(
            result.contains("data:image/png;base64,iVBORw=="),
            "{result}"
        );
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
        let result = post_process(html, &docs);

        assert!(
            result.contains("data:image/png;base64,"),
            "./../ relative path should be converted to data URL: {result}"
        );
    }

    /// GFM's tagfilter escapes `<script>` but knows nothing about the `on*`
    /// family, so a filtered document arrives still carrying whichever of them
    /// it was written with. None of them may survive the rewrite.
    #[test]
    fn filtered_html_loses_its_event_handlers() {
        let temp = TempDir::new().unwrap();
        let html = r#"<img src="missing.png" onerror="alert(1)"><p ontoggle="alert(2)">text</p>"#;
        let result = post_process(html, temp.path());

        assert!(!result.contains("onerror"), "{result}");
        assert!(!result.contains("ontoggle"), "{result}");
        assert!(!result.contains("alert"), "{result}");
        // Only the handlers go; the element and its other attributes stay.
        assert!(result.contains("<p>text</p>"), "{result}");
    }

    /// A `javascript:` URL runs when the link is followed, so it is script by
    /// another route and goes the same way. The anchor itself is left standing:
    /// a link with nowhere to go is still the text the author wrote.
    #[test]
    fn filtered_html_loses_its_script_urls() {
        let temp = TempDir::new().unwrap();
        let html = r#"<a href="javascript:alert(1)">click</a>"#;
        let result = post_process(html, temp.path());

        assert!(!result.contains("javascript:"), "{result}");
        assert!(result.contains("click"), "{result}");
    }

    /// The filter runs before the anchor handler, which is what lets that
    /// handler add the `onmousedown` in-app navigation is driven by. Were the
    /// order the other way round, every Markdown link would stop working.
    #[test]
    fn the_filter_leaves_artos_own_link_handler_alone() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("notes.md");
        fs::write(&target, "# Notes").unwrap();

        let html = r#"<a href="notes.md">notes</a>"#;
        let result = post_process(html, temp.path());

        assert!(result.contains("onmousedown"), "{result}");
        assert!(result.contains("data-md-link"), "{result}");
    }

    /// An ordinary document is all ordinary attributes, and the filter is not
    /// allowed to cost it any of them.
    #[test]
    fn ordinary_attributes_survive_the_filter() {
        let temp = TempDir::new().unwrap();
        let html = r#"<p class="note" id="first" title="javascript:not-a-url">text</p>"#;
        let result = post_process(html, temp.path());

        assert!(result.contains(r#"class="note""#), "{result}");
        assert!(result.contains(r#"id="first""#), "{result}");
        // `title` is shown rather than followed, so what looks like a scheme in
        // it is just the text it says.
        assert!(result.contains("javascript:not-a-url"), "{result}");
    }

    /// `Allow` is the setting that says the document is trusted, and it means
    /// what it says: raw HTML goes through untouched, handlers and all.
    #[test]
    fn allow_passes_event_handlers_through() {
        let temp = TempDir::new().unwrap();
        let html = r#"<p onclick="alert(1)">text</p>"#;

        let filtered = post_process_with(html, temp.path(), RawHtml::Filter);
        assert!(!filtered.contains("onclick"), "{filtered}");

        let allowed = post_process_with(html, temp.path(), RawHtml::Allow);
        assert!(allowed.contains("onclick"), "{allowed}");
    }
}
