//! Pure functions for sidebar cursor navigation.
//!
//! These functions compute cursor movement within a flat list of visible items,
//! enabling j/k style keyboard navigation in the sidebar file tree.
//! No Dioxus dependency — easy to unit test.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::sidebar::{Group, PanelRow, TreeRow};
use crate::utils::file::is_markdown_file;

/// Maximum recursion depth for directory traversal.
/// Prevents unbounded recursion from symlink cycles or extremely deep trees.
const MAX_DEPTH: usize = 128;

/// Build a flat list of visible tree nodes, over every root the tree shows.
///
/// Replicates the ordering in `file_explorer.rs`: each root heads its own
/// subtree, directories before files and both alphabetical, so the cursor
/// moves down through one root and on into the next exactly as the eye does.
/// Respects `show_all_files` (hides non-markdown files when false) and only
/// recurses into expanded directories.
pub fn visible_items_in_roots(
    roots: &[PanelRow],
    expanded: &HashSet<TreeRow>,
    show_all_files: bool,
) -> Vec<PanelRow> {
    let mut items = Vec::new();
    for (group, root) in roots {
        items.push((*group, root.clone()));
        // A shut root is one row, not a subtree. The rows under it are not
        // drawn, and a cursor that walked them would disappear into a folder
        // nobody had opened.
        if expanded.contains(&(*group, root.clone(), root.clone())) {
            collect_visible(*group, root, root, expanded, show_all_files, &mut items, 0);
        }
    }
    items
}

fn collect_visible(
    group: Group,
    root: &Path,
    dir: &Path,
    expanded: &HashSet<TreeRow>,
    show_all_files: bool,
    out: &mut Vec<PanelRow>,
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

        out.push((group, child.clone()));

        // Recurse into expanded directories. The row is the root it descends
        // from as well as its path: one folder can be drawn under two roots,
        // open in one and shut in the other.
        if is_dir && expanded.contains(&(group, root.to_path_buf(), child.clone())) {
            collect_visible(
                group,
                root,
                &child,
                expanded,
                show_all_files,
                out,
                depth + 1,
            );
        }
    }
}

/// Move cursor down (next item). Returns the new cursor position.
///
/// - `None` current → first item
/// - At end → stays at last item (no wrap)
/// - Current not found in items → first item
pub fn move_down<T: Clone + PartialEq>(current: &Option<T>, items: &[T]) -> Option<T> {
    if items.is_empty() {
        return None;
    }
    let Some(cur) = current else {
        return Some(items[0].clone());
    };
    let pos = items.iter().position(|item| item == cur);
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
pub fn move_up<T: Clone + PartialEq>(current: &Option<T>, items: &[T]) -> Option<T> {
    if items.is_empty() {
        return None;
    }
    let Some(cur) = current else {
        return Some(items[items.len() - 1].clone());
    };
    let pos = items.iter().position(|item| item == cur);
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
pub fn find_parent_dir(current: &PanelRow, items: &[PanelRow]) -> Option<PanelRow> {
    let (group, path) = current;
    let parent = path.parent()?;
    items
        .iter()
        .find(|(row_group, row)| row_group == group && row == parent)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// One root's visible items, without the root row `visible_items_in_roots`
    /// draws above them.
    fn visible_items(
        root: &Path,
        expanded: &HashSet<TreeRow>,
        show_all_files: bool,
    ) -> Vec<PathBuf> {
        rows(root, expanded, show_all_files)
            .into_iter()
            .map(|(_, path)| path)
            .collect()
    }

    /// The rows themselves, for the tests that care which list they are in.
    fn rows(root: &Path, expanded: &HashSet<TreeRow>, show_all_files: bool) -> Vec<PanelRow> {
        let mut items = Vec::new();
        collect_visible(
            Group::Current,
            root,
            root,
            expanded,
            show_all_files,
            &mut items,
            0,
        );
        items
    }

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
    fn a_shut_root_is_one_row() {
        let tmp = setup_test_tree();
        let root = tmp.path();

        let items = visible_items_in_roots(
            &[(Group::Current, root.to_path_buf())],
            &HashSet::new(),
            false,
        );

        assert_eq!(items, vec![(Group::Current, root.to_path_buf())]);
    }

    #[test]
    fn an_open_root_is_its_rows_as_well() {
        let tmp = setup_test_tree();
        let root = tmp.path();
        let mut expanded = HashSet::new();
        expanded.insert((Group::Current, root.to_path_buf(), root.to_path_buf()));

        let items =
            visible_items_in_roots(&[(Group::Current, root.to_path_buf())], &expanded, false);

        assert!(items.len() > 1);
        assert_eq!(items[0], (Group::Current, root.to_path_buf()));
    }

    #[test]
    fn visible_items_with_expanded_dir() {
        let tmp = setup_test_tree();
        let root = tmp.path();
        let mut expanded = HashSet::new();
        expanded.insert((Group::Current, root.to_path_buf(), root.join("alpha")));

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

    /// The history draws a document once under every day it was read on, so
    /// two rows can name one path. The day is part of what names a row, and
    /// this is why: keyed by the path alone the cursor found the first of them
    /// wherever it stood, and everything below the second was unreachable.
    #[test]
    fn the_cursor_walks_past_a_document_listed_under_two_days() {
        use crate::state::Group;
        use crate::visits::Bucket;

        let items = vec![
            (Group::Day(Bucket::Today), PathBuf::from("/a.md")),
            (Group::Day(Bucket::Today), PathBuf::from("/b.md")),
            (Group::Day(Bucket::Yesterday), PathBuf::from("/a.md")),
            (Group::Day(Bucket::Yesterday), PathBuf::from("/c.md")),
        ];

        let mut at = Some(items[0].clone());
        for expected in &items[1..] {
            at = move_down(&at, &items);
            assert_eq!(at.as_ref(), Some(expected));
        }

        for expected in items[..items.len() - 1].iter().rev() {
            at = move_up(&at, &items);
            assert_eq!(at.as_ref(), Some(expected));
        }
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
        assert_eq!(move_down::<PathBuf>(&None, &[]), None);
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
        assert_eq!(move_up::<PathBuf>(&None, &[]), None);
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

    /// A row, in the group the tree's own rows are in.
    fn row(path: &str) -> PanelRow {
        (Group::Current, PathBuf::from(path))
    }

    #[test]
    fn find_parent_dir_found() {
        let items = vec![
            row("/root/alpha"),
            row("/root/alpha/file.md"),
            row("/root/beta"),
        ];

        assert_eq!(
            find_parent_dir(&row("/root/alpha/file.md"), &items),
            Some(row("/root/alpha"))
        );
    }

    #[test]
    fn find_parent_dir_not_in_list() {
        let items = vec![row("/root/alpha/file.md")];
        // Parent /root/alpha is not in items
        assert_eq!(find_parent_dir(&row("/root/alpha/file.md"), &items), None);
    }

    #[test]
    fn find_parent_dir_root_path() {
        let items = vec![row("/")];
        // Root has no parent
        assert_eq!(find_parent_dir(&row("/"), &items), None);
    }

    #[test]
    fn find_parent_dir_stays_in_its_own_group() {
        // The same folder is drawn in both groups; going up from a row in one
        // of them must not land in the other.
        let items = vec![
            (Group::Current, PathBuf::from("/w/arto")),
            (Group::Current, PathBuf::from("/w/arto/docs")),
            (Group::Bookmark, PathBuf::from("/w/arto")),
        ];

        assert_eq!(
            find_parent_dir(&(Group::Current, PathBuf::from("/w/arto/docs")), &items),
            Some((Group::Current, PathBuf::from("/w/arto")))
        );
        assert_eq!(
            find_parent_dir(&(Group::Bookmark, PathBuf::from("/w/arto/docs")), &items),
            Some((Group::Bookmark, PathBuf::from("/w/arto")))
        );
        // …and the group with no such parent drawn in it has nowhere to go.
        assert_eq!(
            find_parent_dir(&(Group::Flat, PathBuf::from("/w/arto/docs")), &items),
            None
        );
    }
}
