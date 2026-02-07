use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Compute the flat list of visible items in the sidebar file tree.
///
/// This mirrors the rendering order in `file_explorer.rs`:
/// directories first (sorted alphabetically), then files (sorted alphabetically),
/// recursing into expanded directories only.
///
/// When `show_all_files` is false, non-markdown files are excluded.
pub fn compute_visible_items(
    root: &Path,
    expanded_dirs: &HashSet<PathBuf>,
    show_all_files: bool,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    collect_visible_items(root, expanded_dirs, show_all_files, &mut result);
    result
}

fn collect_visible_items(
    dir: &Path,
    expanded_dirs: &HashSet<PathBuf>,
    show_all_files: bool,
    result: &mut Vec<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    crate::components::sidebar::file_explorer::sort_entries(&mut items);

    for item in items {
        let is_dir = item.is_dir();

        // Apply the same visibility filter as the file tree renderer
        if !is_dir && !show_all_files && !crate::utils::file::is_markdown_file(&item) {
            continue;
        }

        result.push(item.clone());

        // Recurse into expanded directories
        if is_dir && expanded_dirs.contains(&item) {
            collect_visible_items(&item, expanded_dirs, show_all_files, result);
        }
    }
}

/// Move the cursor down in the visible items list. Returns the new cursor path.
///
/// - If `current` is `None`, returns the first item.
/// - If `current` is the last item, returns it unchanged.
/// - If `current` is not found in the list (stale), returns `None`.
pub fn move_cursor_down(current: Option<&Path>, visible: &[PathBuf]) -> Option<PathBuf> {
    if visible.is_empty() {
        return None;
    }

    let Some(current) = current else {
        return visible.first().cloned();
    };

    let idx = visible.iter().position(|p| p == current)?;

    if idx + 1 < visible.len() {
        Some(visible[idx + 1].clone())
    } else {
        // Already at bottom, stay put
        Some(visible[idx].clone())
    }
}

/// Move the cursor up in the visible items list. Returns the new cursor path.
///
/// - If `current` is `None`, returns the last item.
/// - If `current` is the first item, returns it unchanged.
/// - If `current` is not found in the list (stale), returns `None`.
pub fn move_cursor_up(current: Option<&Path>, visible: &[PathBuf]) -> Option<PathBuf> {
    if visible.is_empty() {
        return None;
    }

    let Some(current) = current else {
        return visible.last().cloned();
    };

    let idx = visible.iter().position(|p| p == current)?;

    if idx > 0 {
        Some(visible[idx - 1].clone())
    } else {
        // Already at top, stay put
        Some(visible[idx].clone())
    }
}

/// Find the parent item of the cursor in the visible list.
///
/// "Parent" here means the directory that contains the current item.
/// Returns `None` if the cursor is at a top-level item (direct child of root).
pub fn find_parent_item(current: &Path, root: &Path) -> Option<PathBuf> {
    let parent = current.parent()?;
    if parent == root {
        None
    } else {
        Some(parent.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a directory structure for testing.
    /// Returns the root path.
    fn setup_tree(temp: &TempDir) -> PathBuf {
        let root = temp.path().to_path_buf();
        // Directories
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::create_dir_all(root.join("beta")).unwrap();
        // Files
        fs::write(root.join("readme.md"), "# Hello").unwrap();
        fs::write(root.join("notes.md"), "Notes").unwrap();
        fs::write(root.join("image.png"), [0x89]).unwrap();
        // Nested
        fs::write(root.join("alpha/a1.md"), "A1").unwrap();
        fs::write(root.join("alpha/a2.txt"), "A2").unwrap();
        root
    }

    #[test]
    fn test_visible_items_markdown_only() {
        let temp = TempDir::new().unwrap();
        let root = setup_tree(&temp);
        let expanded = HashSet::new();

        let visible = compute_visible_items(&root, &expanded, false);

        // Directories are always shown, non-markdown files are hidden
        let names: Vec<&str> = visible
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["alpha", "beta", "notes.md", "readme.md"]);
    }

    #[test]
    fn test_visible_items_show_all() {
        let temp = TempDir::new().unwrap();
        let root = setup_tree(&temp);
        let expanded = HashSet::new();

        let visible = compute_visible_items(&root, &expanded, true);

        let names: Vec<&str> = visible
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(
            names,
            vec!["alpha", "beta", "image.png", "notes.md", "readme.md"]
        );
    }

    #[test]
    fn test_visible_items_expanded_dir() {
        let temp = TempDir::new().unwrap();
        let root = setup_tree(&temp);
        let mut expanded = HashSet::new();
        expanded.insert(root.join("alpha"));

        let visible = compute_visible_items(&root, &expanded, false);

        let names: Vec<&str> = visible
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        // alpha is expanded, so a1.md is visible (a2.txt is hidden because not markdown)
        assert_eq!(
            names,
            vec!["alpha", "a1.md", "beta", "notes.md", "readme.md"]
        );
    }

    #[test]
    fn test_visible_items_expanded_show_all() {
        let temp = TempDir::new().unwrap();
        let root = setup_tree(&temp);
        let mut expanded = HashSet::new();
        expanded.insert(root.join("alpha"));

        let visible = compute_visible_items(&root, &expanded, true);

        let names: Vec<&str> = visible
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(
            names,
            vec![
                "alpha",
                "a1.md",
                "a2.txt",
                "beta",
                "image.png",
                "notes.md",
                "readme.md"
            ]
        );
    }

    #[test]
    fn test_move_cursor_down_empty() {
        assert_eq!(move_cursor_down(None, &[]), None);
    }

    #[test]
    fn test_move_cursor_down_from_none() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(move_cursor_down(None, &items), Some(PathBuf::from("/a")));
    }

    #[test]
    fn test_move_cursor_down_normal() {
        let items = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        assert_eq!(
            move_cursor_down(Some(Path::new("/a")), &items),
            Some(PathBuf::from("/b"))
        );
    }

    #[test]
    fn test_move_cursor_down_at_bottom() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(
            move_cursor_down(Some(Path::new("/b")), &items),
            Some(PathBuf::from("/b"))
        );
    }

    #[test]
    fn test_move_cursor_down_stale() {
        let items = vec![PathBuf::from("/a")];
        assert_eq!(move_cursor_down(Some(Path::new("/gone")), &items), None);
    }

    #[test]
    fn test_move_cursor_up_empty() {
        assert_eq!(move_cursor_up(None, &[]), None);
    }

    #[test]
    fn test_move_cursor_up_from_none() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(move_cursor_up(None, &items), Some(PathBuf::from("/b")));
    }

    #[test]
    fn test_move_cursor_up_normal() {
        let items = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        assert_eq!(
            move_cursor_up(Some(Path::new("/c")), &items),
            Some(PathBuf::from("/b"))
        );
    }

    #[test]
    fn test_move_cursor_up_at_top() {
        let items = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(
            move_cursor_up(Some(Path::new("/a")), &items),
            Some(PathBuf::from("/a"))
        );
    }

    #[test]
    fn test_find_parent_item() {
        assert_eq!(
            find_parent_item(Path::new("/root/alpha/a1.md"), Path::new("/root")),
            Some(PathBuf::from("/root/alpha"))
        );
    }

    #[test]
    fn test_find_parent_item_top_level() {
        // Direct child of root has no parent in the tree
        assert_eq!(
            find_parent_item(Path::new("/root/alpha"), Path::new("/root")),
            None
        );
    }
}
