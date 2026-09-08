//! Paths written the way a reader would say them.

use std::path::Path;

/// The folder a document sits in, shortened to its home-relative form.
///
/// A file name on its own does not say which of three `README.md`s it is, and
/// an absolute path says it in a form nobody reads: `~/notes` is a place,
/// `/Users/someone/notes` is a string.
pub fn parent_label(path: &Path) -> String {
    let Some(parent) = path.parent() else {
        return String::new();
    };
    let parent = parent.to_string_lossy().to_string();
    match dirs::home_dir() {
        Some(home) => match parent.strip_prefix(&home.to_string_lossy().to_string()) {
            Some(rest) => format!("~{rest}"),
            None => parent,
        },
        None => parent,
    }
}

/// The path as its own folders spell it.
///
/// macOS matches names without regard to case, so `/users/alisue/arto` opens
/// exactly the directory `/Users/alisue/Arto` does and is not the same string.
/// A path arrives spelled by whoever wrote it — a state file, a shell, a
/// Finder association — and the difference shows twice: the folder's name is
/// drawn as it was spelled rather than as it is called, and the rows below it
/// are read from directory entries that no longer match it.
///
/// Elsewhere the filesystem distinguishes the two, so there is nothing to
/// repair and this is the path itself. A component that cannot be read is
/// left as it came, along with everything after it.
#[cfg(target_os = "macos")]
pub fn true_spelling(path: &Path) -> std::path::PathBuf {
    use std::path::Component;

    let mut out = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let entry = std::fs::read_dir(&out).ok().and_then(|entries| {
                    entries
                        .flatten()
                        .map(|entry| entry.file_name())
                        .find(|entry| {
                            entry != name
                                && entry.to_string_lossy().to_lowercase()
                                    == name.to_string_lossy().to_lowercase()
                        })
                });
                match entry {
                    Some(entry) => out.push(entry),
                    None => out.push(name),
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(not(target_os = "macos"))]
pub fn true_spelling(path: &Path) -> std::path::PathBuf {
    path.to_path_buf()
}

/// A document as the lists name it: the folder it sits in, then its own name.
///
/// `samples/README.md`. A bare file name cannot tell three README.md apart and
/// a whole path does not fit a column, so every list that offers documents
/// says exactly this much and keeps the path for the row's title.
pub fn short_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    match folder_name(path) {
        folder if folder.is_empty() => name,
        folder => format!("{folder}/{name}"),
    }
}

/// The same name in its two halves: the folder with its slash, and the file.
///
/// The folder is there to tell two documents of one name apart, and that is
/// all it is there for, so a list draws it a step quieter than the name it
/// qualifies. Splitting is this function's job rather than each list's.
pub fn split_name(path: &Path) -> (String, String) {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    match folder_name(path) {
        folder if folder.is_empty() => (String::new(), name),
        folder => (format!("{folder}/"), name),
    }
}

/// The name of the folder a document sits in.
///
/// What a narrow column can afford: `daily.md` beside `notes` says which of
/// the three `daily.md`s this is, where the whole path would be the row and
/// leave no room for the name. The path itself is a tooltip away.
pub fn folder_name(path: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(target_os = "macos")]
    #[test]
    fn true_spelling_answers_with_the_name_the_folder_has() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Arto/Docs")).unwrap();

        let asked = dir.path().join("arto/docs");
        assert_eq!(true_spelling(&asked), dir.path().join("Arto/Docs"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn true_spelling_leaves_a_path_that_is_not_there() {
        let dir = tempfile::TempDir::new().unwrap();
        let asked = dir.path().join("nowhere/at/all");

        assert_eq!(true_spelling(&asked), asked);
    }

    #[test]
    fn a_path_outside_home_is_left_as_it_is() {
        assert_eq!(
            parent_label(&PathBuf::from("/etc/hosts")),
            "/etc".to_string()
        );
    }

    #[test]
    fn a_path_under_home_is_written_with_a_tilde() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let path = home.join("notes").join("daily.md");
        // The label replaces the home prefix and leaves the rest of the path
        // as the platform spells it, separator included — `~/notes` on Unix
        // and `~\notes` on Windows.
        let expected = format!("~{}notes", std::path::MAIN_SEPARATOR);
        assert_eq!(parent_label(&path), expected);
    }

    #[test]
    fn a_document_is_named_by_its_folder_and_itself() {
        assert_eq!(
            short_name(&PathBuf::from("/a/b/samples/README.md")),
            "samples/README.md".to_string()
        );
    }

    #[test]
    fn a_document_with_no_folder_is_named_by_itself() {
        assert_eq!(
            short_name(&PathBuf::from("/README.md")),
            "README.md".to_string()
        );
    }

    #[test]
    fn the_two_halves_join_back_into_the_name() {
        let path = PathBuf::from("/a/samples/README.md");
        let (folder, name) = split_name(&path);
        assert_eq!(folder, "samples/".to_string());
        assert_eq!(name, "README.md".to_string());
        assert_eq!(format!("{folder}{name}"), short_name(&path));
    }

    #[test]
    fn a_folder_is_named_by_its_last_component() {
        assert_eq!(
            folder_name(&PathBuf::from("/a/b/notes/daily.md")),
            "notes".to_string()
        );
    }

    #[test]
    fn a_document_at_the_root_has_no_folder_name() {
        assert_eq!(folder_name(&PathBuf::from("/daily.md")), String::new());
    }

    #[test]
    fn a_document_with_no_folder_has_no_label() {
        assert_eq!(parent_label(&PathBuf::from("/")), String::new());
    }
}
