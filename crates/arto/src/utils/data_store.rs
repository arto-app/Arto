//! Files the app keeps as data rather than as cache: one record per file in
//! a directory of its own, written whole or not at all, and trimmed to a
//! size by dropping the records used longest ago.
//!
//! Data because a new build clears the caches, and what is kept here is the
//! reader's own — answers that took minutes to write, the version of a
//! document they last read.

use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Write `bytes` to `path` whole or not at all: a reader never sees half a
/// record, even when the app stops half-way through writing one.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let partial = path.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_record_is_written_into_a_directory_that_did_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("record.json");

        write_atomically(&path, b"{}").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"{}");
        assert!(!path.with_extension("partial").exists());
    }

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
