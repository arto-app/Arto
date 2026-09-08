//! Which directories the file tree is rooted at, and how that set changes.
//!
//! A single root meant that going somewhere lost where you were: opening a
//! document elsewhere, or looking into another folder, replaced what the tree
//! was showing. Two kinds of root replace it, because the old one was really
//! carrying two different things at once:
//!
//! - **Places** are chosen deliberately, saved, and shared by every window.
//!   They are the directory bookmarks; this module only reads them.
//! - **Temporaries** belong to one window and last as long as it does. They
//!   arrive as a side effect of opening a document, or because someone
//!   dropped a folder or typed a path.
//!
//! Nothing here replaces anything: a new root is added, so browsing and
//! comparing are the same gesture. What changes is only *whether* a root is
//! added, which is the whole of [`Roots::decide`].

use std::path::{Path, PathBuf};

/// How many temporary roots one window keeps.
///
/// Each collapsed root is a single row, so the ceiling is about keeping the
/// list readable rather than saving memory.
pub const MAX_TEMPS: usize = 8;

/// Directories that mark the top of a body of work.
///
/// A document's own parent is a poor root — it strands the tree in a leaf
/// with none of its siblings in view — so the search walks up looking for one
/// of these. `.git` is a file rather than a directory inside a worktree or a
/// submodule, so existence is what matters, not kind.
const PROJECT_MARKERS: [&str; 4] = [".git", ".hg", ".svn", ".jj"];

/// Why a directory is being reached for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A side effect of opening a document. The reader asked for the
    /// document, not for the folder, so an existing root that already covers
    /// it is the answer.
    Implicit,
    /// A folder someone pointed at — dropped, typed, or chosen from a menu.
    /// Pointing at it is the intent, so it is added even when covered; only
    /// an exact duplicate is refused.
    Explicit,
}

/// What [`Roots::decide`] concluded. Applying it is a separate step so the
/// rule can be tested without any state changing hands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Already covered: expand this root down to the target instead.
    Reveal { root: PathBuf },
    /// Not covered: add this directory, dropping the temporaries it swallows.
    Push {
        root: PathBuf,
        /// Temporary roots below `root`, which it makes redundant.
        absorbed: Vec<PathBuf>,
    },
}

/// The roots one window is showing.
///
/// `temps` is ordered least- to most-recently-touched, which is also the
/// order they are drawn in and the order they are dropped in once the list is
/// full. Opening a document touches its root, so the root holding whatever is
/// being read is always at the end and can never be the one evicted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Roots {
    places: Vec<PathBuf>,
    temps: Vec<PathBuf>,
}

impl Roots {
    pub fn new(places: Vec<PathBuf>, temps: Vec<PathBuf>) -> Self {
        Self { places, temps }
    }

    pub fn places(&self) -> &[PathBuf] {
        &self.places
    }

    pub fn temps(&self) -> &[PathBuf] {
        &self.temps
    }

    /// Every root, places first, in the order the tree draws them.
    pub fn all(&self) -> impl Iterator<Item = &PathBuf> {
        self.places.iter().chain(self.temps.iter())
    }

    /// Replace the places, absorbing any temporary they now cover.
    ///
    /// A place outranks a temporary pointing into the same tree, so keeping
    /// both would only show the same directory twice.
    pub fn set_places(&mut self, places: Vec<PathBuf>) {
        self.places = places;
        let places = self.places.clone();
        self.temps
            .retain(|temp| !places.iter().any(|place| is_under(temp, place)));
    }

    /// The deepest root covering `target`, if any.
    ///
    /// Deepest rather than first: with both a repository and one of its
    /// subdirectories open, a document inside the subdirectory belongs to the
    /// narrower of the two.
    pub fn covering(&self, target: &Path) -> Option<&PathBuf> {
        self.all()
            .filter(|root| is_under(target, root))
            .max_by_key(|root| root.components().count())
    }

    /// The whole rule for whether a directory joins the tree.
    ///
    /// `target` is the document being opened for [`Origin::Implicit`], and the
    /// directory being pointed at for [`Origin::Explicit`]. Both must already
    /// be normalised by [`canonical_key`], or the same place stacks twice
    /// under two spellings.
    pub fn decide(&self, target: &Path, origin: Origin) -> Decision {
        match origin {
            Origin::Implicit => match self.covering(target) {
                Some(root) => Decision::Reveal {
                    root: root.to_path_buf(),
                },
                None => self.push_of(start_root_for(target)),
            },
            Origin::Explicit => {
                if let Some(root) = self.all().find(|root| root.as_path() == target) {
                    Decision::Reveal {
                        root: root.to_path_buf(),
                    }
                } else {
                    self.push_of(target.to_path_buf())
                }
            }
        }
    }

    fn push_of(&self, root: PathBuf) -> Decision {
        // Widening swallows what it now contains; narrowing keeps both, since
        // that is someone asking to concentrate on a part of what is already
        // there.
        let absorbed = self
            .temps
            .iter()
            .filter(|temp| temp.as_path() != root && is_under(temp, &root))
            .cloned()
            .collect();
        Decision::Push { root, absorbed }
    }

    /// Carry out a decision, and report the roots that fell off the end.
    ///
    /// Revealing an existing temporary counts as touching it, so it moves to
    /// the end and outlives the ones nobody has looked at.
    pub fn apply(&mut self, decision: &Decision) -> Vec<PathBuf> {
        match decision {
            Decision::Reveal { root } => {
                self.touch(root);
                Vec::new()
            }
            Decision::Push { root, absorbed } => {
                self.temps.retain(|temp| !absorbed.contains(temp));
                self.temps.retain(|temp| temp != root);
                self.temps.push(root.clone());
                self.evict_overflow()
            }
        }
    }

    /// Drop a temporary root. Places are removed by unbookmarking them.
    pub fn close_temp(&mut self, root: &Path) {
        self.temps.retain(|temp| temp.as_path() != root);
    }

    fn touch(&mut self, root: &Path) {
        if let Some(index) = self.temps.iter().position(|temp| temp.as_path() == root) {
            let root = self.temps.remove(index);
            self.temps.push(root);
        }
    }

    fn evict_overflow(&mut self) -> Vec<PathBuf> {
        if self.temps.len() <= MAX_TEMPS {
            return Vec::new();
        }
        let excess = self.temps.len() - MAX_TEMPS;
        self.temps.drain(..excess).collect()
    }
}

/// Where to root the tree for a document that no root covers.
///
/// Walks up for a project marker and settles for the document's parent when
/// there is none. A marker found at the home directory or at the top of a
/// volume is refused: a root that broad is not a root, it is everything.
pub fn start_root_for(file: &Path) -> PathBuf {
    let parent = file.parent().unwrap_or(file).to_path_buf();
    let home = dirs::home_dir();

    let mut candidate = Some(parent.as_path());
    while let Some(dir) = candidate {
        if PROJECT_MARKERS
            .iter()
            .any(|marker| dir.join(marker).exists())
        {
            let too_broad = dir.parent().is_none()
                || home.as_deref().is_some_and(|home| home == dir)
                || dir.as_os_str().is_empty();
            return if too_broad { parent } else { dir.to_path_buf() };
        }
        candidate = dir.parent();
    }

    parent
}

/// One spelling per directory, so containment and equality can be trusted.
///
/// Symlinks are resolved and `..` folded away where the path exists; where it
/// does not, the lexical form is the best available. On macOS the default
/// filesystem ignores case, so two spellings of one directory would otherwise
/// each get a root of their own.
pub fn canonical_key(path: &Path) -> PathBuf {
    let resolved = path
        .canonicalize()
        .unwrap_or_else(|_| lexically_normal(path));

    #[cfg(target_os = "macos")]
    {
        PathBuf::from(resolved.to_string_lossy().to_lowercase())
    }
    #[cfg(not(target_os = "macos"))]
    {
        resolved
    }
}

/// Fold `.` and `..` away without touching the filesystem.
fn lexically_normal(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Whether `path` is `root` or sits below it.
fn is_under(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn roots(places: &[&str], temps: &[&str]) -> Roots {
        Roots::new(
            places.iter().map(|s| p(s)).collect(),
            temps.iter().map(|s| p(s)).collect(),
        )
    }

    // === decide(): the four cases of the specification's table ===

    #[test]
    fn implicit_inside_a_root_only_reveals() {
        let roots = roots(&["/w/arto"], &[]);
        assert_eq!(
            roots.decide(&p("/w/arto/docs/cli.md"), Origin::Implicit),
            Decision::Reveal { root: p("/w/arto") }
        );
    }

    #[test]
    fn implicit_outside_every_root_pushes() {
        let roots = roots(&["/w/arto"], &[]);
        let decision = roots.decide(&p("/elsewhere/notes/today.md"), Origin::Implicit);
        assert_eq!(
            decision,
            Decision::Push {
                root: p("/elsewhere/notes"),
                absorbed: Vec::new(),
            }
        );
    }

    #[test]
    fn implicit_picks_the_deepest_root_that_covers_it() {
        let roots = roots(&["/w/arto"], &["/w/arto/docs"]);
        assert_eq!(
            roots.decide(&p("/w/arto/docs/cli.md"), Origin::Implicit),
            Decision::Reveal {
                root: p("/w/arto/docs")
            }
        );
    }

    #[test]
    fn explicit_pushes_even_when_already_covered() {
        let roots = roots(&["/w/arto"], &[]);
        assert_eq!(
            roots.decide(&p("/w/arto/docs"), Origin::Explicit),
            Decision::Push {
                root: p("/w/arto/docs"),
                absorbed: Vec::new(),
            }
        );
    }

    #[test]
    fn explicit_on_an_exact_duplicate_only_reveals() {
        let roots = roots(&[], &["/w/arto"]);
        assert_eq!(
            roots.decide(&p("/w/arto"), Origin::Explicit),
            Decision::Reveal { root: p("/w/arto") }
        );
    }

    // === nesting ===

    #[test]
    fn pushing_an_ancestor_absorbs_the_temps_below_it() {
        let roots = roots(&[], &["/w/arto/docs", "/w/arto/crates", "/other"]);
        let decision = roots.decide(&p("/w/arto"), Origin::Explicit);
        let Decision::Push { root, absorbed } = decision else {
            panic!("expected a push, got {decision:?}");
        };
        assert_eq!(root, p("/w/arto"));
        assert_eq!(absorbed, vec![p("/w/arto/docs"), p("/w/arto/crates")]);
    }

    #[test]
    fn pushing_a_descendant_keeps_both() {
        let roots = roots(&[], &["/w/arto"]);
        let decision = roots.decide(&p("/w/arto/docs"), Origin::Explicit);
        assert_eq!(
            decision,
            Decision::Push {
                root: p("/w/arto/docs"),
                absorbed: Vec::new(),
            }
        );
    }

    #[test]
    fn a_new_place_absorbs_the_temps_under_it() {
        let mut roots = roots(&[], &["/w/arto/docs", "/other"]);
        roots.set_places(vec![p("/w/arto")]);
        assert_eq!(roots.places(), [p("/w/arto")]);
        assert_eq!(roots.temps(), [p("/other")]);
    }

    // === apply(): ordering and the ceiling ===

    #[test]
    fn apply_moves_a_revealed_temp_to_the_end() {
        let mut roots = roots(&[], &["/a", "/b", "/c"]);
        let decision = roots.decide(&p("/a/file.md"), Origin::Implicit);
        roots.apply(&decision);
        assert_eq!(roots.temps(), [p("/b"), p("/c"), p("/a")]);
    }

    #[test]
    fn the_ceiling_drops_the_least_recently_touched() {
        let mut roots = Roots::default();
        for i in 0..MAX_TEMPS {
            let decision = roots.decide(&p(&format!("/r{i}")), Origin::Explicit);
            roots.apply(&decision);
        }
        assert_eq!(roots.temps().len(), MAX_TEMPS);

        let decision = roots.decide(&p("/newest"), Origin::Explicit);
        let dropped = roots.apply(&decision);

        assert_eq!(dropped, vec![p("/r0")]);
        assert_eq!(roots.temps().len(), MAX_TEMPS);
        assert_eq!(roots.temps().last(), Some(&p("/newest")));
    }

    #[test]
    fn the_root_being_read_is_never_the_one_dropped() {
        let mut roots = Roots::default();
        for i in 0..MAX_TEMPS {
            let decision = roots.decide(&p(&format!("/r{i}")), Origin::Explicit);
            roots.apply(&decision);
        }

        // Reading in the oldest root touches it, so the next push cannot take it.
        let read = roots.decide(&p("/r0/file.md"), Origin::Implicit);
        roots.apply(&read);
        let push = roots.decide(&p("/newest"), Origin::Explicit);
        let dropped = roots.apply(&push);

        assert_eq!(dropped, vec![p("/r1")]);
        assert!(roots.temps().contains(&p("/r0")));
    }

    #[test]
    fn closing_a_temp_removes_only_that_one() {
        let mut roots = roots(&["/place"], &["/a", "/b"]);
        roots.close_temp(&p("/a"));
        assert_eq!(roots.temps(), [p("/b")]);
        assert_eq!(roots.places(), [p("/place")]);
    }

    // === start_root_for() ===

    #[test]
    fn start_root_climbs_to_the_project_marker() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("arto");
        let deep = repo.join("docs/api");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();

        assert_eq!(start_root_for(&deep.join("auth.md")), repo);
    }

    #[test]
    fn start_root_accepts_a_marker_that_is_a_file() {
        // Worktrees and submodules write `.git` as a file, not a directory.
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("worktree");
        let deep = repo.join("docs");
        fs::create_dir_all(&deep).unwrap();
        fs::write(repo.join(".git"), "gitdir: /elsewhere\n").unwrap();

        assert_eq!(start_root_for(&deep.join("readme.md")), repo);
    }

    #[test]
    fn start_root_falls_back_to_the_parent_without_a_marker() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("downloads");
        fs::create_dir_all(&dir).unwrap();

        assert_eq!(start_root_for(&dir.join("spec.md")), dir);
    }

    #[test]
    fn start_root_refuses_the_top_of_a_volume() {
        // A marker at "/" would otherwise root the tree at the whole disk.
        assert_eq!(start_root_for(&p("/notes/today.md")), p("/notes"));
    }

    // === canonical_key() ===

    #[test]
    fn canonical_key_folds_parent_components() {
        let key = canonical_key(&p("/w/arto/docs/../crates"));
        assert_eq!(key, canonical_key(&p("/w/arto/crates")));
    }

    // Windows needs a privilege to create a symlink that CI does not grant,
    // so the check runs where symlinks are ordinary.
    #[cfg(unix)]
    #[test]
    fn canonical_key_resolves_a_symlink_to_its_target() {
        let temp = TempDir::new().unwrap();
        let real = temp.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert_eq!(canonical_key(&link), canonical_key(&real));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn canonical_key_ignores_case_on_macos() {
        assert_eq!(canonical_key(&p("/W/Arto")), canonical_key(&p("/w/arto")));
    }

    // === the specification's worked example, step by step ===

    #[test]
    fn the_six_step_walkthrough_from_the_specification() {
        let mut roots = Roots::new(vec![p("/src/arto"), p("/notes")], Vec::new());

        // 1. A document inside a place: nothing is added.
        let step = roots.decide(&p("/src/arto/docs/cli.md"), Origin::Implicit);
        assert!(matches!(step, Decision::Reveal { .. }));
        roots.apply(&step);
        assert!(roots.temps().is_empty());

        // 2. A document outside every root: its folder joins.
        let step = roots.decide(&p("/work/handbook/README.md"), Origin::Implicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/work/handbook")]);

        // 3. Another, elsewhere.
        let step = roots.decide(&p("/downloads/spec.md"), Origin::Implicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/work/handbook"), p("/downloads")]);

        // 4. A folder pointed at, inside one that is already there.
        let step = roots.decide(&p("/work/handbook/docs"), Origin::Explicit);
        roots.apply(&step);
        assert_eq!(
            roots.temps(),
            [
                p("/work/handbook"),
                p("/downloads"),
                p("/work/handbook/docs")
            ]
        );

        // 5. Their ancestor, which swallows both.
        let step = roots.decide(&p("/work"), Origin::Explicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/downloads"), p("/work")]);

        // 6. A temporary promoted to a place leaves the temporaries.
        roots.set_places(vec![p("/src/arto"), p("/notes"), p("/downloads")]);
        assert_eq!(roots.temps(), [p("/work")]);
    }
}
