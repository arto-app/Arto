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

use crate::utils::paths::true_spelling;
use std::path::{Path, PathBuf};

/// How many roots of its own one window keeps.
///
/// One. A window is somewhere: the folder it is working in, beside the places
/// that are always there. Keeping several turned "where am I" into a list to
/// read, and the answer to it changed shape every time a document was opened
/// outside the last one. Reaching for another folder moves the window to it.
pub const MAX_TEMPS: usize = 1;

/// Why a directory is being reached for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A side effect of opening a document. The reader asked for the
    /// document, not for a folder, so the window stays where it is when it is
    /// already in one that holds the document — and moves to the document's
    /// own when it is not.
    Implicit,
    /// A folder someone pointed at — dropped, typed, or chosen from a menu.
    /// Pointing at it is the intent, so it is added even when a place or a
    /// wider root already covers it; only the folder the window is already in
    /// is refused.
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
/// `temps` is in the order they joined, which is the order they are drawn in
/// and the order they are dropped in once the list is full. Reading a document
/// does not move its root: a list that reordered itself every time something
/// was opened would make the tree a different shape after every click, and a
/// reader looking for the folder they were in a moment ago would have to find
/// it again. What being read protects is the root itself — it is never the one
/// evicted — not its place in the list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Roots {
    places: Vec<PathBuf>,
    temps: Vec<PathBuf>,
}

impl Roots {
    pub fn new(places: Vec<PathBuf>, temps: Vec<PathBuf>) -> Self {
        // The places arrive from a list that repairs its own spellings; the
        // temporaries arrive from a state file written before that was true.
        Self {
            places,
            temps: temps.iter().map(|temp| true_spelling(temp)).collect(),
        }
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

    /// Replace the places.
    ///
    /// The window's own folder is left alone, even when a place now covers it.
    /// The two lists answer different questions — "folders I keep" and "where
    /// this window is" — and starring the folder you are working in answers
    /// the first without changing the answer to the second. Taking it out of
    /// the window's list on the grounds that it appears elsewhere left the
    /// window claiming to be nowhere.
    pub fn set_places(&mut self, mut places: Vec<PathBuf>) {
        // Never the same folder twice: the tree draws one row per root and
        // tells them apart by their path, so a repeat is two rows that cannot
        // be told apart.
        let mut seen = std::collections::HashSet::new();
        places.retain(|place| seen.insert(place.clone()));
        self.places = places;
    }

    /// The deepest root covering `target`, if any.
    ///
    /// Deepest rather than first: with both a repository and one of its
    /// subdirectories open, a document inside the subdirectory belongs to the
    /// narrower of the two.
    pub fn covering(&self, target: &Path) -> Option<&PathBuf> {
        deepest(self.all(), target)
    }

    /// The window's own folder, if it holds `target`.
    ///
    /// One folder, so this is a test rather than a search — but it is written
    /// the same way, because the ceiling on temporaries is a number and not a
    /// law of the module.
    fn covering_temp(&self, target: &Path) -> Option<&PathBuf> {
        deepest(self.temps.iter(), target)
    }

    /// The whole rule for whether a directory joins the tree.
    ///
    /// `target` is the document being opened for [`Origin::Implicit`], and the
    /// directory being pointed at for [`Origin::Explicit`]. It is passed as
    /// the reader spells it: matching folds the spellings away
    /// ([`canonical_key`]), and what is stored keeps the name the folder
    /// actually has, since that name is what the tree draws.
    pub fn decide(&self, target: &Path, origin: Origin) -> Decision {
        match origin {
            // Against the window's own folder, not against the bookmarks. A
            // bookmark is somewhere the reader keeps; being asked to read
            // something inside one does not make the window's answer to
            // "where am I" that bookmark, and leaving the window in a folder
            // the document is not in made the tree describe somewhere else.
            Origin::Implicit => match self.covering_temp(target) {
                Some(root) => Decision::Reveal {
                    root: root.to_path_buf(),
                },
                None => self.push_of(folder_of(target)),
            },
            // Against this window's own folder, not against the places. The
            // places are shortcuts every window carries; being asked to work
            // in one of them is still being asked to move, and answering "it
            // is already on the list" leaves the button that said "change this
            // window's folder" doing nothing.
            Origin::Explicit => {
                if let Some(root) = self.temps.iter().find(|temp| same(temp, target)) {
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
        // Spelled by the folder rather than by the caller: this is the name
        // the tree draws, and the one its children have to agree with.
        let root = true_spelling(&root);
        // Widening swallows what it now contains; narrowing keeps both, since
        // that is someone asking to concentrate on a part of what is already
        // there.
        let absorbed = self
            .temps
            .iter()
            .filter(|temp| !same(temp, &root) && is_under(temp, &root))
            .cloned()
            .collect();
        Decision::Push { root, absorbed }
    }

    /// Carry out a decision, and report the roots that fell off the end.
    ///
    /// Revealing an existing root leaves the list exactly as it was: it is
    /// already there, and where it sits is not something opening a document
    /// has an opinion about.
    pub fn apply(&mut self, decision: &Decision) -> Vec<PathBuf> {
        match decision {
            Decision::Reveal { .. } => Vec::new(),
            Decision::Push { root, absorbed } => {
                self.temps.retain(|temp| !absorbed.contains(temp));
                self.temps.retain(|temp| !same(temp, root));
                self.temps.push(root.clone());
                self.evict_overflow(root)
            }
        }
    }

    /// Drop the oldest roots until the list fits, never the one just reached.
    fn evict_overflow(&mut self, keep: &Path) -> Vec<PathBuf> {
        let mut evicted = Vec::new();
        while self.temps.len() > MAX_TEMPS {
            let Some(index) = self.temps.iter().position(|temp| !same(temp, keep)) else {
                break;
            };
            evicted.push(self.temps.remove(index));
        }
        evicted
    }
}

/// Where the window is when it opens a document nothing already holds.
///
/// The folder the document is in, and nothing cleverer. Walking up for a
/// project marker put the window at the top of a repository, which reads as
/// the window having gone somewhere the reader did not ask for — and when
/// that repository is bookmarked, at a folder already drawn a few rows above.
fn folder_of(file: &Path) -> PathBuf {
    file.parent().unwrap_or(file).to_path_buf()
}

/// One spelling per directory, so containment and equality can be trusted.
///
/// For comparison only — never for storage. The lowercasing below would put
/// `Arto` on screen as `arto`, so a root keeps the path it arrived with and
/// this is applied to both sides of a comparison instead.
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

/// The deepest of `roots` that holds `target`.
///
/// Deepest rather than first: with both a repository and one of its
/// subdirectories open, a document inside the subdirectory belongs to the
/// narrower of the two.
fn deepest<'a>(roots: impl Iterator<Item = &'a PathBuf>, target: &Path) -> Option<&'a PathBuf> {
    roots
        .filter(|root| is_under(target, root))
        .max_by_key(|root| root.components().count())
}

/// Whether two paths name the same directory, however each is spelled.
fn same(a: &Path, b: &Path) -> bool {
    canonical_key(a) == canonical_key(b)
}

/// Whether `path` is `root` or sits below it.
fn is_under(path: &Path, root: &Path) -> bool {
    let path = canonical_key(path);
    let root = canonical_key(root);
    path == root || path.starts_with(&root)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_window_lands_in_the_documents_own_folder() {
        // Not at the top of the repository holding it: that is somewhere the
        // reader did not ask to go, and it is often a bookmark as well.
        let roots = roots(&[], &[]);
        assert_eq!(
            roots.decide(&p("/w/arto/docs/cli.md"), Origin::Implicit),
            Decision::Push {
                root: p("/w/arto/docs"),
                absorbed: Vec::new(),
            }
        );
    }

    #[test]
    fn implicit_inside_the_windows_own_folder_only_reveals() {
        let roots = roots(&[], &["/w/arto"]);
        assert_eq!(
            roots.decide(&p("/w/arto/docs/cli.md"), Origin::Implicit),
            Decision::Reveal { root: p("/w/arto") }
        );
    }

    #[test]
    fn implicit_outside_it_moves_the_window_to_the_document() {
        let roots = roots(&[], &["/w/arto"]);
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
    fn implicit_inside_a_bookmark_still_moves_the_window() {
        // A bookmark is somewhere kept, not where the window is. Reading
        // inside one has to leave the tree describing the document.
        let roots = roots(&["/w/arto"], &[]);
        assert_eq!(
            roots.decide(&p("/w/arto/docs/cli.md"), Origin::Implicit),
            Decision::Push {
                root: p("/w/arto/docs"),
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
    fn explicit_on_the_folder_the_window_is_in_only_reveals() {
        let roots = roots(&[], &["/w/arto"]);
        assert_eq!(
            roots.decide(&p("/w/arto"), Origin::Explicit),
            Decision::Reveal { root: p("/w/arto") }
        );
    }

    #[test]
    fn explicit_on_a_place_still_moves_the_window_into_it() {
        // Being on the list of places is not being where the window is, so
        // asking to work in one has to move it there.
        let roots = roots(&["/w/arto"], &[]);
        assert_eq!(
            roots.decide(&p("/w/arto"), Origin::Explicit),
            Decision::Push {
                root: p("/w/arto"),
                absorbed: Vec::new(),
            }
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
    fn one_row_per_folder_however_often_it_is_listed() {
        let mut roots = roots(&[], &[]);
        roots.set_places(vec![p("/w/arto"), p("/w/notes"), p("/w/arto")]);

        assert_eq!(roots.places(), [p("/w/arto"), p("/w/notes")]);
    }

    #[test]
    fn a_new_place_leaves_the_window_where_it_is() {
        let mut roots = roots(&[], &["/w/arto/docs"]);
        roots.set_places(vec![p("/w/arto")]);

        // Starring a folder answers "folders I keep"; it does not move the
        // window out of the one it is working in, even that one's parent.
        assert_eq!(roots.places(), [p("/w/arto")]);
        assert_eq!(roots.temps(), [p("/w/arto/docs")]);
    }

    // === apply(): ordering and the ceiling ===

    #[test]
    fn revealing_a_temp_leaves_the_order_alone() {
        let mut roots = roots(&[], &["/a", "/b", "/c"]);
        let decision = roots.decide(&p("/a/file.md"), Origin::Implicit);
        roots.apply(&decision);
        assert_eq!(roots.temps(), [p("/a"), p("/b"), p("/c")]);
    }

    #[test]
    fn a_window_is_in_one_folder_at_a_time() {
        let mut roots = Roots::default();
        let first = roots.decide(&p("/first"), Origin::Explicit);
        roots.apply(&first);

        let second = roots.decide(&p("/second"), Origin::Explicit);
        let dropped = roots.apply(&second);

        assert_eq!(dropped, vec![p("/first")]);
        assert_eq!(roots.temps(), [p("/second")]);
    }

    #[test]
    fn the_folder_just_reached_for_is_the_one_kept() {
        let mut roots = Roots::default();
        roots.apply(&roots.decide(&p("/before"), Origin::Explicit).clone());

        // Opening a document no root covers moves the window to its folder.
        let opened = roots.decide(&p("/elsewhere/notes/today.md"), Origin::Implicit);
        roots.apply(&opened);

        assert_eq!(roots.temps(), [p("/elsewhere/notes")]);
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
        use std::fs;
        use tempfile::TempDir;

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
    fn the_walkthrough_from_the_specification() {
        let mut roots = Roots::new(vec![p("/src/arto"), p("/notes")], Vec::new());

        // 1. A document inside a place: the window moves to the document's
        //    own folder, because a place is not where the window is.
        let step = roots.decide(&p("/src/arto/docs/cli.md"), Origin::Implicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/src/arto/docs")]);

        // 2. A document outside it: its folder becomes where the window is.
        let step = roots.decide(&p("/work/handbook/README.md"), Origin::Implicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/work/handbook")]);

        // 3. Another, elsewhere: the window moves rather than collecting.
        let step = roots.decide(&p("/downloads/spec.md"), Origin::Implicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/downloads")]);

        // 4. A folder pointed at is where the window goes.
        let step = roots.decide(&p("/work/handbook/docs"), Origin::Explicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/work/handbook/docs")]);

        // 5. Its ancestor, which swallows it.
        let step = roots.decide(&p("/work"), Origin::Explicit);
        roots.apply(&step);
        assert_eq!(roots.temps(), [p("/work")]);

        // 6. Starring the folder the window is in adds it to the places and
        //    leaves the window where it is.
        roots.set_places(vec![p("/src/arto"), p("/notes"), p("/work")]);
        assert_eq!(roots.temps(), [p("/work")]);
    }
}
