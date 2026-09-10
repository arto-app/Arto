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
//! - **It is bounded.** [`MAX_FILES`] files, and the walk stops there. A
//!   reader who points a window at their home directory gets a list that is
//!   honest about being partial rather than a window that stops responding.
//! - **It skips what a reader would not open.** Hidden files, and everything
//!   `.gitignore` (or `.ignore`) excludes — the same rule `ripgrep` follows,
//!   through the same crate — so `node_modules` and `target` cost nothing.
//!
//! What one scan produces is a [`Listing`], and a few of them are kept at
//! once: one window's folder is not another's, and moving between windows
//! should not re-read the disk.

use crate::utils::file::is_markdown_file;
use ignore::{WalkBuilder, WalkState};
use parking_lot::{Mutex, RwLock};
use std::collections::HashSet;
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
/// mounted drive — where the walk would otherwise run for minutes and the
/// list would be too long to be a list.
pub const MAX_FILES: usize = 20_000;

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
    /// Whether [`MAX_FILES`] cut the walk short.
    pub truncated: bool,
}

impl Listing {
    /// Whether this listing answers a question about `root`.
    fn answers(&self, root: &Path, all_files: bool) -> bool {
        self.root == root && self.all_files == all_files
    }

    /// A file's path relative to the folder it was found under.
    ///
    /// What the query is matched against: the folder is the same for every
    /// row, so the characters of its name are noise in the middle of the
    /// haystack — and worse than noise once they start scoring.
    pub fn relative<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}

/// One listing and when it was read.
struct Kept {
    listing: Arc<Listing>,
    at: Instant,
}

/// The listings, and the scans in flight.
#[derive(Default)]
struct Index {
    kept: Vec<Kept>,
    /// The folders being read right now, so two looks do not start two scans.
    scanning: HashSet<(PathBuf, bool)>,
}

impl Index {
    /// Keep a finished listing, dropping the oldest once there are too many.
    ///
    /// The listing replaces any earlier one of the same folder, so a rescan
    /// updates in place rather than pushing the other folders out.
    fn remember(&mut self, listing: Listing) {
        self.scanning
            .remove(&(listing.root.clone(), listing.all_files));
        self.kept
            .retain(|kept| !kept.listing.answers(&listing.root, listing.all_files));
        self.kept.push(Kept {
            listing: Arc::new(listing),
            at: Instant::now(),
        });
        while self.kept.len() > MAX_LISTINGS {
            self.kept.remove(0);
        }
    }
}

static INDEX: LazyLock<RwLock<Index>> = LazyLock::new(|| RwLock::new(Index::default()));

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
        .kept
        .iter()
        .find(|kept| kept.listing.answers(root, all_files))
        .map(|kept| Arc::clone(&kept.listing))
}

/// Read `root` if it has not been read lately, on a thread of its own.
///
/// Called as the palette opens rather than as the app starts: a window that
/// is never asked to find a file should never have walked a directory for
/// it. Calling it again while a scan is running, or while the last one is
/// still fresh, does nothing.
pub fn ensure(root: &Path, all_files: bool) {
    let key = (root.to_path_buf(), all_files);
    {
        let mut index = INDEX.write();
        if index.scanning.contains(&key) {
            return;
        }
        let fresh = index
            .kept
            .iter()
            .any(|kept| kept.listing.answers(root, all_files) && kept.at.elapsed() < FRESH_FOR);
        if fresh {
            return;
        }
        index.scanning.insert(key.clone());
    }

    // A plain thread rather than a task: walking a directory is blocking work
    // from first call to last, and the runtime this would otherwise sit on is
    // the one drawing the window.
    std::thread::spawn(move || {
        let (root, all_files) = key;
        let started = Instant::now();
        let listing = scan(&root, all_files);
        tracing::debug!(
            ?root,
            files = listing.files.len(),
            truncated = listing.truncated,
            elapsed = ?started.elapsed(),
            "Listed the files under a folder"
        );
        INDEX.write().remember(listing);
        let _ = FILES_CHANGED.send(());
    });
}

/// Forget everything read so far, so the next look reads the disk again.
///
/// For the reader who has just asked for the tree to be reloaded: the palette
/// is a window on the same folders, and answering from a listing taken before
/// the reload would be the one place the app still showed the old files.
pub fn forget() {
    INDEX.write().kept.clear();
}

/// Walk `root` and collect the files worth offering.
///
/// In parallel, because the cost of a large tree is the waiting on the
/// filesystem rather than the work, and bounded, because the tree can always
/// turn out to be larger than anyone meant.
fn scan(root: &Path, all_files: bool) -> Listing {
    let found = Mutex::new(Vec::new());
    let count = AtomicUsize::new(0);
    let truncated = AtomicBool::new(false);

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
        .run(|| {
            Box::new(|entry| {
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    return WalkState::Continue;
                }
                let path = entry.into_path();
                if !all_files && !is_markdown_file(&path) {
                    return WalkState::Continue;
                }
                if count.fetch_add(1, Ordering::Relaxed) >= MAX_FILES {
                    truncated.store(true, Ordering::Relaxed);
                    return WalkState::Quit;
                }
                found.lock().push(path);
                WalkState::Continue
            })
        });

    let mut files = found.into_inner();
    // Shallowest first, and alphabetical within a depth. Nothing here decides
    // what the reader sees — the query's score does — but two candidates that
    // answer a query equally well should come out in the same order every
    // time, and the shallower of two identical names is the likelier one.
    files.sort_by_cached_key(|path| (path.components().count(), path.clone()));
    files.truncate(MAX_FILES);

    Listing {
        root: root.to_path_buf(),
        all_files,
        files,
        truncated: truncated.load(Ordering::Relaxed),
    }
}

/// How many threads the walk runs on.
///
/// Enough to keep the filesystem busy, capped so that a machine with many
/// cores does not answer a palette keystroke by starting a small storm.
fn threads() -> usize {
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

    fn listing_of(root: &str) -> Listing {
        Listing {
            root: PathBuf::from(root),
            all_files: false,
            files: Vec::new(),
            truncated: false,
        }
    }

    #[test]
    fn only_the_last_few_folders_are_kept() {
        // Not the shared index: this is the eviction rule, asked of an index
        // of its own rather than through a global other tests also write to.
        let mut index = Index::default();
        for at in 0..MAX_LISTINGS + 2 {
            index.remember(listing_of(&format!("/w/{at}")));
        }

        assert_eq!(index.kept.len(), MAX_LISTINGS);
        assert_eq!(index.kept[0].listing.root, PathBuf::from("/w/2"));
    }

    #[test]
    fn reading_a_folder_again_replaces_what_was_kept() {
        let mut index = Index::default();
        index.remember(listing_of("/w/arto"));
        let mut second = listing_of("/w/arto");
        second.files.push(PathBuf::from("/w/arto/README.md"));
        index.remember(second);

        assert_eq!(index.kept.len(), 1);
        assert_eq!(index.kept[0].listing.files.len(), 1);
    }
}
