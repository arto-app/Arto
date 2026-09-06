//! Wiki links: `[[Page]]` and `[[Page|Label]]`.
//!
//! ox-content does not implement the syntax, and its parser splits the
//! opening `[[` into two `Text` nodes of their own, so a render hook never
//! sees the construct whole. In the rendered HTML the run is contiguous
//! again, which is why [`super::annotate`] applies this over the finished
//! text instead. Delete this module if ox-content grows the syntax itself.
//!
//! The text handed in is already escaped for HTML, so the label passes
//! straight through and only the surrounding markup is added.

/// Rewrite every `[[…]]` in `text` to an anchor, or `None` when there is
/// none to rewrite. A pair that names no target is left as written.
pub(super) fn rewrite(text: &str) -> Option<String> {
    if !text.contains("[[") {
        return None;
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut rewrote = false;

    while let Some(open) = rest.find("[[") {
        let Some(close) = rest[open + 2..].find("]]").map(|end| open + 2 + end) else {
            break;
        };
        let (before, inner, after) = (&rest[..open], &rest[open + 2..close], &rest[close + 2..]);

        match split_target(inner) {
            Some((target, label)) => {
                out.push_str(before);
                out.push_str("<a href=\"");
                out.push_str(&href(target));
                out.push_str("\">");
                out.push_str(label);
                out.push_str("</a>");
                rewrote = true;
            }
            // `[[]]` and `[[|x]]` name nothing; keep them as written.
            None => out.push_str(&rest[..close + 2]),
        }
        rest = after;
    }

    out.push_str(rest);
    rewrote.then_some(out)
}

/// Split `target|label` into its parts, defaulting the label to the target.
fn split_target(inner: &str) -> Option<(&str, &str)> {
    let (target, label) = inner
        .split_once('|')
        .map_or((inner, None), |(target, label)| (target, Some(label)));
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    let label = label.map(str::trim).filter(|label| !label.is_empty());
    Some((target, label.unwrap_or(target)))
}

/// The href a wiki target points at.
///
/// A target without a file extension names a Markdown document, so `.md` is
/// added and the post-processing pass turns the anchor into an in-app link
/// like any other document link.
fn href(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }

    let (path, fragment) = target
        .split_once('#')
        .map_or((target, None), |(path, fragment)| (path, Some(fragment)));

    let mut href = String::with_capacity(target.len() + 3);
    href.push_str(path);
    let last_segment = path.rsplit('/').next().unwrap_or_default();
    if !path.is_empty() && !last_segment.contains('.') {
        href.push_str(".md");
    }
    if let Some(fragment) = fragment {
        href.push('#');
        href.push_str(fragment);
    }
    href
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_target_links_to_the_document_of_that_name() {
        assert_eq!(
            rewrite("See [[README]] here.").unwrap(),
            r#"See <a href="README.md">README</a> here."#
        );
    }

    #[test]
    fn a_label_after_the_bar_becomes_the_link_text() {
        assert_eq!(
            rewrite("[[README|Back to the index]]").unwrap(),
            r#"<a href="README.md">Back to the index</a>"#
        );
    }

    #[test]
    fn several_links_in_one_run_are_all_rewritten() {
        assert_eq!(
            rewrite("[[a]] and [[b|B]]").unwrap(),
            r#"<a href="a.md">a</a> and <a href="b.md">B</a>"#
        );
    }

    #[test]
    fn a_target_that_already_names_a_file_keeps_its_extension() {
        assert_eq!(
            rewrite("[[notes/today.md]]").unwrap(),
            r#"<a href="notes/today.md">notes/today.md</a>"#
        );
        assert_eq!(
            rewrite("[[https://example.com]]").unwrap(),
            r#"<a href="https://example.com">https://example.com</a>"#
        );
    }

    #[test]
    fn a_fragment_stays_on_the_end_of_the_href() {
        assert_eq!(
            rewrite("[[Guide#Setup|setup]]").unwrap(),
            r#"<a href="Guide.md#Setup">setup</a>"#
        );
    }

    #[test]
    fn text_without_a_pair_is_left_alone() {
        assert_eq!(rewrite("no links here"), None);
        assert_eq!(rewrite("[[unclosed"), None);
        assert_eq!(rewrite("[[]] and [[|only a label]]"), None);
    }

    #[test]
    fn the_surrounding_text_is_preserved_byte_for_byte() {
        // The input is already escaped, so nothing may be re-escaped.
        assert_eq!(
            rewrite("a &amp; b [[c]] &lt;d&gt;").unwrap(),
            r#"a &amp; b <a href="c.md">c</a> &lt;d&gt;"#
        );
    }
}
