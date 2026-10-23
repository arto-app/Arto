//! The files under the folder a window is working in.
//!
//! The palette can offer the history, what is starred and the places kept,
//! because those are short lists the app already holds. What it could not
//! offer is the document that has never been opened — and that is most of a
//! repository. This is that list: everything under the window's own folder,
//! read once and kept, so a query can be answered from memory rather than
//! from the disk.
//!
//! Three things keep it from being expensive:
//!
//! - **It is read in the background.** [`ensure`] starts a scan on a thread
//!   of its own and returns; the palette draws what it has and redraws when
//!   [`FILES_CHANGED`] says there is more. Nothing waits on the disk.
//! - **It is bounded three ways.** [`MAX_FILES`] kept, [`MAX_VISITED`] looked
//!   at, and [`TIME_BUDGET`] to do it in — because a folder can be too much
//!   in three different ways, and a bound on what is kept is no bound at all
//!   on a million files holding twelve documents. A reader who points a
//!   window at their home directory gets a list that says it is partial.
//! - **It skips what a reader would not open.** Hidden files, and everything
//!   `.gitignore` (or `.ignore`) excludes — the same rule `ripgrep` follows,
//!   through the same crate — so `node_modules` and `target` cost nothing.
//!
//! What one scan produces is a [`Listing`], and a few of them are kept at
//! once: one window's folder is not another's, and moving between windows
//! should not re-read the disk.

use crate::utils::file::is_markdown_file;
use crate::utils::kept_index::{self, KeptIndex};
use ignore::{WalkBuilder, WalkState};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// How many files one folder contributes.
///
/// Enough for any repository and most of the folders people keep documents
/// in; a bound rather than a limit anyone is expected to reach. What it
/// really guards is the folder nobody meant to index — a home directory, a
/// mounted drive — where the list would otherwise be too long to be a list.
pub const MAX_FILES: usize = 20_000;

/// How many files the walk will look at, whether or not it keeps them.
///
/// [`MAX_FILES`] alone does not bound the walk: a folder of a million files
/// holding twelve Markdown documents never reaches it, and the walk goes on
/// to the end. This is the bound that does not depend on what is found —
/// and it is the one that matters on a network mount, where every directory
/// is a round trip.
const MAX_VISITED: usize = 400_000;

/// How long the walk is allowed to take.
///
/// The last resort, for the filesystem that answers slowly rather than the
/// tree that is large: neither bound above is reached, and the thread would
/// otherwise still be walking when the reader has given up and typed the
/// path by hand.
const TIME_BUDGET: Duration = Duration::from_secs(10);

/// How many folders are remembered at once.
///
/// One per window, near enough: each window is in one folder, and moving
/// between two windows is the case worth not re-reading the disk for.
const MAX_LISTINGS: usize = 4;

/// How long a listing is trusted before the next look asks for a fresh one.
///
/// A file written while the palette is closed should be findable soon after,
/// and re-reading a folder costs a background thread that nothing waits on.
/// The stale listing is what answers in the meantime, so the delay is never
/// something the reader sits through.
const FRESH_FOR: Duration = Duration::from_secs(30);

/// What one scan found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// The folder that was read.
    pub root: PathBuf,
    /// Whether everything was listed, or only what Arto renders.
    pub all_files: bool,
    /// The files, shallowest first, then by path.
    pub files: Vec<PathBuf>,
    /// Whether the walk was cut short — by any of its three bounds, so the
    /// folder holds files this listing does not name.
    pub truncated: bool,
}

impl Listing {
    /// A file's path relative to the folder it was found under.
    ///
    /// What the query is matched against: the folder is the same for every
    /// row, so the characters of its name are noise in the middle of the
    /// haystack — and worse than noise once they start scoring.
    pub fn relative<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}

/// The listings by folder and whether everything was listed, and the scans
/// in flight.
type Index = KeptIndex<(PathBuf, bool), Arc<Listing>>;

static INDEX: LazyLock<RwLock<Index>> = LazyLock::new(|| RwLock::new(Index::new(MAX_LISTINGS)));

/// Announced whenever a scan finishes, so an open palette redraws.
pub static FILES_CHANGED: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(16).0);

/// The files under `root`, as far as they have been read.
///
/// [`None`] until the first scan of that folder finishes. It never blocks and
/// it never reads the disk: this is what the palette draws from, once per
/// keystroke.
pub fn listing(root: &Path, all_files: bool) -> Option<Arc<Listing>> {
    INDEX
        .read()
        .get(&(root.to_path_buf(), all_files))
        .map(Arc::clone)
}

/// Read `root` if it has not been read lately, on a thread of its own.
///
/// Called as the palette opens rather than as the app starts: a window that
/// is never asked to find a file should never have walked a directory for
/// it. Calling it again while a scan is running, or while the last one is
/// still fresh, does nothing.
pub fn ensure(root: &Path, all_files: bool) {
    let Some(mark) = INDEX
        .write()
        .begin((root.to_path_buf(), all_files), FRESH_FOR, |_| true)
    else {
        return;
    };
    kept_index::spawn(&INDEX, mark, &FILES_CHANGED, |scanning| {
        let (root, all_files) = scanning.key();
        let started = Instant::now();
        let listing = scan(root, *all_files);
        tracing::debug!(
            ?root,
            files = listing.files.len(),
            truncated = listing.truncated,
            elapsed = ?started.elapsed(),
            "Listed the files under a folder"
        );
        Some(Arc::new(listing))
    });
}

/// Forget everything read so far, so the next look reads the disk again.
///
/// For the reader who has just asked for the tree to be reloaded: the palette
/// is a window on the same folders, and answering from a listing taken before
/// the reload would be the one place the app still showed the old files.
///
/// A walk that is already under way was reading the disk as it was before
/// the reload, so what it is about to come back with is forgotten too — the
/// era it started in has passed. Otherwise the one thing a reload could not
/// refresh would be the folder that happened to be being read as it was
/// asked for, and it would look freshly read for the next half minute.
pub fn forget() {
    INDEX.write().forget();
}

/// What one walk is allowed to spend.
///
/// Three bounds rather than one, because a walk can be too much in three
/// different ways: too many documents to list, too many files to look
/// through to find them, or too slow a filesystem for either count to be
/// reached before the reader has given up. A field rather than a constant
/// read straight from the walk so that the rule can be tested against a
/// budget small enough to reach.
#[derive(Debug, Clone, Copy)]
struct Budget {
    /// How many files to keep.
    files: usize,
    /// How many to look at, kept or not.
    visited: usize,
    /// How long to spend looking.
    time: Duration,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            files: MAX_FILES,
            visited: MAX_VISITED,
            time: TIME_BUDGET,
        }
    }
}

/// Walk `root` and collect the files worth offering.
fn scan(root: &Path, all_files: bool) -> Listing {
    walk(root, all_files, Budget::default())
}

/// The walk itself, within `budget`.
///
/// In parallel, because the cost of a large tree is the waiting on the
/// filesystem rather than the work, and bounded, because the tree can always
/// turn out to be larger than anyone meant.
fn walk(root: &Path, all_files: bool, budget: Budget) -> Listing {
    let tally = Tally::new(all_files, budget);
    walker(root).run(|| Box::new(|entry| tally.visit(entry)));
    let truncated = tally.truncated.load(Ordering::Relaxed);

    let mut files = tally.found.into_inner();
    // Shallowest first, and alphabetical within a depth. Nothing here decides
    // what the reader sees — the query's score does — but two candidates that
    // answer a query equally well should come out in the same order every
    // time, and the shallower of two identical names is the likelier one.
    files.sort_by_cached_key(|path| (path.components().count(), path.clone()));
    files.truncate(budget.files);

    Listing {
        root: root.to_path_buf(),
        all_files,
        files,
        truncated,
    }
}

/// The walk over `root`, skipping what a reader would not open.
fn walker(root: &Path) -> ignore::WalkParallel {
    WalkBuilder::new(root)
        // What a reader would not have opened by hand. `.git` and the rest of
        // the dotted world are noise in a list of documents, and the ignore
        // files are the project's own statement of what is not source.
        .hidden(true)
        .parents(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // A folder opened on its own is often a part of a repository rather
        // than the whole of one, and its `.gitignore` still says what is not
        // worth listing even when the `.git` directory is somewhere above.
        .require_git(false)
        // A link out of the tree is another tree, and following it is how a
        // walk goes round in a circle.
        .follow_links(false)
        .threads(threads())
        .build_parallel()
}

/// What the walk's threads share: what was kept, what was looked at, and
/// whether the budget ran out before the tree did.
struct Tally {
    all_files: bool,
    budget: Budget,
    deadline: Instant,
    found: Mutex<Vec<PathBuf>>,
    count: AtomicUsize,
    visited: AtomicUsize,
    truncated: AtomicBool,
}

impl Tally {
    fn new(all_files: bool, budget: Budget) -> Self {
        Self {
            all_files,
            budget,
            deadline: Instant::now() + budget.time,
            found: Mutex::new(Vec::new()),
            count: AtomicUsize::new(0),
            visited: AtomicUsize::new(0),
            truncated: AtomicBool::new(false),
        }
    }

    fn visit(&self, entry: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
        let Ok(entry) = entry else {
            return WalkState::Continue;
        };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            return WalkState::Continue;
        }
        // Counted before anything is decided about the file, because this is
        // the bound on the walk rather than on the list. The clock is read
        // once every so often: asking it per file would cost more than
        // looking at the file.
        let seen = self.visited.fetch_add(1, Ordering::Relaxed);
        let spent = seen.is_multiple_of(1024) && Instant::now() >= self.deadline;
        if seen >= self.budget.visited || spent {
            return self.stop();
        }
        let path = entry.into_path();
        if !self.all_files && !is_markdown_file(&path) {
            return WalkState::Continue;
        }
        if self.count.fetch_add(1, Ordering::Relaxed) >= self.budget.files {
            return self.stop();
        }
        self.found.lock().push(path);
        WalkState::Continue
    }

    fn stop(&self) -> WalkState {
        self.truncated.store(true, Ordering::Relaxed);
        WalkState::Quit
    }
}

/// How many threads the walk runs on.
///
/// Enough to keep the filesystem busy, capped so that a machine with many
/// cores does not answer a palette keystroke by starting a small storm.
pub(crate) fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().min(8))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A folder with a document, a nested one, something ignored and
    /// something hidden.
    fn tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::write(root.join("notes.txt"), "").unwrap();
        fs::write(root.join("docs/guide.md"), "").unwrap();
        fs::write(root.join("target/built.md"), "").unwrap();
        fs::write(root.join(".hidden/secret.md"), "").unwrap();
        fs::write(root.join(".ignore"), "target\n").unwrap();
        dir
    }

    fn names(listing: &Listing) -> Vec<String> {
        listing
            .files
            .iter()
            .map(|path| listing.relative(path).to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_scan_finds_the_documents_and_leaves_the_rest() {
        let dir = tree();
        let listing = scan(dir.path(), false);

        let mut found = names(&listing);
        found.sort();
        assert_eq!(
            found,
            vec![
                "README.md".to_string(),
                format!("docs{}guide.md", std::path::MAIN_SEPARATOR),
            ]
        );
        assert!(!listing.truncated);
    }

    #[test]
    fn everything_means_everything_that_is_not_ignored() {
        let dir = tree();
        let listing = scan(dir.path(), true);

        let mut found = names(&listing);
        found.sort();
        assert!(found.contains(&"notes.txt".to_string()));
        assert!(!found.iter().any(|name| name.contains("built.md")));
        assert!(!found.iter().any(|name| name.contains("secret.md")));
    }

    #[test]
    fn a_list_cut_short_says_so() {
        let dir = tree();
        let listing = walk(
            dir.path(),
            true,
            Budget {
                files: 1,
                ..Budget::default()
            },
        );

        assert_eq!(listing.files.len(), 1);
        assert!(listing.truncated);
    }

    #[test]
    fn a_walk_is_bounded_by_what_it_looks_at_and_not_only_by_what_it_keeps() {
        // Nothing here is Markdown, so the file bound is never approached;
        // what stops the walk is having looked at enough.
        let dir = TempDir::new().unwrap();
        for at in 0..8 {
            fs::write(dir.path().join(format!("note-{at}.txt")), "").unwrap();
        }

        let listing = walk(
            dir.path(),
            false,
            Budget {
                visited: 2,
                ..Budget::default()
            },
        );

        assert!(listing.files.is_empty());
        assert!(listing.truncated);
    }

    #[test]
    fn a_walk_that_runs_out_of_time_stops() {
        let dir = tree();
        let listing = walk(
            dir.path(),
            true,
            Budget {
                time: Duration::ZERO,
                ..Budget::default()
            },
        );

        assert!(listing.truncated);
    }

    #[test]
    fn the_shallowest_come_first() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/b/deep.md"), "").unwrap();
        fs::write(dir.path().join("top.md"), "").unwrap();

        let listing = scan(dir.path(), false);
        assert_eq!(names(&listing).first().unwrap(), "top.md");
    }

    #[test]
    fn a_relative_path_drops_the_folder_every_row_shares() {
        let listing = Listing {
            root: PathBuf::from("/w/arto"),
            all_files: false,
            files: Vec::new(),
            truncated: false,
        };
        assert_eq!(
            listing.relative(Path::new("/w/arto/docs/guide.md")),
            Path::new("docs/guide.md")
        );
        // Something from outside keeps the only name it has.
        assert_eq!(
            listing.relative(Path::new("/elsewhere/guide.md")),
            Path::new("/elsewhere/guide.md")
        );
    }
}
