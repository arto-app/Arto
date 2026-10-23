//! What a URL written in a document is, by the question asked of it.
//!
//! Each question has its own answer here rather than all of them sharing one
//! notion of "remote" or "local", because they disagree at the edges and the
//! disagreement is the point of each:
//!
//! - [`fetches_from_elsewhere`]: does loading this leave the machine? Asked
//!   before media is shown on a page that must not send requests. Anything
//!   with a scheme but `data:` counts, `file:` and one-letter schemes
//!   included, and the URL is read the way a browser reads it.
//! - [`has_foreign_scheme`]: does this href address something other than a
//!   file? Asked of a link, and of a `srcset` candidate that could not be
//!   resolved. `file:` and a Windows drive letter (`C:`) are files.
//! - [`needs_no_resolution`]: is this image reference loadable as written?
//!   Only `http:`, `https:` and `data:` are; anything else is tried as a
//!   path, so a file named `shot:1.png` is still found.
//! - [`file_url_to_path`]: which file does a `file:` URL name?

use std::path::PathBuf;

/// Whether loading `url` makes the page fetch something from another host:
/// an absolute or protocol-relative address that is neither inline data nor
/// under `local`, where the app serves images from.
///
/// Read the way a browser's URL parser reads it on a page served over
/// http: tabs and newlines dropped, a backslash taken for a slash — so
/// `\\host/x` is as protocol-relative as `//host/x`.
pub(crate) fn fetches_from_elsewhere(url: &str, local: Option<&str>) -> bool {
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

/// Whether `href` names a URL scheme that is not `file:`.
///
/// `http:`, `mailto:`, `tel:` and the like address something outside the
/// file system, so they stay anchors instead of being resolved as document
/// paths. A one-letter prefix is not read as a scheme, which keeps a Windows
/// path such as `C:\notes\a.md` a path.
pub(crate) fn has_foreign_scheme(href: &str) -> bool {
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

/// Whether an image reference is something the page loads as written, with
/// no file to look for.
pub(crate) fn needs_no_resolution(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:")
}

/// A `file:` URL as a filesystem path, or `None` when `href` is not one or
/// names no file on this machine.
///
/// `file://`, `file://localhost/…` and `file:/…` all parse; percent-encoding
/// and the platform's path shape are handled by `url`.
pub(crate) fn file_url_to_path(href: &str) -> Option<PathBuf> {
    if !href.starts_with("file:") {
        return None;
    }
    let path = url::Url::parse(href)
        .ok()
        .and_then(|url| url.to_file_path().ok());
    if path.is_none() {
        tracing::debug!(?href, "file: URL names no file on this machine");
    }
    path
}

/// One entry of a `srcset` attribute: a URL and the descriptor that follows it.
pub(crate) struct SrcsetCandidate {
    pub(crate) url: String,
    pub(crate) descriptor: String,
}

/// Split a `srcset` attribute into its candidates.
///
/// A candidate is a run of non-whitespace characters followed by an optional
/// width or density descriptor, and candidates are separated by commas. The
/// URL is taken up to the first whitespace rather than up to the first comma,
/// as HTML specifies, so the commas inside a `data:` URL stay part of it; a
/// URL that ends in a comma ends its candidate and carries no descriptor.
/// Only ASCII whitespace separates, again as HTML specifies, so a file name
/// holding an ideographic or non-breaking space stays one URL.
pub(crate) fn parse_srcset(value: &str) -> Vec<SrcsetCandidate> {
    let mut candidates = Vec::new();
    let mut rest = value;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            return candidates;
        }
        let url_end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let (url, tail) = rest.split_at(url_end);
        let (url, descriptor, tail) = if url.ends_with(',') {
            (url.trim_end_matches(','), "", tail)
        } else {
            let (descriptor, tail) = tail.split_at(tail.find(',').unwrap_or(tail.len()));
            (url, descriptor.trim(), tail)
        };
        if !url.is_empty() {
            candidates.push(SrcsetCandidate {
                url: url.to_string(),
                descriptor: descriptor.to_string(),
            });
        }
        rest = tail;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_srcset_reads_urls_and_descriptors() {
        let candidates = parse_srcset("./a.png 1x,  ./a@2x.png 2x ,./b.png");
        let pairs: Vec<(&str, &str)> = candidates
            .iter()
            .map(|c| (c.url.as_str(), c.descriptor.as_str()))
            .collect();

        assert_eq!(
            pairs,
            vec![("./a.png", "1x"), ("./a@2x.png", "2x"), ("./b.png", "")]
        );
    }

    #[test]
    fn parse_srcset_keeps_the_commas_inside_a_data_url() {
        let candidates = parse_srcset("data:image/png;base64,AAA= 2x, data:image/gif;base64,BBB=,");
        let pairs: Vec<(&str, &str)> = candidates
            .iter()
            .map(|c| (c.url.as_str(), c.descriptor.as_str()))
            .collect();

        assert_eq!(
            pairs,
            vec![
                ("data:image/png;base64,AAA=", "2x"),
                ("data:image/gif;base64,BBB=", ""),
            ]
        );
    }

    /// A comma with no whitespace after it does NOT start a new candidate:
    /// HTML collects the URL as a run of non-whitespace, so `a.png,b.png 2x`
    /// is the single URL `a.png,b.png`. Verified against Chromium, which
    /// resolves that value to one request for `a.png,b.png`. Splitting on the
    /// comma instead would inline an image the browser would never have
    /// chosen, so this pins the spec reading rather than the tempting one.
    #[test]
    fn parse_srcset_does_not_split_a_candidate_on_a_comma_without_whitespace() {
        let candidates = parse_srcset("a.png,b.png 2x");
        let pairs: Vec<(&str, &str)> = candidates
            .iter()
            .map(|c| (c.url.as_str(), c.descriptor.as_str()))
            .collect();

        assert_eq!(pairs, vec![("a.png,b.png", "2x")]);
    }

    /// HTML separates candidates on ASCII whitespace only, so a file name
    /// holding an ideographic space is one URL, not a URL and a descriptor.
    #[test]
    fn parse_srcset_keeps_non_ascii_whitespace_inside_a_url() {
        let candidates = parse_srcset("./図\u{3000}1.png 2x");
        let pairs: Vec<(&str, &str)> = candidates
            .iter()
            .map(|c| (c.url.as_str(), c.descriptor.as_str()))
            .collect();

        assert_eq!(pairs, vec![("./図\u{3000}1.png", "2x")]);
    }
}
