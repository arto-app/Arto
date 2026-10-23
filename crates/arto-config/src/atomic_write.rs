//! Writing a file whole or not at all.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Who may read what [`write_atomically`] puts down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The owner alone: on Unix, a new directory is `0700` and the file
    /// `0600`, whatever it allowed before. For what the app keeps of the
    /// reader's own.
    Private,
    /// What the file already allows, or the process's default for a file
    /// that is new. For a file the reader keeps and edits by hand, whose
    /// mode is theirs to choose.
    Kept,
}

/// Write `bytes` to `path` whole or not at all: a reader never sees half a
/// file, even when the writer stops half-way through.
///
/// The bytes go to a file of their own beside the one they replace, which
/// is then renamed into place; two writers saving the same file at once
/// each put down a whole one rather than one renaming the other's
/// half-written file into place.
///
/// A symlink is followed to the file it names, however many links lead
/// there, and that file is the one replaced, so the link stays a link: a
/// configuration kept in a dotfiles repository and linked into place goes on
/// being kept there. A link to a file that does not exist yet creates that
/// file, as writing through the link always has.
pub fn write_atomically(path: &Path, bytes: &[u8], access: Access) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let target = follow_links(path)?;
    let dir = target.parent().unwrap_or(Path::new("."));
    create_dir(dir, access)?;
    let partial = target.with_extension(format!(
        "partial-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let written = create_file(&partial, &target, access)
        .and_then(|mut file| file.write_all(bytes))
        .and_then(|()| fs::rename(&partial, &target));
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    written
}

/// How many links are followed before a chain is taken to be a loop, as
/// the kernel's own `ELOOP` bound does.
const MAX_LINKS: usize = 40;

/// The file `path` names once every symlink on the way is followed; `path`
/// itself when it is not a link or does not exist.
fn follow_links(path: &Path) -> io::Result<PathBuf> {
    let mut path = path.to_path_buf();
    for _ in 0..MAX_LINKS {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let link = fs::read_link(&path)?;
                path = match path.parent() {
                    Some(parent) => parent.join(link),
                    None => link,
                };
            }
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(path),
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(format!(
        "more than {MAX_LINKS} symlinks lead to {}",
        path.display()
    )))
}

fn create_dir(dir: &Path, access: Access) -> io::Result<()> {
    match access {
        Access::Private => create_private_dir(dir),
        Access::Kept => fs::create_dir_all(dir),
    }
}

/// The file the bytes go to before they replace `target`.
fn create_file(partial: &Path, target: &Path, access: Access) -> io::Result<fs::File> {
    match access {
        Access::Private => create_private_file(partial),
        Access::Kept => {
            let file = fs::File::options()
                .write(true)
                .create_new(true)
                .open(partial)?;
            match fs::metadata(target) {
                Ok(existing) => file.set_permissions(existing.permissions())?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            Ok(file)
        }
    }
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::File::options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> io::Result<fs::File> {
    fs::File::options().write(true).create_new(true).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_file_is_written_into_a_directory_that_did_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("record.json");

        write_atomically(&path, b"{}", Access::Private).unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"{}");
        assert_eq!(entries(path.parent().unwrap()), ["record.json"]);
    }

    #[test]
    fn a_plain_file_is_replaced_and_nothing_is_left_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "old").unwrap();

        write_atomically(&path, b"new", Access::Kept).unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(entries(dir.path()), ["config.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_stays_a_symlink_and_the_file_it_names_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let dotfiles = dir.path().join("dotfiles");
        fs::create_dir(&dotfiles).unwrap();
        let real = dotfiles.join("config.json");
        fs::write(&real, "old").unwrap();
        let config = dir.path().join("arto");
        fs::create_dir(&config).unwrap();
        let link = config.join("config.json");
        std::os::unix::fs::symlink("../dotfiles/config.json", &link).unwrap();

        write_atomically(&link, b"new", Access::Kept).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&real).unwrap(), b"new");
        assert_eq!(entries(&dotfiles), ["config.json"]);
        assert_eq!(entries(&config), ["config.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_chain_of_symlinks_is_followed_to_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        fs::write(&real, "old").unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("middle.json")).unwrap();
        let link = dir.path().join("config.json");
        std::os::unix::fs::symlink("middle.json", &link).unwrap();

        write_atomically(&link, b"new", Access::Kept).unwrap();

        assert_eq!(fs::read(&real).unwrap(), b"new");
        assert!(fs::symlink_metadata(dir.path().join("middle.json"))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_creates_the_file_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles").join("config.json");
        let link = dir.path().join("config.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_atomically(&link, b"new", Access::Kept).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&real).unwrap(), b"new");
    }

    #[cfg(unix)]
    #[test]
    fn a_loop_of_symlinks_is_an_error_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.json");
        let b = dir.path().join("b.json");
        std::os::unix::fs::symlink(&b, &a).unwrap();
        std::os::unix::fs::symlink(&a, &b).unwrap();

        assert!(write_atomically(&a, b"new", Access::Kept).is_err());
        assert_eq!(entries(dir.path()), ["a.json", "b.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_kept_file_keeps_the_mode_it_had() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        write_atomically(&path, b"new", Access::Kept).unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_new_kept_file_gets_the_mode_any_new_file_would() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain.json");
        fs::write(&plain, "").unwrap();
        let path = dir.path().join("config.json");

        write_atomically(&path, b"new", Access::Kept).unwrap();

        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), mode(&plain));
    }

    #[cfg(unix)]
    #[test]
    fn a_private_file_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("record.json");

        write_atomically(&path, b"{}", Access::Private).unwrap();

        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }
}
