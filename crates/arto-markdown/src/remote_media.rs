//! Taking out what a page that may not fetch from elsewhere would fetch.

use lol_html::html_content::{ContentType, Element};
use lol_html::{element, Settings};

use crate::rewrite::rewrite;

/// Whether `url` makes the page fetch something from another host: an
/// absolute or protocol-relative address that is neither inline data nor
/// under `local`, where the app serves images from.
///
/// Read the way a browser's URL parser reads it on a page served over
/// http: tabs and newlines dropped, a backslash taken for a slash — so
/// `\\host/x` is as protocol-relative as `//host/x`.
fn is_remote(url: &str, local: Option<&str>) -> bool {
    let url: String = url
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .map(|c| if c == '\\' { '/' } else { c })
        .collect();
    let url = url.trim_matches(|c: char| c <= ' ');
    if local.is_some_and(|local| url.starts_with(local)) {
        return false;
    }
    if url.starts_with("//") {
        return true;
    }
    match url.split_once(':') {
        Some((scheme, _)) => {
            let is_scheme = !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
            is_scheme && !scheme.eq_ignore_ascii_case("data")
        }
        None => false,
    }
}

/// `html` with every image, video or other media that would be fetched from
/// another host taken out, for a page that is not allowed to load it.
///
/// An image leaves its alt text; a video or an audio player leaves the
/// fallback content inside it, which for a GitHub video is the link to it.
/// Inline `data:` media is kept.
pub fn without_remote_media(html: impl AsRef<str>) -> String {
    strip_remote_media(html.as_ref(), None, false)
}

/// `html` without its `data-source-range` attributes, and without media
/// fetched from anywhere but `local` (see [`crate::render_detached`]).
pub(crate) fn detach(html: &str, local: Option<&str>) -> String {
    strip_remote_media(html, local, true)
}

/// `html` without media fetched from anywhere but `local`, and without its
/// `data-source-range` attributes when `strip_ranges` is set.
fn strip_remote_media(html: &str, local: Option<&str>, strip_ranges: bool) -> String {
    let fetches_elsewhere = |el: &Element| {
        let srcset = el.get_attribute("srcset").unwrap_or_default();
        ["src", "poster", "data"]
            .iter()
            .filter_map(|name| el.get_attribute(name))
            .chain(
                srcset
                    .split(',')
                    .filter_map(|candidate| candidate.split_whitespace().next())
                    .map(str::to_string),
            )
            .any(|url| is_remote(&url, local))
    };
    let mut settings = Settings::new();
    if strip_ranges {
        settings = settings.append_element_content_handler(element!("[data-source-range]", |el| {
            el.remove_attribute("data-source-range");
            Ok(())
        }));
    }
    let settings = settings.append_element_content_handler(element!(
        "img, video, audio, source, track, picture, object, embed, input",
        move |el| {
            if fetches_elsewhere(el) {
                // What a player holds is shown only where it cannot play, so
                // it is the stand-in; the media nested in it is matched by
                // this handler as well.
                if matches!(el.tag_name().as_str(), "video" | "audio") {
                    el.remove_and_keep_content();
                } else {
                    let alt = el.get_attribute("alt").unwrap_or_default();
                    el.replace(&alt, ContentType::Text);
                }
            }
            Ok(())
        }
    ));
    // A page that may not fetch from elsewhere gets a document cut short
    // rather than the media this pass did not reach.
    rewrite(html, settings).unwrap_or_else(|interrupted| {
        tracing::debug!(error = %interrupted.error, "stripping remote media stopped part way");
        interrupted.written
    })
}
