//! The files a document names, as the host holds them.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where the files a document links to and draws live.
///
/// A render asks it whether a linked document exists and for the images it
/// draws. [`FileSystem`] answers from this machine's disk; a host that keeps
/// documents elsewhere — in a browser, on a phone, in a database — answers
/// with an implementation of its own, addressing files by the same paths it
/// renders the document at.
pub trait Files {
    /// The regular file `path` names, or `None` when there is none.
    ///
    /// [`FileStat::path`] is the one spelling of the file: two paths naming
    /// the same file stat to the same `path`, which is what the id of a
    /// deferred image is made from.
    fn stat(&self, path: &Path) -> Option<FileStat>;

    /// The bytes of the file `path` names, or `None` when it cannot be read
    /// or holds more than `limit` bytes.
    fn read(&self, path: &Path, limit: u64) -> Option<Vec<u8>>;
}

/// A file as [`Files::stat`] found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub path: PathBuf,
    pub len: u64,
    /// When the content last changed, when the host knows. A different time
    /// gives a deferred image a different URL, so a cache keyed on the URL
    /// does not keep showing what the file used to hold.
    pub modified: Option<SystemTime>,
}

/// The files on this machine's disk.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileSystem;

impl Files for FileSystem {
    fn stat(&self, path: &Path) -> Option<FileStat> {
        let path = path.canonicalize().ok()?;
        let metadata = std::fs::metadata(&path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        Some(FileStat {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            path,
        })
    }

    /// Bounded by `Read::take` as well as the reported length, so a file
    /// whose length is unreliable — a device file, one that grows between
    /// the stat and the read — cannot exceed `limit`.
    fn read(&self, path: &Path, limit: u64) -> Option<Vec<u8>> {
        if std::fs::metadata(path).ok()?.len() > limit {
            return None;
        }
        let file = std::fs::File::open(path).ok()?;
        let mut buf = Vec::new();
        file.take(limit + 1).read_to_end(&mut buf).ok()?;
        (buf.len() as u64 <= limit).then_some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_file_over_the_limit_is_not_read() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("big.png");
        std::fs::write(&path, vec![0u8; 2048]).unwrap();

        assert_eq!(FileSystem.read(&path, 1024), None);
    }

    #[test]
    fn a_file_at_the_limit_is_read_whole() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("ok.png");
        std::fs::write(&path, vec![0u8; 1024]).unwrap();

        assert_eq!(
            FileSystem.read(&path, 1024).map(|bytes| bytes.len()),
            Some(1024)
        );
    }

    #[test]
    fn a_directory_is_not_a_file() {
        let temp = TempDir::new().unwrap();

        assert_eq!(FileSystem.stat(temp.path()), None);
    }

    #[test]
    fn two_spellings_of_one_file_stat_to_one_path() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir(temp.path().join("sub")).unwrap();
        std::fs::write(temp.path().join("a.png"), [1]).unwrap();

        let direct = FileSystem.stat(&temp.path().join("a.png")).unwrap();
        let roundabout = FileSystem.stat(&temp.path().join("sub/../a.png")).unwrap();

        assert_eq!(direct.path, roundabout.path);
    }
}
