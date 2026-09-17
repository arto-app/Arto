//! What raw HTML is allowed to carry into a rendered document.
//!
//! [`RawHtml::Filter`](crate::RawHtml::Filter) promises to neutralize the tags
//! that restyle or script the page, and GFM's tagfilter — the extension that
//! keeps that promise — is a list of tag names: `<script>`, `<style>`,
//! `<iframe>` and six more. A document scripts the page through none of them.
//! `<img src=x onerror=…>` wears an ordinary tag and `<a href="javascript:…">`
//! an ordinary attribute, so both reach the reader untouched by the filter
//! that is meant to have stopped them.
//!
//! This module is the rest of that promise. The predicates below name what a
//! filtered document may not carry, and [`super::post_process`] drops whatever
//! they match while it is already walking the HTML.
//!
//! It matters more in the app than in `arto page`. A standalone page carries a
//! `Content-Security-Policy` whose `script-src` allowlists its own inline
//! scripts by hash, so script a document smuggles in cannot run there whatever
//! this module does. The app's WebView has no such policy to fall back on:
//! Dioxus boots its interpreter from inline `<script type="module">` blocks it
//! writes into the body itself, and their text carries a key that differs per
//! webview, so there is no stable hash to allowlist and no nonce to read. The
//! document is written into that same WebView with `dangerous_inner_html`,
//! beside the bridge that talks to Rust. There, this is the boundary.

/// Whether an attribute is an inline event handler — `onclick`, `onerror`,
/// `ontoggle` and the rest of the `on*` family.
///
/// Every one of them is script the document brought with it, and a handler is
/// only ever spelled `on` followed by the name of an event, so the shape is
/// the whole test rather than a list that a new event would age out of. The
/// bare name `on` is not one: it is the prefix with no event behind it. Nor is
/// a name carrying anything but letters and digits after the prefix — no event
/// is named with a hyphen or a colon, which is what keeps `only-child` an
/// ordinary attribute.
fn is_event_handler(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 2
        && bytes[0].eq_ignore_ascii_case(&b'o')
        && bytes[1].eq_ignore_ascii_case(&b'n')
        && bytes[2..].iter().all(u8::is_ascii_alphanumeric)
}

/// The attributes whose value a browser follows as a URL.
///
/// `srcset` is deliberately absent: it holds a list of candidates rather than
/// one URL, and every candidate is fetched as an image rather than followed,
/// so a scheme that only means anything when followed means nothing there.
const URL_ATTRIBUTES: &[&str] = &[
    "href",
    "src",
    "action",
    "formaction",
    "data",
    "poster",
    "background",
    "ping",
    "xlink:href",
];

/// Whether following `value` as a URL would run script.
///
/// The scheme compared is the one a browser reads rather than the one written:
/// ASCII whitespace and NUL are stripped out of a URL before its scheme is
/// parsed, so `java&#9;script:alert(1)` names the same scheme as
/// `javascript:alert(1)` and has to fail the same test.
fn is_script_url(value: &str) -> bool {
    let normalized: String = value
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '\0')
        .collect();
    let Some(colon) = normalized.find(':') else {
        return false;
    };
    let scheme = &normalized[..colon];
    scheme.eq_ignore_ascii_case("javascript") || scheme.eq_ignore_ascii_case("vbscript")
}

/// Whether an attribute is one a filtered document may not keep.
///
/// Both halves are about the same thing reaching the page by a different
/// route: script written into an attribute, and script written into a URL an
/// attribute points at.
pub(super) fn is_unsafe_attribute(name: &str, value: &str) -> bool {
    is_event_handler(name) || (URL_ATTRIBUTES.contains(&name) && is_script_url(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_handlers_go_by_their_prefix() {
        assert!(is_event_handler("onclick"));
        assert!(is_event_handler("onerror"));
        assert!(is_event_handler("ontoggle"));
        // lol_html lowercases attribute names, but the test does not depend on
        // it having done so.
        assert!(is_event_handler("OnLoad"));
    }

    #[test]
    fn ordinary_attributes_are_not_event_handlers() {
        assert!(!is_event_handler("on"));
        assert!(!is_event_handler("href"));
        assert!(!is_event_handler(""));
        // No event is named with a hyphen or a colon, so neither of these is
        // a handler however much it looks like one.
        assert!(!is_event_handler("only-child"));
        assert!(!is_event_handler("on-click"));
        assert!(!is_event_handler("xlink:on"));
        // A non-ASCII name is read by bytes, so it must not panic on a
        // character boundary.
        assert!(!is_event_handler("お名前"));
    }

    #[test]
    fn script_urls_are_recognised_however_they_are_spelled() {
        assert!(is_script_url("javascript:alert(1)"));
        assert!(is_script_url("JaVaScRiPt:alert(1)"));
        assert!(is_script_url("vbscript:msgbox(1)"));
        assert!(is_script_url("  javascript:alert(1)"));
        // The characters a browser strips before reading the scheme.
        assert!(is_script_url("java\tscript:alert(1)"));
        assert!(is_script_url("java\nscript:alert(1)"));
        assert!(is_script_url("java\0script:alert(1)"));
    }

    #[test]
    fn ordinary_urls_are_left_alone() {
        assert!(!is_script_url("https://example.com/a.png"));
        assert!(!is_script_url("./notes.md"));
        assert!(!is_script_url("#heading"));
        assert!(!is_script_url("mailto:someone@example.com"));
        assert!(!is_script_url("data:image/png;base64,AAAA"));
        // A relative path may carry a colon of its own without naming a scheme.
        assert!(!is_script_url("./a:b.md"));
    }

    #[test]
    fn only_url_attributes_are_read_as_urls() {
        assert!(is_unsafe_attribute("href", "javascript:alert(1)"));
        assert!(is_unsafe_attribute("src", "javascript:alert(1)"));
        assert!(is_unsafe_attribute("formaction", "javascript:alert(1)"));
        // `alt` is not followed, so what looks like a scheme in it is text.
        assert!(!is_unsafe_attribute("alt", "javascript:alert(1)"));
        assert!(!is_unsafe_attribute("title", "javascript:alert(1)"));
    }

    #[test]
    fn safe_attributes_survive() {
        assert!(!is_unsafe_attribute("href", "./notes.md"));
        assert!(!is_unsafe_attribute("src", "image.png"));
        assert!(!is_unsafe_attribute("class", "md-link"));
    }
}
