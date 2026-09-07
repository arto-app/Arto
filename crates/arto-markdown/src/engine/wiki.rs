//! The href a wiki link target points at.
//!
//! ox-content parses `[[Page]]` and `[[Page|Label]]` into a link whose url is
//! the target as written. What is left is Arto's own rule: a target names a
//! Markdown document in the same directory, so one without a file extension
//! gets `.md` and the post-processing pass turns the anchor into an in-app
//! link like any other document link.

/// The href a wiki target points at.
pub(super) fn href(target: &str) -> String {
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
    fn a_bare_target_names_the_document_of_that_name() {
        assert_eq!(href("README"), "README.md");
    }

    #[test]
    fn a_target_that_already_names_a_file_keeps_its_extension() {
        assert_eq!(href("notes/today.md"), "notes/today.md");
    }

    #[test]
    fn a_url_is_left_as_written() {
        assert_eq!(href("https://example.com"), "https://example.com");
    }

    #[test]
    fn a_fragment_stays_on_the_end_of_the_href() {
        assert_eq!(href("Guide#Setup"), "Guide.md#Setup");
        assert_eq!(href("notes/today.md#Setup"), "notes/today.md#Setup");
    }

    #[test]
    fn a_target_that_is_only_a_fragment_stays_an_in_page_anchor() {
        assert_eq!(href("#Setup"), "#Setup");
    }
}
