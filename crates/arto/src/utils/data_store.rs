//! Files the app keeps as data rather than as cache: one record per file in
//! a directory of its own, written whole or not at all, and trimmed to a
//! size by dropping the records used longest ago — or one value in one JSON
//! file ([`JsonFile`]), shared by every window ([`Shared`]).
//!
//! Data because a new build clears the caches, and what is kept here is the
//! reader's own — answers that took minutes to write, the version of a
//! document they last read.

use arto_config::Access;
use parking_lot::{RwLock, RwLockReadGuard};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::sync::broadcast;

/// Write `bytes` to `path` whole or not at all: a reader never sees half a
/// record, even when the app stops half-way through writing one.
///
/// What is kept here is the reader's own — copies of what they read, what
/// was said about it — so a directory or a record this creates is readable
/// by its owner alone, whatever the documents themselves allow.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    arto_config::write_atomically(path, bytes, Access::Private)
}

/// Remove the records in `root` used longest ago until the rest fit in
/// `max_bytes`, never `keep` — the one just written. Files named in `aside`
/// are not records: they are neither counted nor removed.
pub(crate) fn evict(root: &Path, max_bytes: u64, keep: &Path, aside: &[&str]) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut records: Vec<(PathBuf, u64, SystemTime)> = entries
        .filter_map(Result::ok)
        .filter(|entry| !aside.iter().any(|name| entry.file_name() == *name))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            Some((entry.path(), metadata.len(), metadata.modified().ok()?))
        })
        .collect();
    let mut total: u64 = records.iter().map(|(_, size, _)| size).sum();
    records.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in records {
        if total <= max_bytes {
            break;
        }
        if path != keep && fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
}

/// The name a record is filed under: the SHA-256 of `parts`, separated by
/// a NUL so that no two lists of parts run together into the same bytes,
/// in hex, with `extension`.
pub(crate) fn record_file_name(parts: &[&str], extension: &str) -> String {
    let mut hash = Sha256::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            hash.update([0]);
        }
        hash.update(part.as_bytes());
    }
    let digest: [u8; 32] = hash.finalize().into();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{hex}.{extension}")
}

/// Mark `path` as just used, so that it is the last to be evicted.
pub(crate) fn touch(path: &Path) {
    if let Ok(file) = fs::File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// `<platform data dir>/arto/<name>`, or `~/.arto/<name>` when the platform
/// has no data directory, or just `<name>` as a last resort.
///
/// This is for what the app writes about itself — bookmarks, the visit
/// history, the last window's state — as opposed to `config.json`, which a
/// reader edits by hand and which lives in the configuration directory
/// instead.
pub(crate) fn app_data_file(name: &str) -> PathBuf {
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("arto");
        path.push(name);
        return path;
    }

    if let Some(mut path) = dirs::home_dir() {
        path.push(".arto");
        path.push(name);
        return path;
    }

    PathBuf::from(name)
}

/// One value kept as one JSON file.
///
/// What is kept this way is a convenience — a list the reader built up, the
/// size of the last window — so a file that is missing or cannot be read
/// starts the value over rather than stopping anything.
pub(crate) struct JsonFile<T> {
    path: PathBuf,
    discard_when: Option<fn(&T) -> bool>,
}

impl<T: Serialize + DeserializeOwned + Default> JsonFile<T> {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            discard_when: None,
        }
    }

    /// Remove the file instead of writing a value `empty` calls empty.
    pub fn discarded_when(self, empty: fn(&T) -> bool) -> Self {
        Self {
            discard_when: Some(empty),
            ..self
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value the file holds, or the default when there is none to read.
    pub fn load_or_default(&self) -> T {
        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return T::default(),
            Err(err) => {
                tracing::warn!(path = %self.path.display(), %err, "Ignoring an unreadable file");
                return T::default();
            }
        };
        serde_json::from_str(&content).unwrap_or_else(|err| {
            tracing::warn!(path = %self.path.display(), %err, "Ignoring a file that does not parse");
            T::default()
        })
    }

    pub fn save(&self, value: &T) -> io::Result<()> {
        if self.discard_when.is_some_and(|empty| empty(value)) {
            return match fs::remove_file(&self.path) {
                Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            };
        }
        write_atomically(&self.path, &serde_json::to_vec_pretty(value)?)
    }
}

/// A value every window shares, kept on disk, with word sent out whenever it
/// changes so that open windows draw it again.
pub(crate) struct Shared<T> {
    value: RwLock<T>,
    file: JsonFile<T>,
    changed: broadcast::Sender<()>,
}

impl<T: Serialize + DeserializeOwned + Default> Shared<T> {
    /// What `file` holds, or the default.
    pub fn load(file: JsonFile<T>) -> Self {
        let value = file.load_or_default();
        Self::new(file, value)
    }

    /// `value`, kept in `file` from the next change on.
    pub fn new(file: JsonFile<T>, value: T) -> Self {
        Self {
            value: RwLock::new(value),
            file,
            changed: broadcast::channel(16).0,
        }
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.value.read()
    }

    /// Told after every change [`Shared::update`] keeps.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.changed.subscribe()
    }

    /// Change the value with `change`, which says whether it changed
    /// anything. Only a change is written out and announced: a request that
    /// leaves the value as it was neither rewrites the file nor makes every
    /// window draw again.
    pub fn update<R>(&self, change: impl FnOnce(&mut T) -> (R, bool)) -> R {
        let (result, changed) = {
            let mut value = self.value.write();
            let (result, changed) = change(&mut value);
            if changed {
                self.save_locked(&value);
            }
            (result, changed)
        };
        if changed {
            let _ = self.changed.send(());
        }
        result
    }

    /// Write the value out as it stands, without announcing anything: for a
    /// change [`Shared::update`] was told to keep in memory alone.
    pub fn save(&self) {
        self.save_locked(&self.value.read());
    }

    fn save_locked(&self, value: &T) {
        if let Err(err) = self.file.save(value) {
            tracing::warn!(path = %self.file.path().display(), %err, "Failed to save");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn eviction_drops_the_records_used_longest_ago_but_never_the_kept_one() {
        let dir = tempfile::tempdir().unwrap();
        let names = ["a", "b", "c"];
        for name in names {
            fs::write(dir.path().join(name), vec![0u8; 1000]).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        // `a` is the oldest, but it is the one being kept.
        let keep = dir.path().join("a");

        evict(dir.path(), 2000, &keep, &[]);

        assert!(keep.exists());
        assert!(!dir.path().join("b").exists());
        assert!(dir.path().join("c").exists());
    }

    #[test]
    fn files_set_aside_are_neither_counted_nor_removed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("list.json"), vec![0u8; 1000]).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(dir.path().join("a"), vec![0u8; 1000]).unwrap();

        evict(dir.path(), 1000, &dir.path().join("a"), &["list.json"]);

        assert!(dir.path().join("list.json").exists());
        assert!(dir.path().join("a").exists());
    }

    #[test]
    fn a_record_touched_is_kept_longer() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a", "b"] {
            fs::write(dir.path().join(name), vec![0u8; 1000]).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        touch(&dir.path().join("a"));

        evict(dir.path(), 1000, &dir.path().join("none"), &[]);

        assert!(dir.path().join("a").exists());
        assert!(!dir.path().join("b").exists());
    }

    // === JsonFile ===

    #[derive(Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    struct List {
        items: Vec<String>,
    }

    fn list(items: &[&str]) -> List {
        List {
            items: items.iter().map(|item| item.to_string()).collect(),
        }
    }

    #[test]
    fn app_data_lives_in_the_platform_data_directory() {
        let Some(dir) = dirs::data_local_dir() else {
            return;
        };
        assert_eq!(
            app_data_file("bookmarks.json"),
            dir.join("arto").join("bookmarks.json")
        );
    }

    #[test]
    fn a_missing_file_reads_as_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let file = JsonFile::<List>::at(dir.path().join("list.json"));

        assert_eq!(file.load_or_default(), List::default());
    }

    #[test]
    fn a_file_that_does_not_parse_reads_as_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.json");
        fs::write(&path, "{ not json").unwrap();

        assert_eq!(
            JsonFile::<List>::at(&path).load_or_default(),
            List::default()
        );
    }

    #[test]
    fn what_is_saved_is_loaded_again() {
        let dir = tempfile::tempdir().unwrap();
        let file = JsonFile::<List>::at(dir.path().join("nested").join("list.json"));

        file.save(&list(&["a", "b"])).unwrap();

        assert_eq!(file.load_or_default(), list(&["a", "b"]));
    }

    #[test]
    fn a_saved_file_is_pretty_printed_json() {
        let dir = tempfile::tempdir().unwrap();
        let file = JsonFile::<List>::at(dir.path().join("list.json"));

        file.save(&list(&["a"])).unwrap();

        assert_eq!(
            fs::read_to_string(file.path()).unwrap(),
            serde_json::to_string_pretty(&list(&["a"])).unwrap()
        );
    }

    #[test]
    fn a_value_called_empty_removes_the_file_instead() {
        let dir = tempfile::tempdir().unwrap();
        let file = JsonFile::<List>::at(dir.path().join("list.json"))
            .discarded_when(|list| list.items.is_empty());
        file.save(&list(&["a"])).unwrap();

        file.save(&List::default()).unwrap();
        assert!(!file.path().exists());

        // Nothing there to remove is not an error.
        file.save(&List::default()).unwrap();
    }

    // === Shared ===

    fn shared(dir: &tempfile::TempDir) -> Shared<List> {
        Shared::load(JsonFile::at(dir.path().join("list.json")))
    }

    #[test]
    fn a_shared_value_starts_from_its_file() {
        let dir = tempfile::tempdir().unwrap();
        JsonFile::at(dir.path().join("list.json"))
            .save(&list(&["a"]))
            .unwrap();

        assert_eq!(*shared(&dir).read(), list(&["a"]));
    }

    #[test]
    fn a_change_is_saved_and_announced() {
        let dir = tempfile::tempdir().unwrap();
        let shared = shared(&dir);
        let mut changed = shared.subscribe();

        let answer = shared.update(|list| {
            list.items.push("a".to_string());
            (42, true)
        });

        assert_eq!(answer, 42);
        assert_eq!(
            JsonFile::<List>::at(dir.path().join("list.json")).load_or_default(),
            list(&["a"])
        );
        assert!(changed.try_recv().is_ok());
    }

    #[test]
    fn what_changed_nothing_is_neither_saved_nor_announced() {
        let dir = tempfile::tempdir().unwrap();
        let shared = shared(&dir);
        let mut changed = shared.subscribe();

        shared.update(|_| ((), false));

        assert!(!dir.path().join("list.json").exists());
        assert!(changed.try_recv().is_err());
    }

    #[test]
    fn a_change_kept_in_memory_goes_out_with_the_next_save() {
        let dir = tempfile::tempdir().unwrap();
        let shared = shared(&dir);
        let mut changed = shared.subscribe();

        shared.update(|list| {
            list.items.push("a".to_string());
            ((), false)
        });
        assert!(!dir.path().join("list.json").exists());
        shared.save();

        assert_eq!(
            JsonFile::<List>::at(dir.path().join("list.json")).load_or_default(),
            list(&["a"])
        );
        assert!(changed.try_recv().is_err());
    }

    #[test]
    fn a_file_name_is_the_hash_of_its_parts() {
        let mut hash = Sha256::new();
        hash.update(b"/notes/a.md");
        hash.update([0]);
        hash.update(b"translate");
        let digest: [u8; 32] = hash.finalize().into();
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();

        assert_eq!(
            record_file_name(&["/notes/a.md", "translate"], "json"),
            format!("{hex}.json")
        );
        assert_ne!(
            record_file_name(&["/notes/a.md", "translate"], "json"),
            record_file_name(&["/notes/a.mdtranslate"], "json")
        );
    }
}
