use percent_encoding::percent_decode_str;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMarkdownLink {
    pub path: PathBuf,
    pub heading_id: Option<String>,
}

fn split_href_fragment(href: &str) -> (&str, Option<&str>) {
    match href.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (href, None),
    }
}

fn normalize_heading_fragment(fragment: Option<&str>) -> Option<String> {
    fragment.and_then(|fragment| {
        let decoded = percent_decode_str(fragment).decode_utf8_lossy();
        let trimmed = decoded.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub fn is_internal_markdown_href(href: &str) -> bool {
    if href.starts_with('#') {
        return true;
    }

    let (path, _) = split_href_fragment(href);
    crate::utils::file::is_markdown_file(path)
}

pub fn resolve_markdown_link(
    base_dir: &Path,
    current_file: &Path,
    href: &str,
) -> Option<ResolvedMarkdownLink> {
    let (path, fragment) = split_href_fragment(href);
    let heading_id = normalize_heading_fragment(fragment);

    let resolved_path = if path.is_empty() {
        current_file.to_path_buf()
    } else {
        base_dir.join(path).canonicalize().ok()?
    };

    Some(ResolvedMarkdownLink {
        path: resolved_path,
        heading_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_internal_markdown_href_supports_fragments() {
        assert!(is_internal_markdown_href("#section-1"));
        assert!(is_internal_markdown_href("guide.md#overview"));
        assert!(is_internal_markdown_href("guide.markdown"));
        assert!(!is_internal_markdown_href("guide.txt#overview"));
    }

    #[test]
    fn test_resolve_markdown_link_same_file_heading() {
        let current_file = PathBuf::from("/tmp/doc.md");
        let resolved = resolve_markdown_link(Path::new("/tmp"), &current_file, "#section-1");

        assert_eq!(
            resolved,
            Some(ResolvedMarkdownLink {
                path: current_file,
                heading_id: Some("section-1".to_string()),
            })
        );
    }

    #[test]
    fn test_resolve_markdown_link_other_file_heading() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current_file = temp_dir.path().join("current.md");
        let linked_file = temp_dir.path().join("linked.md");
        std::fs::write(&current_file, "# current").unwrap();
        std::fs::write(&linked_file, "# linked").unwrap();
        let linked_file = linked_file.canonicalize().unwrap();

        let resolved = resolve_markdown_link(temp_dir.path(), &current_file, "linked.md#part%201");

        assert_eq!(
            resolved,
            Some(ResolvedMarkdownLink {
                path: linked_file,
                heading_id: Some("part 1".to_string()),
            })
        );
    }
}
