//! Answers kept on disk, so that a document opened again — after a restart,
//! a day later — shows what its lenses answered without asking again, and
//! says which answers are about text that has since changed.
//!
//! Each document and lens has a record of answers, each filed under the
//! place it answers for — the whole document, or a block by its position —
//! and the key of the request it answered (see [`super::job`]). A request
//! whose key is on file is answered *fresh*, wherever it is now; a place
//! whose request changed keeps its old answer as *stale*, shown for what it
//! is until the reader asks for it again.
//!
//! The records live with the app's data rather than its caches: the caches
//! are cleared whenever a new build starts, and these are the reader's
//! answers, some of which took minutes to write. What grows past
//! [`MAX_BYTES`] loses the records used longest ago.
//!
//! Beside them is the list of lenses each document had open, which is what
//! opens them again with the document.

use crate::utils::data_store::{evict, record_file_name, touch, write_atomically};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// How much the records may take on disk, together.
const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// The version of the record format, so an older one is read as empty
/// rather than misread.
const VERSION: u32 = 1;

/// The file that lists the lenses each document had open.
const OPEN_FILE: &str = "open.json";

/// The store the app uses, or `None` where the system names no data
/// directory.
pub(crate) static STORE: LazyLock<Option<Store>> = LazyLock::new(|| {
    dirs::data_local_dir().map(|dir| Store::new(dir.join("arto").join("lenses"), MAX_BYTES))
});

/// A lens a document had open, and whether its answer was shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RememberedLens {
    pub id: String,
    pub shown: bool,
}

/// A request key, as [`super::job::Job`] carries it.
pub(crate) type Key = [u8; 32];

/// What a lens answered about one document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Record {
    version: u32,
    entries: Vec<Entry>,
}

impl Default for Record {
    /// No answers, in the format this build writes — so a record filed
    /// afresh equals the same answers read back, and nothing is written
    /// again for them.
    fn default() -> Self {
        Self {
            version: VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entry {
    /// The place the answer is for: `document`, or `block:<n>`.
    slot: String,
    key: String,
    answer: String,
}

impl Record {
    /// Whether the lens has answered anything about the document yet.
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The answer to the request `key`, wherever in the document it was
    /// asked: a block moved by an edit above it is still the same request.
    pub(crate) fn fresh(&self, key: &Key) -> Option<&str> {
        let key = hex(key);
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.answer.as_str())
    }

    /// The last answer filed for `slot`, whatever it was asked about —
    /// unless it answers one of `claimed`, the requests that find it fresh
    /// elsewhere: a block pushed down by one inserted above it takes its
    /// answer along, and the newcomer in its old place has none.
    pub(crate) fn stale(&self, slot: &str, claimed: &HashSet<Key>) -> Option<&str> {
        let claimed: HashSet<String> = claimed.iter().map(hex).collect();
        self.entries
            .iter()
            .find(|entry| entry.slot == slot && !claimed.contains(&entry.key))
            .map(|entry| entry.answer.as_str())
    }

    /// File `answer` for `slot`, as the answer to `key`.
    pub(crate) fn put(&mut self, slot: &str, key: &Key, answer: &str) {
        self.entries.retain(|entry| entry.slot != slot);
        self.entries.push(Entry {
            slot: slot.to_string(),
            key: hex(key),
            answer: answer.to_string(),
        });
    }

    /// File for `slot` what `from` has filed there, as it is — an answer
    /// kept for a place whose text changed, until it is asked again.
    pub(crate) fn carry(&mut self, from: &Record, slot: &str) {
        if let Some(entry) = from.entries.iter().find(|entry| entry.slot == slot) {
            self.entries.retain(|entry| entry.slot != slot);
            self.entries.push(entry.clone());
        }
    }
}

/// The records, in one directory.
pub(crate) struct Store {
    root: PathBuf,
    max_bytes: u64,
    /// Serializes the read-modify-write of the open list across windows.
    open_lock: Mutex<()>,
}

impl Store {
    pub(crate) fn new(root: PathBuf, max_bytes: u64) -> Self {
        Self {
            root,
            max_bytes,
            open_lock: Mutex::new(()),
        }
    }

    /// What `lens_id` answered about `document`, empty when it answered
    /// nothing yet or the record cannot be read.
    pub(crate) fn load(&self, document: &Path, lens_id: &str) -> Record {
        let path = self.record_path(document, lens_id);
        let record = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            .filter(|record| record.version == VERSION)
            .unwrap_or_default();
        if !record.is_empty() {
            // Read is use: what was read last is kept longest.
            touch(&path);
        }
        record
    }

    /// Keep `record` as what `lens_id` answered about `document`.
    pub(crate) fn save(&self, document: &Path, lens_id: &str, record: &Record) -> io::Result<()> {
        let record = Record {
            version: VERSION,
            entries: record.entries.clone(),
        };
        let path = self.record_path(document, lens_id);
        write_atomically(&path, &serde_json::to_vec(&record)?)?;
        evict(&self.root, self.max_bytes, &path, &[OPEN_FILE]);
        Ok(())
    }

    /// The lenses `document` had open, in the order they were opened.
    pub(crate) fn remembered(&self, document: &Path) -> Vec<RememberedLens> {
        let _guard = self.open_lock.lock();
        self.open_list()
            .remove(&document_name(document))
            .unwrap_or_default()
    }

    /// Note that `document` has `lens_id` open, `shown` or hidden. A lens
    /// shown goes last, as the one shown most lately; one hidden keeps its
    /// place.
    pub(crate) fn remember(&self, document: &Path, lens_id: &str, shown: bool) {
        self.change_open_list(document, |lenses| {
            let lens = RememberedLens {
                id: lens_id.to_string(),
                shown,
            };
            match lenses.iter().position(|lens| lens.id == lens_id) {
                Some(index) if !shown => lenses[index] = lens,
                Some(index) => {
                    lenses.remove(index);
                    lenses.push(lens);
                }
                None => lenses.push(lens),
            }
        });
    }

    /// Note that `document` no longer has `lens_id` open.
    pub(crate) fn forget(&self, document: &Path, lens_id: &str) {
        self.change_open_list(document, |lenses| lenses.retain(|lens| lens.id != lens_id));
    }

    /// Delete what `lens_id` answered about `document`, and forget that the
    /// document had it open.
    pub(crate) fn delete(&self, document: &Path, lens_id: &str) {
        let path = self.record_path(document, lens_id);
        if let Err(error) = fs::remove_file(&path) {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(%error, "a lens's answers were not deleted");
            }
        }
        self.forget(document, lens_id);
    }

    fn change_open_list(&self, document: &Path, change: impl FnOnce(&mut Vec<RememberedLens>)) {
        let _guard = self.open_lock.lock();
        let mut list = self.open_list();
        let name = document_name(document);
        let before = list.get(&name).cloned();
        let lenses = list.entry(name.clone()).or_default();
        change(lenses);
        if lenses.is_empty() {
            list.remove(&name);
        }
        if list.get(&name).cloned() == before {
            // Opening a document says again what it had open; there is
            // nothing to write when that is what is on file.
            return;
        }
        let written = serde_json::to_vec(&list)
            .map_err(io::Error::from)
            .and_then(|bytes| write_atomically(&self.root.join(OPEN_FILE), &bytes));
        if let Err(error) = written {
            tracing::warn!(%error, "the lenses a document had open were not kept");
        }
    }

    fn open_list(&self) -> HashMap<String, Vec<RememberedLens>> {
        fs::read(self.root.join(OPEN_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn record_path(&self, document: &Path, lens_id: &str) -> PathBuf {
        self.root.join(record_file_name(
            &[&document_name(document), lens_id],
            "json",
        ))
    }
}

/// What a document is filed under: its path as the page names it.
fn document_name(document: &Path) -> String {
    document.to_string_lossy().into_owned()
}

fn hex(bytes: &Key) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "/notes/a.md";

    fn key(n: u8) -> Key {
        [n; 32]
    }

    fn store(dir: &tempfile::TempDir, max_bytes: u64) -> Store {
        Store::new(dir.path().to_path_buf(), max_bytes)
    }

    #[test]
    fn an_answer_kept_is_there_when_the_document_is_opened_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut record = Record::default();
        record.put("document", &key(1), "訳");
        store(&dir, MAX_BYTES)
            .save(Path::new(DOC), "translate", &record)
            .unwrap();

        let again = store(&dir, MAX_BYTES).load(Path::new(DOC), "translate");

        assert_eq!(again.fresh(&key(1)), Some("訳"));
        assert!(store(&dir, MAX_BYTES)
            .load(Path::new(DOC), "summarize")
            .is_empty());
        assert!(store(&dir, MAX_BYTES)
            .load(Path::new("/notes/b.md"), "translate")
            .is_empty());
    }

    #[test]
    fn answers_read_back_equal_the_same_answers_filed_afresh() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir, MAX_BYTES);
        let mut record = Record::default();
        record.put("document", &key(1), "訳");
        store.save(Path::new(DOC), "translate", &record).unwrap();

        assert_eq!(store.load(Path::new(DOC), "translate"), record);
    }

    #[test]
    fn a_changed_request_finds_its_places_old_answer_as_stale() {
        let mut record = Record::default();
        record.put("block:0", &key(1), "old");

        assert_eq!(record.fresh(&key(2)), None);
        assert_eq!(record.stale("block:0", &HashSet::new()), Some("old"));
        assert_eq!(record.stale("block:1", &HashSet::new()), None);
    }

    #[test]
    fn an_answer_found_fresh_elsewhere_is_no_stale_answer_for_its_old_place() {
        let mut record = Record::default();
        record.put("block:0", &key(1), "belongs to the block pushed down");

        assert_eq!(record.stale("block:0", &HashSet::from([key(1)])), None);
    }

    #[test]
    fn a_moved_block_is_still_answered_fresh() {
        let mut record = Record::default();
        record.put("block:3", &key(7), "answer");

        assert_eq!(record.fresh(&key(7)), Some("answer"));
    }

    #[test]
    fn a_place_holds_one_answer_and_a_carried_one_keeps_its_request() {
        let mut before = Record::default();
        before.put("block:0", &key(1), "first");
        before.put("block:0", &key(2), "second");
        before.put("block:1", &key(3), "gone");

        let mut next = Record::default();
        next.carry(&before, "block:0");

        assert_eq!(next.stale("block:0", &HashSet::new()), Some("second"));
        assert_eq!(next.fresh(&key(2)), Some("second"));
        assert_eq!(next.fresh(&key(1)), None);
        assert_eq!(next.stale("block:1", &HashSet::new()), None);
    }

    #[test]
    fn a_record_that_cannot_be_read_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir, MAX_BYTES);
        let path = store.record_path(Path::new(DOC), "translate");
        fs::write(&path, b"{ not json").unwrap();

        assert!(store.load(Path::new(DOC), "translate").is_empty());
    }

    #[test]
    fn records_past_the_limit_lose_the_one_used_longest_ago() {
        let dir = tempfile::tempdir().unwrap();
        let big = "x".repeat(1000);
        let store = store(&dir, 2500);
        let mut record = Record::default();
        record.put("document", &key(1), &big);
        for document in ["/a.md", "/b.md", "/c.md"] {
            store.save(Path::new(document), "l", &record).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert!(store.load(Path::new("/a.md"), "l").is_empty());
        assert!(!store.load(Path::new("/c.md"), "l").is_empty());
    }

    fn remembered(store: &Store, doc: &Path) -> Vec<(String, bool)> {
        store
            .remembered(doc)
            .into_iter()
            .map(|lens| (lens.id, lens.shown))
            .collect()
    }

    #[test]
    fn the_lenses_a_document_had_open_are_remembered_shown_last() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir, MAX_BYTES);
        let doc = Path::new(DOC);

        store.remember(doc, "translate", true);
        store.remember(doc, "terms", true);
        store.remember(doc, "translate", true);
        assert_eq!(
            remembered(&store, doc),
            [("terms".to_string(), true), ("translate".to_string(), true)]
        );

        // Hidden, it keeps its place: it is not the one last shown.
        store.remember(doc, "terms", false);
        assert_eq!(
            remembered(&store, doc),
            [
                ("terms".to_string(), false),
                ("translate".to_string(), true)
            ]
        );

        store.forget(doc, "terms");
        assert_eq!(remembered(&store, doc), [("translate".to_string(), true)]);
        assert!(store.remembered(Path::new("/other.md")).is_empty());
    }

    #[test]
    fn deleting_a_lens_s_answers_forgets_it_too() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir, MAX_BYTES);
        let doc = Path::new(DOC);
        let mut record = Record::default();
        record.put("document", &key(1), "訳");
        store.save(doc, "translate", &record).unwrap();
        store.save(doc, "terms", &record).unwrap();
        store.remember(doc, "translate", true);

        store.delete(doc, "translate");

        assert!(store.load(doc, "translate").is_empty());
        assert!(!store.load(doc, "terms").is_empty());
        assert!(store.remembered(doc).is_empty());
    }
}
