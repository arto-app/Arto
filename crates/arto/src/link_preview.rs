//! Another document, rendered to be previewed beside a link to it.
//!
//! The page asks for the whole document and cuts the part it shows out of
//! it (`frontend/src/link-preview.ts`): the heading ids it matches a
//! fragment against are the ones the renderer wrote, so they are not
//! worked out a second time here.
//!
//! A pointer passes over the same links again and again, so what was
//! rendered is kept in memory for as long as the file and the preferences
//! it was rendered with stay the same. Nothing is written to disk.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use crate::document_link::{resolve_document_link, split_link_fragment};
use crate::markdown::{self, RenderOptions};
use crate::utils::file::is_markdown_file;

/// Documents larger than this are not previewed: the whole rendering
/// crosses into the page, and a preview is not worth a stall.
pub const MAX_PREVIEW_BYTES: u64 = 2 * 1024 * 1024;
/// How much rendered HTML is kept, in bytes: a budget rather than a count,
/// since one rendering can be many times the size of another. The same
/// shape as the lens cache (`lenses/cache.rs`).
const CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
/// How many renderings are kept, however small: an empty document renders
/// to nothing, and would otherwise count for nothing against the budget.
const CACHE_MAX_ENTRIES: usize = 32;

static CACHE: LazyLock<Mutex<PreviewCache>> =
    LazyLock::new(|| Mutex::new(PreviewCache::new(CACHE_MAX_ENTRIES, CACHE_MAX_BYTES)));

/// Why a link has no preview.
#[derive(Debug, thiserror::Error)]
pub enum Unavailable {
    #[error("the link names no file")]
    Unresolved,
    #[error("the file is not Markdown")]
    NotMarkdown,
    #[error("the file is larger than {MAX_PREVIEW_BYTES} bytes")]
    TooLarge,
    #[error("the file cannot be read: {0}")]
    Unreadable(String),
}

/// What a file was when it was rendered; a change to either means it has
/// been written since.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

struct Entry {
    path: PathBuf,
    stamp: Stamp,
    options: RenderOptions,
    html: Arc<String>,
}

/// The documents rendered last, the most recently used at the back.
struct PreviewCache {
    max_entries: usize,
    max_bytes: usize,
    entries: VecDeque<Entry>,
}

impl PreviewCache {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
            entries: VecDeque::new(),
        }
    }

    fn bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.html.len()).sum()
    }

    /// The rendering of `path`, if it was made from the file as it is now
    /// and with the same preferences. A stale one is dropped.
    fn get(&mut self, path: &Path, stamp: Stamp, options: &RenderOptions) -> Option<Arc<String>> {
        let index = self.entries.iter().position(|entry| entry.path == path)?;
        let entry = self.entries.remove(index)?;
        if entry.stamp != stamp || &entry.options != options {
            return None;
        }
        let html = entry.html.clone();
        self.entries.push_back(entry);
        Some(html)
    }

    /// Keep `entry`, unless it alone is larger than the whole cache, letting
    /// the least recently used go until the rest fits.
    fn insert(&mut self, entry: Entry) {
        self.entries.retain(|kept| kept.path != entry.path);
        if entry.html.len() > self.max_bytes {
            return;
        }
        self.entries.push_back(entry);
        while self.entries.len() > self.max_entries || self.bytes() > self.max_bytes {
            self.entries.pop_front();
        }
    }
}

/// The document `link` (as written in `current_file`) points at, rendered
/// for a preview.
pub async fn preview(current_file: PathBuf, link: String) -> Result<Arc<String>, Unavailable> {
    let (path, _fragment) = split_link_fragment(&link);
    let target = resolve_document_link(&current_file, path).ok_or(Unavailable::Unresolved)?;
    let options = markdown::preview_options();
    tokio::task::spawn_blocking(move || load(&target, &options, &CACHE))
        .await
        .map_err(|error| Unavailable::Unreadable(error.to_string()))?
}

/// The rendering of the Markdown file at `path`, from `cache` when the file
/// has not changed since.
fn load(
    path: &Path,
    options: &RenderOptions,
    cache: &Mutex<PreviewCache>,
) -> Result<Arc<String>, Unavailable> {
    if !is_markdown_file(path) {
        return Err(Unavailable::NotMarkdown);
    }
    let unreadable = |error: std::io::Error| Unavailable::Unreadable(error.to_string());
    let metadata = std::fs::metadata(path).map_err(unreadable)?;
    if !metadata.is_file() {
        return Err(Unavailable::Unreadable("not a regular file".to_string()));
    }
    if metadata.len() > MAX_PREVIEW_BYTES {
        return Err(Unavailable::TooLarge);
    }
    let stamp = Stamp {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    };
    if let Some(html) = lock(cache).get(path, stamp, options) {
        return Ok(html);
    }

    // Bounded as well, in case the file grew after it was measured.
    let mut source = String::new();
    std::fs::File::open(path)
        .map_err(unreadable)?
        .take(MAX_PREVIEW_BYTES + 1)
        .read_to_string(&mut source)
        .map_err(unreadable)?;
    if source.len() as u64 > MAX_PREVIEW_BYTES {
        return Err(Unavailable::TooLarge);
    }
    let html = markdown::render_preview(&source, path, options)
        .map(Arc::new)
        .map_err(|error| Unavailable::Unreadable(error.to_string()))?;
    lock(cache).insert(Entry {
        path: path.to_path_buf(),
        stamp,
        options: options.clone(),
        html: html.clone(),
    });
    Ok(html)
}

/// The cache holds nothing a panic could leave half-written, so a poisoned
/// lock is taken as it is.
fn lock(cache: &Mutex<PreviewCache>) -> std::sync::MutexGuard<'_, PreviewCache> {
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use tempfile::TempDir;

    fn cache() -> Mutex<PreviewCache> {
        Mutex::new(PreviewCache::new(CACHE_MAX_ENTRIES, CACHE_MAX_BYTES))
    }

    fn entry(path: &str, len: u64) -> Entry {
        Entry {
            path: PathBuf::from(path),
            stamp: Stamp {
                modified: None,
                len,
            },
            options: RenderOptions::default(),
            html: Arc::new(path.to_string()),
        }
    }

    fn stamp(len: u64) -> Stamp {
        Stamp {
            modified: None,
            len,
        }
    }

    #[test]
    fn a_cached_rendering_is_used_while_the_file_and_preferences_stay_the_same() {
        let mut cache = PreviewCache::new(4, 64);
        cache.insert(entry("/a.md", 1));
        let options = RenderOptions::default();

        assert!(cache.get(Path::new("/a.md"), stamp(1), &options).is_some());
        assert!(cache.get(Path::new("/a.md"), stamp(2), &options).is_none());
        // The stale one is gone rather than kept beside a fresh one.
        assert!(cache.get(Path::new("/a.md"), stamp(1), &options).is_none());
    }

    #[test]
    fn a_change_of_preferences_renders_again() {
        let mut cache = PreviewCache::new(4, 64);
        cache.insert(entry("/a.md", 1));
        let options = RenderOptions {
            raw_html: markdown::RawHtml::Escape,
            ..RenderOptions::default()
        };

        assert!(cache.get(Path::new("/a.md"), stamp(1), &options).is_none());
    }

    #[test]
    fn the_least_recently_used_rendering_goes_first() {
        // Room for two of the renderings `entry` makes, which are as long as
        // their paths.
        let mut cache = PreviewCache::new(4, "/a.md".len() * 2);
        let options = RenderOptions::default();
        cache.insert(entry("/a.md", 1));
        cache.insert(entry("/b.md", 1));
        cache.get(Path::new("/a.md"), stamp(1), &options);
        cache.insert(entry("/c.md", 1));

        assert!(cache.get(Path::new("/b.md"), stamp(1), &options).is_none());
        assert!(cache.get(Path::new("/a.md"), stamp(1), &options).is_some());
        assert!(cache.get(Path::new("/c.md"), stamp(1), &options).is_some());
    }

    #[test]
    fn renderings_of_nothing_still_count_against_the_number_kept() {
        let mut cache = PreviewCache::new(2, 64);
        for path in ["/a.md", "/b.md", "/c.md"] {
            cache.insert(Entry {
                html: Arc::new(String::new()),
                ..entry(path, 1)
            });
        }

        assert_eq!(cache.entries.len(), 2);
        assert!(cache
            .get(Path::new("/a.md"), stamp(1), &RenderOptions::default())
            .is_none());
    }

    #[test]
    fn a_rendering_larger_than_the_whole_cache_is_not_kept() {
        let mut cache = PreviewCache::new(4, 4);
        cache.insert(entry("/a.md", 1));

        assert!(cache
            .get(Path::new("/a.md"), stamp(1), &RenderOptions::default())
            .is_none());
    }

    #[test]
    fn a_document_is_rendered_with_its_heading_ids() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");
        std::fs::write(
            &path,
            indoc! {"
                # Title

                Body.
            "},
        )
        .unwrap();

        let html = load(&path, &RenderOptions::default(), &cache()).unwrap();

        assert!(html.contains(r#"id="title""#), "{html}");
        assert!(html.contains("Body."), "{html}");
    }

    #[test]
    fn an_unchanged_document_is_not_rendered_again() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# Title").unwrap();
        let cache = cache();
        let options = RenderOptions::default();

        let first = load(&path, &options, &cache).unwrap();
        let second = load(&path, &options, &cache).unwrap();
        assert!(Arc::ptr_eq(&first, &second));

        std::fs::write(&path, "# Another title").unwrap();
        let third = load(&path, &options, &cache).unwrap();
        assert!(third.contains("Another title"), "{third}");
    }

    #[test]
    fn a_file_that_is_not_markdown_has_no_preview() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, "text").unwrap();

        let result = load(&path, &RenderOptions::default(), &cache());

        assert!(
            matches!(result, Err(Unavailable::NotMarkdown)),
            "{result:?}"
        );
    }

    #[test]
    fn a_document_over_the_limit_has_no_preview() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("huge.md");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_PREVIEW_BYTES + 1).unwrap();

        let result = load(&path, &RenderOptions::default(), &cache());

        assert!(matches!(result, Err(Unavailable::TooLarge)), "{result:?}");
    }

    #[test]
    fn a_missing_document_has_no_preview() {
        let dir = TempDir::new().unwrap();

        let result = load(
            &dir.path().join("gone.md"),
            &RenderOptions::default(),
            &cache(),
        );

        assert!(
            matches!(result, Err(Unavailable::Unreadable(_))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn a_link_that_names_no_file_has_no_preview() {
        let dir = TempDir::new().unwrap();

        let result = preview(dir.path().join("doc.md"), "gone.md#part".to_string()).await;

        assert!(matches!(result, Err(Unavailable::Unresolved)), "{result:?}");
    }
}
