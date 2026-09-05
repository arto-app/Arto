//! Pure functions for sidebar cursor navigation.
//!
//! These functions compute cursor movement within a flat list of visible items,
//! enabling j/k style keyboard navigation in the sidebar file tree.
//! No Dioxus dependency — easy to unit test.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::file::is_markdown_file;

/// Maximum recursion depth for directory traversal.
/// Prevents unbounded recursion from symlink cycles or extremely deep trees.
const MAX_DEPTH: usize = 128;

/// Build a flat list of visible tree nodes by walking the directory tree.
///
/// Replicates the ordering in `file_explorer.rs`:
/// directories first, then files, both alphabetical.
/// Respects `show_all_files` filter (hides non-markdown files when false).
/// Only recurses into expanded directories.
pub fn visible_items(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    show_all_files: bool,
) -> Vec<PathBuf> {
    let mut items = Vec::new();
    collect_visible(root, expanded, show_all_files, &mut items, 0);
    items
}

fn collect_visible(
    dir: &Path,
    expanded: &HashSet<PathBuf>,
    show_all_files: bool,
    out: &mut Vec<PathBuf>,
    depth: usize,
) {
    if depth >= MAX_DEPTH {
        tracing::warn!(?dir, depth, "Reached max recursion depth in sidebar cursor");
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        tracing::debug!(?dir, "Failed to read directory for cursor navigation");
        return;
    };

    // Collect (path, is_dir) tuples using DirEntry::file_type() to avoid
    // redundant stat syscalls. Each entry is stat'd once here instead of
    // multiple times in sort comparator + loop body.
    let mut children: Vec<(PathBuf, bool)> = entries
        .filter_map(|e| e.ok())
        .map(|e| {
            let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            (e.path(), is_dir)
        })
        .collect();

    // Sort: directories first, then files, both alphabetical
    children.sort_by(|(a, a_is_dir), (b, b_is_dir)| match (a_is_dir, b_is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.file_name().cmp(&b.file_name()),
    });

    for (child, is_dir) in children {
        // Filter non-markdown files when show_all_files is false
        if !show_all_files && !is_dir && !is_markdown_file(&child) {
            continue;
        }

        out.push(child.clone());

        // Recurse into expanded directories
        if is_dir && expanded.contains(&child) {
            collect_visible(&child, expanded, show_all_files, out, depth + 1);
        }
    }
}

/// Move cursor down (next item). Returns the new cursor position.
///
/// - `None` current → first item
/// - At end → stays at last item (no wrap)
/// - Current not found in items → first item
pub fn move_down(current: &Option<PathBuf>, items: &[PathBuf]) -> Option<PathBuf> {
    if items.is_empty() {
        return None;
    }
    let Some(cur) = current else {
        return Some(items[0].clone());
    };
    let pos = items.iter().position(|p| p == cur);
    match pos {
        Some(i) if i + 1 < items.len() => Some(items[i + 1].clone()),
        Some(i) => Some(items[i].clone()), // stay at end
        None => Some(items[0].clone()),    // current not found
    }
}

/// Move cursor up (previous item). Returns the new cursor position.
///
/// - `None` current → last item
/// - At start → stays at first item (no wrap)
/// - Current not found in items → last item
pub fn move_up(current: &Option<PathBuf>, items: &[PathBuf]) -> Option<PathBuf> {
    if items.is_empty() {
        return None;
    }
    let Some(cur) = current else {
        return Some(items[items.len() - 1].clone());
    };
    let pos = items.iter().position(|p| p == cur);
    match pos {
        Some(0) => Some(items[0].clone()), // stay at start
        Some(i) => Some(items[i - 1].clone()),
        None => Some(items[items.len() - 1].clone()), // current not found
    }
}

/// Find the parent directory entry in the visible list.
///
/// Used for the "collapse" action: when cursor is on a file or collapsed directory,
/// move cursor to its parent directory in the tree.
pub fn find_parent_dir(current: &Path, items: &[PathBuf]) -> Option<PathBuf> {
    let parent = current.parent()?;
    items.iter().find(|p| *p == parent).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_tree() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create structure:
        // root/
        //   alpha/
        //     nested.md
        //   beta/
        //   file_a.md
        //   file_b.txt
        //   file_c.markdown
        fs::create_dir(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/nested.md"), "# Nested").unwrap();
        fs::create_dir(root.join("beta")).unwrap();
        fs::write(root.join("file_a.md"), "# A").unwrap();
        fs::write(root.join("file_b.txt"), "text").unwrap();
        fs::write(root.join("file_c.markdown"), "# C").unwrap();

        tmp
    }

    #[test]
    fn visible_items_no_expanded_markdown_only() {
        let tmp = setup_test_tree();
        let root = tmp.path();
        let expanded = HashSet::new();

        let items = visible_items(root, &expanded, false);

        // Dirs first (alpha, beta), then markdown files (file_a.md, file_c.markdown)
        // file_b.txt is hidden (not markdown)
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].file_name().unwrap(), "alpha");
        assert_eq!(items[1].file_name().unwrap(), "beta");
        assert_eq!(items[2].file_name().unwrap(), "file_a.md");
        assert_eq!(items[3].file_name().unwrap(), "file_c.markdown");
    }

    #[test]
    fn visible_items_show_all_files() {
        let tmp = setup_test_tree();
        let root = tmp.path();
        let expanded = HashSet::new();

        let items = visible_items(root, &expanded, true);

        // Dirs first, then all files including .txt
        assert_eq!(items.len(), 5);
        assert_eq!(items[2].file_name().unwrap(), "file_a.md");
        assert_eq!(items[3].file_name().unwrap(), "file_b.txt");
        assert_eq!(items[4].file_name().unwrap(), "file_c.markdown");
    }

    #[test]
    fn visible_items_with_expanded_dir() {
        let tmp = setup_test_tree();
        let root = tmp.path();
        let mut expanded = HashSet::new();
        expanded.insert(root.join("alpha"));

        let items = visible_items(root, &expanded, false);

        // alpha, alpha/nested.md, beta, file_a.md, file_c.markdown
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].file_name().unwrap(), "alpha");
        assert_eq!(items[1].file_name().unwrap(), "nested.md");
        assert_eq!(items[2].file_name().unwrap(), "beta");
    }

    #[test]
    fn visible_items_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let expanded = HashSet::new();

        let items = visible_items(tmp.path(), &expanded, true);
        assert!(items.is_empty());
    }

    #[test]
    fn move_down_from_none_selects_first() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(move_down(&None, &items), Some(PathBuf::from("/a")));
    }

    #[test]
    fn move_down_advances() {
        let items = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        let current = Some(PathBuf::from("/a"));
        assert_eq!(move_down(&current, &items), Some(PathBuf::from("/b")));
    }

    #[test]
    fn move_down_stays_at_end() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let current = Some(PathBuf::from("/b"));
        assert_eq!(move_down(&current, &items), Some(PathBuf::from("/b")));
    }

    #[test]
    fn move_down_current_not_found_selects_first() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let current = Some(PathBuf::from("/missing"));
        assert_eq!(move_down(&current, &items), Some(PathBuf::from("/a")));
    }

    #[test]
    fn move_down_empty_list() {
        assert_eq!(move_down(&None, &[]), None);
        assert_eq!(move_down(&Some(PathBuf::from("/a")), &[]), None);
    }

    #[test]
    fn move_up_from_none_selects_last() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(move_up(&None, &items), Some(PathBuf::from("/b")));
    }

    #[test]
    fn move_up_advances() {
        let items = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        let current = Some(PathBuf::from("/c"));
        assert_eq!(move_up(&current, &items), Some(PathBuf::from("/b")));
    }

    #[test]
    fn move_up_stays_at_start() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let current = Some(PathBuf::from("/a"));
        assert_eq!(move_up(&current, &items), Some(PathBuf::from("/a")));
    }

    #[test]
    fn move_up_current_not_found_selects_last() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let current = Some(PathBuf::from("/missing"));
        assert_eq!(move_up(&current, &items), Some(PathBuf::from("/b")));
    }

    #[test]
    fn move_up_empty_list() {
        assert_eq!(move_up(&None, &[]), None);
    }

    #[test]
    fn move_single_item() {
        let items = vec![PathBuf::from("/only")];
        assert_eq!(move_down(&None, &items), Some(PathBuf::from("/only")));
        assert_eq!(move_up(&None, &items), Some(PathBuf::from("/only")));
        let current = Some(PathBuf::from("/only"));
        assert_eq!(move_down(&current, &items), Some(PathBuf::from("/only")));
        assert_eq!(move_up(&current, &items), Some(PathBuf::from("/only")));
    }

    #[test]
    fn find_parent_dir_found() {
        let items = vec![
            PathBuf::from("/root/alpha"),
            PathBuf::from("/root/alpha/file.md"),
            PathBuf::from("/root/beta"),
        ];
        let current = PathBuf::from("/root/alpha/file.md");
        assert_eq!(
            find_parent_dir(&current, &items),
            Some(PathBuf::from("/root/alpha"))
        );
    }

    #[test]
    fn find_parent_dir_not_in_list() {
        let items = vec![PathBuf::from("/root/alpha/file.md")];
        let current = PathBuf::from("/root/alpha/file.md");
        // Parent /root/alpha is not in items
        assert_eq!(find_parent_dir(&current, &items), None);
    }

    #[test]
    fn find_parent_dir_root_path() {
        let items = vec![PathBuf::from("/")];
        let current = PathBuf::from("/");
        // Root has no parent
        assert_eq!(find_parent_dir(&current, &items), None);
    }
}
