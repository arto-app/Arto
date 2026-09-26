//! The documents that link to the one being read.
//!
//! A link is found the way a click follows it: `arto_markdown::document_links`
//! says which links a source makes and what path each one opens, and the path
//! is joined onto the source's own folder. What is left here is deciding
//! which of those land on the document on screen, and saying where in the
//! source each one was written.
//!
//! The search runs over the folder listing [`crate::files`] already keeps,
//! on a thread of its own, and is cheap to repeat for two reasons:
//!
//! - **Most files are never parsed.** A file is read in full only when it
//!   spells the name of the document being looked for ([`mentions`]).
//! - **What a file links to is remembered.** The links it makes do not
//!   depend on which document is being looked for, so they are kept by file
//!   and trusted until its size or modification time changes; moving to
//!   another document costs a `stat` per file already read.
//!
//! Nothing is written to disk. A search is a picture of the folder taken
//! when the Links face asked, as the folder listing is.

use crate::files::Listing;
use crate::markdown::RenderOptions;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::broadcast;

/// How much of a line is kept to show where a link was written.
///
/// Enough to read the sentence around the link in a panel row; a longer line
/// is cut down to a window around the link rather than to its start, which
/// could leave the link itself out.
const EXCERPT_CHARS: usize = 120;

/// How much of what comes before a link is kept when its line is cut down.
const EXCERPT_LEAD_CHARS: usize = 30;

/// A line of a source, cut down to show one link in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    pub text: String,
    /// The bytes of `text` the link itself covers.
    pub mark: Range<usize>,
}

impl Excerpt {
    /// The text before the link, the link, and the text after it.
    pub fn parts(&self) -> (&str, &str, &str) {
        (
            &self.text[..self.mark.start],
            &self.text[self.mark.clone()],
            &self.text[self.mark.end..],
        )
    }
}

/// One link a source makes, resolved to the file it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLink {
    /// The file the link opens, as [`crate::roots::canonical_key`] spells it.
    pub target: PathBuf,
    /// The 1-based line of the source the link was written on.
    pub line: u32,
    pub excerpt: Excerpt,
}

/// Every link in `content` that opens an existing Markdown document.
///
/// `source` is the file `content` was read from; relative links are joined
/// onto its folder, as a click on them would be.
pub fn links_of(source: &Path, content: &str, options: &RenderOptions) -> Vec<ResolvedLink> {
    let folder = source.parent().unwrap_or_else(|| Path::new("."));
    let lines = lines(content);
    arto_markdown::document_links(content, options)
        .into_iter()
        .filter_map(|link| {
            let target = resolve(folder, &link.path)?;
            let start = link.range.start;
            let text = lines.get(start.line.checked_sub(1)?)?;
            // A link that runs onto the next line is marked to the end of
            // the line it starts on, which is the one the row shows.
            let end = if link.range.end.line == start.line {
                link.range.end.column
            } else {
                text.chars().count()
            };
            Some(ResolvedLink {
                target,
                line: u32::try_from(start.line).ok()?,
                excerpt: excerpt(text, start.column, end),
            })
        })
        .collect()
}

/// The file a link opens from `folder`, or `None` when there is none — by
/// the rule a click follows ([`crate::document_link::resolve_link_path`]),
/// spelled for comparison by [`crate::roots::canonical_key`].
pub fn resolve(folder: &Path, path: &str) -> Option<PathBuf> {
    crate::document_link::resolve_link_path(folder, path)
        .filter(|path| path.is_file())
        .map(|path| crate::roots::canonical_key(&path))
}

/// Whether a source could name the document whose file stem is `stem`.
///
/// Read before a source is parsed, and only to decide whether to parse it:
/// a link to a document spells its name, give or take case — the disk may
/// ignore it — and percent-encoding, which Markdown asks for in a path with
/// a space in it. Most of a folder does not mention a given document at all,
/// and those files are never parsed on its account.
///
/// A link through a symlink of another name is the one it cannot see; that
/// source is found once it has been parsed for some other document.
pub fn mentions(content: &[u8], stem: &str) -> bool {
    let stem = stem.to_lowercase();
    String::from_utf8_lossy(content)
        .to_lowercase()
        .contains(&stem)
        || (content.contains(&b'%')
            && percent_encoding::percent_decode(content)
                .decode_utf8_lossy()
                .to_lowercase()
                .contains(&stem))
}

/// The line a link was written on, cut down to show it.
///
/// `start` and `end` are the 1-based columns, in characters, of the first
/// and last character of the link. The indentation goes; a line longer than
/// [`EXCERPT_CHARS`] keeps a window that starts a little before the link.
pub fn excerpt(line: &str, start: usize, end: usize) -> Excerpt {
    let chars: Vec<char> = line.chars().collect();
    let first = chars
        .iter()
        .position(|c| !c.is_whitespace())
        .unwrap_or(chars.len());
    let last = chars.len()
        - chars
            .iter()
            .rev()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);
    let mark_start = start.saturating_sub(1).clamp(first, last);
    let mark_end = end.clamp(mark_start, last);

    let (from, to) = if last - first <= EXCERPT_CHARS {
        (first, last)
    } else {
        let to =
            (mark_start.saturating_sub(EXCERPT_LEAD_CHARS).max(first) + EXCERPT_CHARS).min(last);
        (to - EXCERPT_CHARS, to)
    };
    let mark_start = mark_start.clamp(from, to);
    let mark_end = mark_end.clamp(mark_start, to);

    let slice = |range: Range<usize>| chars[range].iter().collect::<String>();
    let mut text = String::new();
    if from > first {
        text.push('…');
    }
    text.push_str(&slice(from..mark_start));
    let mark_from = text.len();
    text.push_str(&slice(mark_start..mark_end));
    let mark = mark_from..text.len();
    text.push_str(&slice(mark_end..to));
    if to < last {
        text.push('…');
    }
    Excerpt { text, mark }
}

/// The largest source that is read for links.
///
/// A Markdown file bigger than this is a generated dump rather than a
/// document someone links from, and reading one would hold up every other
/// file behind it.
const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;

/// How long one search is allowed to take.
///
/// The same bound the folder listing has, for the same reader: the one on a
/// slow disk, for whom a list that says it is partial is better than one
/// that never arrives.
const TIME_BUDGET: Duration = Duration::from_secs(10);

/// How many searches are remembered at once.
///
/// One per window, near enough, as with the folder listings.
const MAX_KEPT: usize = 4;

/// How long a search is trusted before the next look asks for a fresh one,
/// unless the folder listing it read from has been replaced since.
const FRESH_FOR: Duration = Duration::from_secs(30);

/// One document that links to the target, and every place it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The document, as the folder listing names it.
    pub path: PathBuf,
    /// The links to the target, by line, one per line.
    pub links: Vec<ResolvedLink>,
}

impl Source {
    /// The line of the first link, which is where opening the source lands.
    pub fn first_line(&self) -> Option<u32> {
        self.links.first().map(|link| link.line)
    }
}

/// What one search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlinks {
    /// The folder that was searched.
    pub root: PathBuf,
    /// The document searched for, as it was asked about.
    pub target: PathBuf,
    /// The documents that link to it, by path.
    pub sources: Vec<Source>,
    /// Whether some of the folder was not searched: the listing was cut short
    /// or the search ran out of time.
    pub partial: bool,
}

impl Backlinks {
    fn answers(&self, root: &Path, target: &Path) -> bool {
        self.root == root && self.target == target
    }
}

/// The links one file makes, and the version of the file they were read
/// from.
struct Entry {
    modified: Option<SystemTime>,
    len: u64,
    links: Arc<[ResolvedLink]>,
}

/// A search, and when it was made.
struct Kept {
    backlinks: Arc<Backlinks>,
    /// The folder listing it searched, so a newer listing makes it stale.
    listing: Arc<Listing>,
    at: Instant,
}

/// The searches, the files they read, and the searches in flight.
#[derive(Default)]
struct Index {
    kept: Vec<Kept>,
    /// The links each file makes, whatever they point at.
    ///
    /// Kept apart from any one search because they do not depend on the
    /// target: moving to another document asks the same files again, and a
    /// file that has not changed is answered from here with a `stat`.
    links: HashMap<PathBuf, Entry>,
    /// The searches marked as running, each with the number of the search
    /// that holds the mark — see [`Scanning`].
    scanning: HashMap<(PathBuf, PathBuf), u64>,
    /// The number the next search is marked with.
    next_search: u64,
    /// The document most recently asked about, by folder. A search for any
    /// other document in that folder is one nobody is waiting on any more.
    wanted: HashMap<PathBuf, PathBuf>,
    /// The Markdown options everything kept was read under. What is a link
    /// depends on them — wiki links, and math, which can swallow a link —
    /// so a change to them starts a new era.
    options: Option<RenderOptions>,
    /// Which era what is kept belongs to. See [`forget`].
    era: u64,
}

impl Index {
    /// Drop every search and every file read, and start a new era.
    fn forget(&mut self) {
        self.kept.clear();
        self.links.clear();
        self.era = self.era.wrapping_add(1);
    }

    /// Keep a finished search, dropping the oldest once there are too many.
    /// A search from before the last [`forget`] is dropped instead.
    fn remember(&mut self, backlinks: Backlinks, listing: Arc<Listing>, era: u64) {
        if era != self.era {
            return;
        }
        self.kept
            .retain(|kept| !kept.backlinks.answers(&backlinks.root, &backlinks.target));
        self.kept.push(Kept {
            backlinks: Arc::new(backlinks),
            listing,
            at: Instant::now(),
        });
        while self.kept.len() > MAX_KEPT {
            self.kept.remove(0);
        }
    }
}

/// A search marked as running, unmarked however it ends.
///
/// The mark carries the search's own number, and only that search takes it
/// off: one that gives up unmarks itself early, and by the time it ends a
/// new search for the same document may hold the mark.
struct Scanning {
    key: (PathBuf, PathBuf),
    id: u64,
}

impl Scanning {
    fn unmark(&self, index: &mut Index) {
        if index.scanning.get(&self.key) == Some(&self.id) {
            index.scanning.remove(&self.key);
        }
    }
}

impl Drop for Scanning {
    fn drop(&mut self) {
        self.unmark(&mut INDEX.write());
    }
}

static INDEX: LazyLock<RwLock<Index>> = LazyLock::new(|| RwLock::new(Index::default()));

/// Held by the search that is running. See [`ensure`].
static TURN: Mutex<()> = Mutex::new(());

/// Announced whenever a search finishes, so an open Links face redraws.
pub static BACKLINKS_CHANGED: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(16).0);

/// What links to `target` from under `root`, as far as it has been searched.
///
/// [`None`] until the first search for that pair finishes. It never blocks
/// and never reads the disk.
pub fn lookup(root: &Path, target: &Path) -> Option<Arc<Backlinks>> {
    INDEX
        .read()
        .kept
        .iter()
        .find(|kept| kept.backlinks.answers(root, target))
        .map(|kept| Arc::clone(&kept.backlinks))
}

/// Search `root` for links to `target` if it has not been searched lately,
/// on a thread of its own.
///
/// The folder is searched through its listing (see [`crate::files`]). When
/// there is none yet, this asks for one and returns: the listing announces
/// itself on [`crate::files::FILES_CHANGED`], and asking again then is what
/// starts the search.
pub fn ensure(root: &Path, target: &Path) {
    crate::files::ensure(root, false);
    let Some(listing) = crate::files::listing(root, false) else {
        return;
    };
    let key = (root.to_path_buf(), target.to_path_buf());
    let options = crate::config::CONFIG.read().markdown.clone();
    let (era, id) = {
        let mut index = INDEX.write();
        if index.options.as_ref() != Some(&options) {
            index.forget();
            index.options = Some(options.clone());
        }
        if index.scanning.contains_key(&key) {
            // Still wanted: the search already marked for it is the answer.
            index
                .wanted
                .insert(root.to_path_buf(), target.to_path_buf());
            return;
        }
        let fresh = index.kept.iter().any(|kept| {
            kept.backlinks.answers(root, target)
                && Arc::ptr_eq(&kept.listing, &listing)
                && kept.at.elapsed() < FRESH_FOR
        });
        if fresh {
            return;
        }
        let id = index.next_search;
        index.next_search = id.wrapping_add(1);
        index.scanning.insert(key.clone(), id);
        index
            .wanted
            .insert(root.to_path_buf(), target.to_path_buf());
        (index.era, id)
    };

    // A plain thread, as for the listing: reading files is blocking work,
    // and the runtime it would otherwise sit on is the one drawing the
    // window.
    std::thread::spawn(move || {
        let scanning = Scanning { key, id };
        let (root, target) = scanning.key.clone();
        // One search at a time, across every window: each one already reads
        // on several threads, and two at once only split the disk between
        // them.
        let _turn = TURN.lock();
        // A reader moving from document to document asks for a search at
        // each, and only the last one asked for in a folder still has anyone
        // waiting on it. The search that is given up says nothing: the one
        // that replaced it is running or waiting its turn, and will announce
        // itself, and every window asks again then.
        //
        // Giving up is decided under the index lock, together with taking
        // the mark off: a request for this document arriving at the same
        // moment then either finds the mark and is answered by this search,
        // or finds it gone and starts one of its own.
        let still_wanted = || {
            let mut index = INDEX.write();
            let wanted = index.wanted.get(&root) == Some(&target);
            if !wanted {
                scanning.unmark(&mut index);
            }
            wanted
        };
        let abandon = || INDEX.read().wanted.get(&root) != Some(&target);
        let started = Instant::now();
        let found = loop {
            if !still_wanted() {
                return;
            }
            let found = scan(
                &INDEX,
                era,
                &listing.files,
                &target,
                &options,
                Instant::now() + TIME_BUDGET,
                &abandon,
            );
            if !found.abandoned {
                break found;
            }
        };
        let backlinks = Backlinks {
            root,
            target,
            sources: found.sources,
            partial: listing.truncated || found.partial,
        };
        tracing::debug!(
            root = ?backlinks.root,
            target = ?backlinks.target,
            sources = backlinks.sources.len(),
            partial = backlinks.partial,
            elapsed = ?started.elapsed(),
            "Searched a folder for backlinks"
        );
        INDEX.write().remember(backlinks, listing, era);
        drop(scanning);
        let _ = BACKLINKS_CHANGED.send(());
    });
}

/// Forget every search and every file read, so the next look reads the disk
/// again — for the reader who has just asked for the tree to be reloaded.
///
/// A search already under way belongs to the era before, and what it comes
/// back with is dropped rather than kept.
pub fn forget() {
    INDEX.write().forget();
}

/// What one pass over a folder's files found.
#[derive(Debug, Default)]
struct Scan {
    /// The sources, in path order.
    sources: Vec<Source>,
    /// Whether some file went unsearched: the deadline came, or a file was
    /// too large to read.
    partial: bool,
    /// Whether the pass was given up because nobody wants its answer any
    /// more; what it found is then not an answer at all.
    abandoned: bool,
}

/// Search `files` for links to `target`, until `deadline` or until
/// `abandon` says the answer is no longer wanted.
fn scan(
    index: &RwLock<Index>,
    era: u64,
    files: &[PathBuf],
    target: &Path,
    options: &RenderOptions,
    deadline: Instant,
    abandon: &(dyn Fn() -> bool + Sync),
) -> Scan {
    let target_key = crate::roots::canonical_key(target);
    let Some(stem) = target.file_stem().map(|stem| stem.to_string_lossy()) else {
        return Scan::default();
    };
    let next = AtomicUsize::new(0);
    let partial = AtomicBool::new(false);
    let abandoned = AtomicBool::new(false);
    let found = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for _ in 0..crate::files::threads() {
            scope.spawn(|| loop {
                let at = next.fetch_add(1, Ordering::Relaxed);
                let Some(file) = files.get(at) else {
                    break;
                };
                if abandoned.load(Ordering::Relaxed) || abandon() {
                    abandoned.store(true, Ordering::Relaxed);
                    break;
                }
                if Instant::now() >= deadline {
                    partial.store(true, Ordering::Relaxed);
                    break;
                }
                let links = match links_from(index, era, file, &stem, options) {
                    Read::Links(links) => links,
                    Read::Nothing => continue,
                    // The target itself is never one of its own sources, so
                    // not reading it leaves nothing out.
                    Read::Skipped => {
                        if crate::roots::canonical_key(file) != target_key {
                            partial.store(true, Ordering::Relaxed);
                        }
                        continue;
                    }
                };
                let mut to_target: Vec<ResolvedLink> = links
                    .iter()
                    .filter(|link| link.target == target_key)
                    .cloned()
                    .collect();
                to_target.dedup_by_key(|link| link.line);
                if to_target.is_empty() || crate::roots::canonical_key(file) == target_key {
                    continue;
                }
                found.lock().push(Source {
                    path: file.clone(),
                    links: to_target,
                });
            });
        }
    });

    let mut sources = found.into_inner();
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Scan {
        sources,
        partial: partial.load(Ordering::Relaxed),
        abandoned: abandoned.load(Ordering::Relaxed),
    }
}

/// What reading one file came to.
enum Read {
    Links(Arc<[ResolvedLink]>),
    /// Nothing that could link to the target: it does not mention it, or it
    /// is gone.
    Nothing,
    /// Too large or unreadable, so whether it links to the target is not
    /// known.
    Skipped,
}

/// The links `file` makes, from the index when the file has not changed.
///
/// A file not read before is read only when it mentions `stem`; one that
/// does not is left unread and unrecorded, since it may yet mention the next
/// document asked about.
fn links_from(
    index: &RwLock<Index>,
    era: u64,
    file: &Path,
    stem: &str,
    options: &RenderOptions,
) -> Read {
    let metadata = match std::fs::metadata(file) {
        Ok(metadata) => metadata,
        Err(error) => return unreadable(&error),
    };
    if !metadata.is_file() {
        return Read::Nothing;
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Read::Skipped;
    }
    let modified = metadata.modified().ok();
    let len = metadata.len();
    if let Some(entry) = index.read().links.get(file) {
        if entry.modified == modified && entry.len == len {
            return Read::Links(Arc::clone(&entry.links));
        }
    }

    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) => return unreadable(&error),
    };
    if !mentions(&bytes, stem) {
        return Read::Nothing;
    }
    let content = String::from_utf8_lossy(&bytes);
    let links: Arc<[ResolvedLink]> = links_of(file, &content, options).into();
    let mut index = index.write();
    if index.era == era {
        index.links.insert(
            file.to_path_buf(),
            Entry {
                modified,
                len,
                links: Arc::clone(&links),
            },
        );
    }
    Read::Links(links)
}

/// What a file that could not be read comes to: nothing, when it has gone
/// since the folder was listed, and unknown otherwise.
fn unreadable(error: &std::io::Error) -> Read {
    if error.kind() == std::io::ErrorKind::NotFound {
        Read::Nothing
    } else {
        Read::Skipped
    }
}

/// The lines of `content`, ended the way the Markdown pipeline ends them: by
/// `\n`, `\r\n` or a lone `\r`.
fn lines(content: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = content;
    while let Some(at) = rest.find(['\n', '\r']) {
        lines.push(&rest[..at]);
        let skip = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
        rest = &rest[at + skip..];
    }
    lines.push(rest);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    fn key(path: &Path) -> PathBuf {
        crate::roots::canonical_key(path)
    }

    /// `docs/guide.md`, `docs/api/auth.md`, `my notes.md` and `README.md`.
    fn tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("docs/api")).unwrap();
        fs::write(root.join("docs/guide.md"), "# Guide\n").unwrap();
        fs::write(root.join("docs/api/auth.md"), "# Auth\n").unwrap();
        fs::write(root.join("my notes.md"), "# Notes\n").unwrap();
        fs::write(root.join("README.md"), "# Readme\n").unwrap();
        dir
    }

    #[test]
    fn a_link_resolves_from_the_folder_of_its_source() {
        let dir = tree();
        let docs = dir.path().join("docs");

        assert_eq!(
            resolve(&docs, "./guide.md"),
            Some(key(&docs.join("guide.md")))
        );
        assert_eq!(
            resolve(&docs, "api/auth.md"),
            Some(key(&docs.join("api/auth.md")))
        );
        assert_eq!(
            resolve(&docs.join("api"), "../../README.md"),
            Some(key(&dir.path().join("README.md")))
        );
    }

    #[test]
    fn a_link_to_nothing_resolves_to_nothing() {
        let dir = tree();
        assert_eq!(resolve(dir.path(), "missing.md"), None);
        assert_eq!(resolve(dir.path(), "docs"), None);
    }

    #[test]
    fn a_percent_encoded_link_names_the_file_it_spells() {
        let dir = tree();
        assert_eq!(
            resolve(dir.path(), "my%20notes.md"),
            Some(key(&dir.path().join("my notes.md")))
        );
    }

    #[test]
    fn an_absolute_path_is_its_own_answer() {
        let dir = tree();
        let readme = dir.path().join("README.md");
        assert_eq!(
            resolve(Path::new("/elsewhere"), &readme.to_string_lossy()),
            Some(key(&readme))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_through_a_symlink_lands_on_the_file_it_points_at() {
        let dir = tree();
        std::os::unix::fs::symlink(
            dir.path().join("docs/guide.md"),
            dir.path().join("alias.md"),
        )
        .unwrap();

        assert_eq!(
            resolve(dir.path(), "alias.md"),
            Some(key(&dir.path().join("docs/guide.md")))
        );
    }

    #[test]
    fn the_links_of_a_source_carry_their_line_and_what_it_says() {
        let dir = tree();
        let source = dir.path().join("docs/api/auth.md");
        let content = indoc! {"
            # Auth

            Start with [the guide](../guide.md), then [[../../README]].
            See [elsewhere](https://example.com) and [nothing](missing.md).
        "};

        let found = links_of(&source, content, &RenderOptions::default());

        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].target, key(&dir.path().join("docs/guide.md")));
        assert_eq!(found[0].line, 3);
        assert_eq!(found[0].excerpt.parts().1, "[the guide](../guide.md)");
        assert_eq!(found[1].target, key(&dir.path().join("README.md")));
        assert_eq!(found[1].excerpt.parts().1, "[[../../README]]");
    }

    #[test]
    fn a_file_url_resolves_like_a_path() {
        let dir = tree();
        let guide = dir.path().join("docs/guide.md");
        // A Windows path has no leading slash and uses backslashes, so the URL
        // is spelled out rather than built by prefixing `file://`.
        let path = guide.to_string_lossy().replace('\\', "/");
        let content = format!("[guide](file:///{})\n", path.trim_start_matches('/'));

        let found = links_of(
            &dir.path().join("README.md"),
            &content,
            &RenderOptions::default(),
        );

        assert_eq!(found[0].target, key(&guide));
    }

    #[test]
    fn a_wiki_link_without_an_extension_finds_its_document() {
        let dir = tree();
        let found = links_of(
            &dir.path().join("docs/index.md"),
            "Read [[guide]].\n",
            &RenderOptions::default(),
        );
        assert_eq!(found[0].target, key(&dir.path().join("docs/guide.md")));
    }

    #[test]
    fn a_mention_ignores_case_and_percent_encoding() {
        assert!(mentions(b"see [x](./Guide.md)", "guide"));
        assert!(mentions(b"see [x](my%20notes.md)", "my notes"));
        assert!(mentions(b"see [x](g%75ide.md)", "guide"));
        assert!(mentions(b"see [x](caf%C3%A9.md)", "caf\u{e9}"));
        assert!(!mentions(b"nothing to see here", "guide"));
    }

    #[test]
    fn an_excerpt_drops_the_indentation_and_marks_the_link() {
        let found = excerpt("    - see [a](a.md) now", 11, 19);
        assert_eq!(found.text, "- see [a](a.md) now");
        assert_eq!(found.parts(), ("- see ", "[a](a.md)", " now"));
    }

    #[test]
    fn a_long_line_keeps_a_window_around_the_link() {
        let line = format!("{} [a](a.md) {}", "x".repeat(200), "y".repeat(200));
        let found = excerpt(&line, 202, 210);

        assert!(found.text.starts_with('…'), "{}", found.text);
        assert!(found.text.ends_with('…'), "{}", found.text);
        assert_eq!(found.parts().1, "[a](a.md)");
        assert!(found.text.chars().count() <= EXCERPT_CHARS + 2);
    }

    #[test]
    fn an_excerpt_counts_characters_rather_than_bytes() {
        let found = excerpt("日本語の[説明](a.md)です", 5, 14);
        assert_eq!(found.parts(), ("日本語の", "[説明](a.md)", "です"));
    }

    #[test]
    fn lines_end_however_the_file_ends_them() {
        assert_eq!(lines("a\nb\r\nc\rd"), vec!["a", "b", "c", "d"]);
    }

    /// A folder where `guide.md` is linked from two documents, one of them
    /// twice on a line, and mentioned by name in a third that does not link.
    fn linked_tree() -> (TempDir, Vec<PathBuf>) {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(
            root.join("guide.md"),
            "# Guide\n\nThis is [the guide](guide.md) itself.\n",
        )
        .unwrap();
        fs::write(
            root.join("README.md"),
            indoc! {"
                # Readme

                Read [the guide](guide.md) or [[guide]].

                Then [the guide](./guide.md#setup) again.
            "},
        )
        .unwrap();
        fs::write(root.join("notes/today.md"), "See [guide](../guide.md).\n").unwrap();
        fs::write(root.join("notes/other.md"), "The guide is `guide.md`.\n").unwrap();
        fs::write(root.join("unrelated.md"), "Nothing here.\n").unwrap();
        let files = [
            "README.md",
            "guide.md",
            "notes/other.md",
            "notes/today.md",
            "unrelated.md",
        ]
        .iter()
        .map(|name| root.join(name))
        .collect();
        (dir, files)
    }

    fn scan_for(index: &RwLock<Index>, files: &[PathBuf], target: &Path) -> Scan {
        let era = index.read().era;
        scan(
            index,
            era,
            files,
            target,
            &RenderOptions::default(),
            Instant::now() + Duration::from_secs(60),
            &|| false,
        )
    }

    #[test]
    fn a_search_lists_every_document_that_links_here_by_path() {
        let (dir, files) = linked_tree();
        let index = RwLock::new(Index::default());

        let Scan {
            sources,
            partial: out_of_time,
            ..
        } = scan_for(&index, &files, &dir.path().join("guide.md"));

        assert!(!out_of_time);
        let paths: Vec<_> = sources.iter().map(|source| source.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                dir.path().join("README.md"),
                dir.path().join("notes/today.md")
            ]
        );
        // Two links on line 3 are one row; line 5 is another.
        let lines: Vec<_> = sources[0].links.iter().map(|link| link.line).collect();
        assert_eq!(lines, vec![3, 5]);
        assert_eq!(sources[0].first_line(), Some(3));
    }

    #[test]
    fn a_document_that_does_not_mention_the_target_is_not_read() {
        let (dir, files) = linked_tree();
        let index = RwLock::new(Index::default());

        scan_for(&index, &files, &dir.path().join("guide.md"));

        let index = index.read();
        assert!(!index.links.contains_key(&dir.path().join("unrelated.md")));
        assert!(index.links.contains_key(&dir.path().join("notes/other.md")));
    }

    #[test]
    fn a_changed_document_is_read_again() {
        let (dir, files) = linked_tree();
        let index = RwLock::new(Index::default());
        let target = dir.path().join("guide.md");
        scan_for(&index, &files, &target);

        fs::write(dir.path().join("notes/today.md"), "No links any more.\n").unwrap();
        let Scan { sources, .. } = scan_for(&index, &files, &target);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path, dir.path().join("README.md"));
    }

    #[test]
    fn what_a_document_links_to_is_reused_for_the_next_target() {
        // README.md was read while looking for guide.md; its link to
        // today.md is answered from the index even though "today" is not
        // in it — nothing about README.md changed.
        let (dir, files) = linked_tree();
        fs::write(
            dir.path().join("README.md"),
            "Read [the guide](guide.md) and [notes](notes/today.md).\n",
        )
        .unwrap();
        let index = RwLock::new(Index::default());
        scan_for(&index, &files, &dir.path().join("guide.md"));

        let Scan { sources, .. } = scan_for(&index, &files, &dir.path().join("notes/today.md"));

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path, dir.path().join("README.md"));
    }

    #[test]
    fn a_search_out_of_time_says_so() {
        let (dir, files) = linked_tree();
        let index = RwLock::new(Index::default());

        let Scan {
            sources,
            partial: out_of_time,
            ..
        } = scan(
            &index,
            0,
            &files,
            &dir.path().join("guide.md"),
            &RenderOptions::default(),
            Instant::now(),
            &|| false,
        );

        assert!(out_of_time);
        assert!(sources.is_empty());
    }

    #[test]
    fn what_was_read_before_a_reload_is_not_recorded() {
        let (dir, files) = linked_tree();
        let index = RwLock::new(Index::default());
        let era = index.read().era;
        index.write().era = era.wrapping_add(1);

        let Scan { sources, .. } = scan(
            &index,
            era,
            &files,
            &dir.path().join("guide.md"),
            &RenderOptions::default(),
            Instant::now() + Duration::from_secs(60),
            &|| false,
        );

        assert_eq!(sources.len(), 2);
        assert!(index.read().links.is_empty());
    }

    #[test]
    fn a_search_nobody_wants_any_more_is_given_up() {
        let (dir, files) = linked_tree();
        let index = RwLock::new(Index::default());

        let found = scan(
            &index,
            0,
            &files,
            &dir.path().join("guide.md"),
            &RenderOptions::default(),
            Instant::now() + Duration::from_secs(60),
            &|| true,
        );

        assert!(found.abandoned);
        assert!(found.sources.is_empty());
    }

    #[test]
    fn a_file_too_large_to_read_makes_the_search_partial() {
        let (dir, mut files) = linked_tree();
        let large = dir.path().join("large.md");
        let mut content = "[guide](guide.md)\n".to_string();
        content.push_str(&"x".repeat(MAX_SOURCE_BYTES as usize));
        fs::write(&large, content).unwrap();
        files.push(large);
        let index = RwLock::new(Index::default());

        let found = scan_for(&index, &files, &dir.path().join("guide.md"));

        assert!(found.partial);
        assert_eq!(found.sources.len(), 2);
    }

    #[test]
    fn a_target_too_large_to_read_leaves_nothing_out() {
        let (dir, files) = linked_tree();
        let target = dir.path().join("guide.md");
        fs::write(&target, "x".repeat(MAX_SOURCE_BYTES as usize + 1)).unwrap();
        let index = RwLock::new(Index::default());

        let found = scan_for(&index, &files, &target);

        assert!(!found.partial);
        assert_eq!(found.sources.len(), 2);
    }

    #[test]
    fn a_search_that_gave_up_leaves_a_newer_mark_alone() {
        let key = (PathBuf::from("/w"), PathBuf::from("/w/a.md"));
        let mut index = Index::default();
        let old = Scanning {
            key: key.clone(),
            id: 1,
        };
        index.scanning.insert(key.clone(), 2);

        old.unmark(&mut index);
        std::mem::forget(old);

        assert_eq!(index.scanning.get(&key), Some(&2));
    }

    fn listing() -> Arc<Listing> {
        Arc::new(Listing {
            root: PathBuf::from("/w"),
            all_files: false,
            files: Vec::new(),
            truncated: false,
        })
    }

    fn backlinks_of(target: &str) -> Backlinks {
        Backlinks {
            root: PathBuf::from("/w"),
            target: PathBuf::from(target),
            sources: Vec::new(),
            partial: false,
        }
    }

    #[test]
    fn only_the_last_few_searches_are_kept() {
        let mut index = Index::default();
        for at in 0..MAX_KEPT + 2 {
            let era = index.era;
            index.remember(backlinks_of(&format!("/w/{at}.md")), listing(), era);
        }

        assert_eq!(index.kept.len(), MAX_KEPT);
        assert_eq!(index.kept[0].backlinks.target, PathBuf::from("/w/2.md"));
    }

    #[test]
    fn a_search_from_before_a_reload_is_not_kept() {
        let mut index = Index::default();
        let era = index.era;
        index.era = era.wrapping_add(1);
        index.remember(backlinks_of("/w/a.md"), listing(), era);

        assert!(index.kept.is_empty());
    }
}
