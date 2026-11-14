//! The directories an MCP client may open things under.

use std::path::{Path, PathBuf};

/// Where `arto mcp` was told it may open files and directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    roots: Vec<PathBuf>,
    base: PathBuf,
}

/// Why a path was not opened.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refusal {
    #[error("{} does not exist", .0.display())]
    Missing(PathBuf),
    #[error("{} is outside the directories this server may open ({})", .path.display(), list(.roots))]
    Outside { path: PathBuf, roots: Vec<PathBuf> },
}

fn list(roots: &[PathBuf]) -> String {
    roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

impl Roots {
    /// The roots given, or `base` alone when none were. Relative roots, and
    /// relative paths asked for later, are read against `base`.
    ///
    /// A root that does not exist is kept as written rather than dropped:
    /// it admits nothing, and leaving it out would quietly fall back to
    /// `base`, which the user did not ask for.
    pub fn new(roots: impl IntoIterator<Item = PathBuf>, base: impl Into<PathBuf>) -> Self {
        let base = base.into();
        let mut roots: Vec<PathBuf> = roots
            .into_iter()
            .map(|root| canonical_or_absolute(&base, &root))
            .collect();
        if roots.is_empty() {
            roots.push(canonical_or_absolute(&base, Path::new(".")));
        }
        Self { roots, base }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// The path to hand Arto, if it is one this server may open.
    ///
    /// Compared after resolving symlinks, so a link inside a root that
    /// points outside it is refused, and `..` cannot climb out.
    pub fn admit(&self, path: impl AsRef<Path>) -> Result<PathBuf, Refusal> {
        let asked = self.base.join(path.as_ref());
        let canonical = asked
            .canonicalize()
            .map_err(|_| Refusal::Missing(asked.clone()))?;
        if self.roots.iter().any(|root| canonical.starts_with(root)) {
            Ok(canonical)
        } else {
            Err(Refusal::Outside {
                path: canonical,
                roots: self.roots.clone(),
            })
        }
    }
}

fn canonical_or_absolute(base: &Path, path: &Path) -> PathBuf {
    let joined = base.join(path);
    joined.canonicalize().unwrap_or(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree {
        _temp: tempfile::TempDir,
        root: PathBuf,
    }

    /// `docs/guide.md` inside the root, `secret.md` beside it.
    fn tree() -> Tree {
        let temp = tempfile::tempdir().unwrap();
        let top = temp.path().canonicalize().unwrap();
        let root = top.join("project");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/guide.md"), "# Guide").unwrap();
        std::fs::write(top.join("secret.md"), "# Secret").unwrap();
        Tree { _temp: temp, root }
    }

    #[test]
    fn a_file_under_a_root_is_admitted_as_its_canonical_path() {
        let tree = tree();
        let roots = Roots::new([tree.root.clone()], "/");
        assert_eq!(
            roots.admit(tree.root.join("docs/./guide.md")),
            Ok(tree.root.join("docs/guide.md"))
        );
    }

    #[test]
    fn a_directory_under_a_root_is_admitted() {
        let tree = tree();
        let roots = Roots::new([tree.root.clone()], "/");
        assert_eq!(
            roots.admit(tree.root.join("docs")),
            Ok(tree.root.join("docs"))
        );
    }

    #[test]
    fn a_path_outside_every_root_is_refused_with_the_roots_named() {
        let tree = tree();
        let roots = Roots::new([tree.root.clone()], "/");
        let outside = tree.root.parent().unwrap().join("secret.md");
        let refusal = roots.admit(&outside).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Outside {
                path: outside,
                roots: vec![tree.root.clone()],
            }
        );
        assert!(
            refusal
                .to_string()
                .contains(&tree.root.display().to_string()),
            "{refusal}"
        );
    }

    #[test]
    fn climbing_out_with_dot_dot_is_refused() {
        let tree = tree();
        let roots = Roots::new([tree.root.clone()], "/");
        assert!(matches!(
            roots.admit(tree.root.join("docs/../../secret.md")),
            Err(Refusal::Outside { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_a_root_that_points_outside_is_refused() {
        let tree = tree();
        let link = tree.root.join("docs/link.md");
        std::os::unix::fs::symlink(tree.root.parent().unwrap().join("secret.md"), &link).unwrap();
        let roots = Roots::new([tree.root.clone()], "/");
        assert!(matches!(roots.admit(&link), Err(Refusal::Outside { .. })));
    }

    #[test]
    fn a_sibling_whose_name_starts_like_the_root_is_outside_it() {
        // `/a/project-old` begins with the characters of `/a/project`, and
        // is still not inside it.
        let tree = tree();
        let sibling = tree.root.with_file_name("project-old");
        std::fs::create_dir_all(&sibling).unwrap();
        let roots = Roots::new([tree.root.clone()], "/");
        assert!(matches!(
            roots.admit(&sibling),
            Err(Refusal::Outside { .. })
        ));
    }

    #[test]
    fn a_missing_path_is_refused_as_missing() {
        let tree = tree();
        let roots = Roots::new([tree.root.clone()], "/");
        assert!(matches!(
            roots.admit(tree.root.join("nope.md")),
            Err(Refusal::Missing(_))
        ));
    }

    #[test]
    fn with_no_roots_the_base_directory_is_the_only_one() {
        let tree = tree();
        let roots = Roots::new([], &tree.root);
        assert_eq!(roots.roots(), std::slice::from_ref(&tree.root));
    }

    #[test]
    fn relative_paths_are_read_against_the_base_directory() {
        let tree = tree();
        let roots = Roots::new([PathBuf::from("docs")], &tree.root);
        assert_eq!(roots.roots(), [tree.root.join("docs")]);
        assert_eq!(
            roots.admit("docs/guide.md"),
            Ok(tree.root.join("docs/guide.md"))
        );
    }
}
