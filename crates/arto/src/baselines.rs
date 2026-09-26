//! The version of each document the reader last read, so that opening it
//! again can say what has changed since.
//!
//! One version per document, the whole source: the blocks a renderer draws
//! move whenever the renderer changes, and a line of text does not. The
//! versions live with the app's data rather than its caches — see
//! [`crate::utils::data_store`] — each one gzipped under the hash of the
//! document's path. A document larger than [`MAX_DOCUMENT_BYTES`] keeps no
//! version and is shown without changes; past [`MAX_BYTES`] together, the
//! versions read longest ago go first.
//!
//! Not in the visit history, which is written out whole on every visit: a
//! copy of every document read would make each of those writes the size of
//! everything the reader has ever opened.

use crate::utils::data_store::{evict, record_file_name, touch, write_atomically};
use chrono::{DateTime, Local};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// How much the versions may take on disk, together.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// The largest document a version is kept of.
const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

/// The version of the file format, so an older one is read as missing
/// rather than misread.
const VERSION: u32 = 1;

/// The store the app uses, or `None` where the system names no data
/// directory.
static STORE: LazyLock<Option<Store>> = LazyLock::new(|| {
    dirs::data_local_dir().map(|dir| {
        Store::new(
            dir.join("arto").join("baselines"),
            MAX_BYTES,
            MAX_DOCUMENT_BYTES,
        )
    })
});

/// The version of a document the reader last read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    version: u32,
    pub path: PathBuf,
    pub read_at: DateTime<Local>,
    pub source: String,
}

/// The versions, in one directory.
pub(crate) struct Store {
    root: PathBuf,
    max_bytes: u64,
    max_document_bytes: usize,
}

impl Store {
    pub(crate) fn new(root: PathBuf, max_bytes: u64, max_document_bytes: usize) -> Self {
        Self {
            root,
            max_bytes,
            max_document_bytes,
        }
    }

    /// The version of `document` last read, or `None` when there is none or
    /// it cannot be read.
    pub(crate) fn load(&self, document: &Path) -> Option<Baseline> {
        let path = self.file(document);
        let file = fs::File::open(&path).ok()?;
        let mut json = Vec::new();
        GzDecoder::new(file).read_to_end(&mut json).ok()?;
        let baseline = serde_json::from_slice::<Baseline>(&json)
            .ok()
            .filter(|baseline| baseline.version == VERSION)?;
        // Read is use: what was read last is kept longest.
        touch(&path);
        Some(baseline)
    }

    /// Keep `source` as the version of `document` last read.
    ///
    /// Returns whether it was kept: a document past the size limit is not.
    pub(crate) fn advance(&self, document: &Path, source: &str) -> io::Result<bool> {
        if source.len() > self.max_document_bytes {
            return Ok(false);
        }
        let baseline = Baseline {
            version: VERSION,
            path: document.to_path_buf(),
            read_at: Local::now(),
            source: source.to_string(),
        };
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(&serde_json::to_vec(&baseline)?)?;
        let path = self.file(document);
        write_atomically(&path, &gzip.finish()?)?;
        evict(&self.root, self.max_bytes, &path, &[]);
        Ok(true)
    }

    fn file(&self, document: &Path) -> PathBuf {
        self.root
            .join(record_file_name(&[&document.to_string_lossy()], "json.gz"))
    }
}

/// The version of `document` last read, from the app's store.
pub fn load(document: &Path) -> Option<Baseline> {
    STORE.as_ref()?.load(document)
}

/// Keep `source` as the version of `document` last read, in the app's store.
pub fn advance(document: &Path, source: &str) {
    let Some(store) = STORE.as_ref() else {
        return;
    };
    if let Err(error) = store.advance(document, source) {
        tracing::warn!(%error, ?document, "the version last read was not kept");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "/notes/a.md";

    fn store(dir: &tempfile::TempDir) -> Store {
        Store::new(dir.path().to_path_buf(), MAX_BYTES, MAX_DOCUMENT_BYTES)
    }

    #[test]
    fn a_version_kept_is_there_when_the_document_is_opened_again() {
        let dir = tempfile::tempdir().unwrap();
        assert!(store(&dir).advance(Path::new(DOC), "# 読む\n").unwrap());

        let baseline = store(&dir).load(Path::new(DOC)).unwrap();

        assert_eq!(baseline.source, "# 読む\n");
        assert_eq!(baseline.path, Path::new(DOC));
        assert!(store(&dir).load(Path::new("/notes/b.md")).is_none());
    }

    #[test]
    fn a_version_kept_again_replaces_the_last() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.advance(Path::new(DOC), "first").unwrap();
        store.advance(Path::new(DOC), "second").unwrap();

        assert_eq!(store.load(Path::new(DOC)).unwrap().source, "second");
    }

    #[test]
    fn a_document_past_the_limit_keeps_no_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf(), MAX_BYTES, 10);

        assert!(!store.advance(Path::new(DOC), "longer than ten").unwrap());
        assert!(store.load(Path::new(DOC)).is_none());
    }

    #[test]
    fn versions_past_the_limit_lose_the_one_read_longest_ago() {
        let dir = tempfile::tempdir().unwrap();
        // Characters gzip cannot shrink much, so each file has a known weight.
        let text: String = (0..3000u32)
            .map(|n| char::from_u32(0x4e00 + (n * 7919) % 20000).unwrap())
            .collect();
        let store = Store::new(dir.path().to_path_buf(), 20_000, MAX_DOCUMENT_BYTES);
        for document in ["/a.md", "/b.md", "/c.md"] {
            store.advance(Path::new(document), &text).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert!(store.load(Path::new("/a.md")).is_none());
        assert!(store.load(Path::new("/c.md")).is_some());
    }

    #[test]
    fn a_version_that_cannot_be_read_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        fs::write(store.file(Path::new(DOC)), b"not gzip").unwrap();

        assert!(store.load(Path::new(DOC)).is_none());
    }

    #[test]
    fn a_version_in_another_format_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        let json = serde_json::json!({
            "version": VERSION + 1,
            "path": DOC,
            "readAt": Local::now(),
            "source": "text",
        });
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(json.to_string().as_bytes()).unwrap();
        fs::write(store.file(Path::new(DOC)), gzip.finish().unwrap()).unwrap();

        assert!(store.load(Path::new(DOC)).is_none());
    }
}
