//! The one document a window is reading.
//!
//! A window used to hold a list of documents and an index into it. Closing a
//! document lost nothing — the history remembers everything read — so the
//! list was a second place to look for what the history already knew, and it
//! stopped working the moment there were more than a dozen of them.
//!
//! What is left is the document on screen and the way it was reached:
//! [`Document`] is that, and the back-and-forward history inside it is
//! movement within the document's own links, not between open windows.
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | `access` | Reading and updating the document on screen |
//! | `file_ops` | Opening a file, and the preferences window |
//! | `history` | Back, forward, and where the reader was on the page |

mod access;
mod file_ops;
mod history;

use crate::history::HistoryManager;
use std::path::{Path, PathBuf};

/// What the window is showing.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DocumentContent {
    /// Nothing yet, so the library is shown instead.
    #[default]
    None,
    /// A file from the filesystem.
    File(PathBuf),
    /// Markdown handed over directly, as the welcome page is.
    Inline(String),
    /// A file that cannot be shown, and why.
    FileError(PathBuf, String),
}

/// The document a window is reading, and how it got there.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub content: DocumentContent,
    /// Back and forward within the document's own links.
    pub history: HistoryManager,
}

impl Document {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        let file = file.into();
        let mut history = HistoryManager::new();
        history.push(file.clone());
        Self {
            content: DocumentContent::File(file),
            history,
        }
    }

    pub fn with_inline_content(content: impl Into<String>) -> Self {
        Self {
            content: DocumentContent::Inline(content.into()),
            history: HistoryManager::new(),
        }
    }

    /// The file being read, if what is shown came from one.
    pub fn file(&self) -> Option<&Path> {
        match &self.content {
            DocumentContent::File(path) | DocumentContent::FileError(path, _) => Some(path),
            _ => None,
        }
    }

    /// Whether there is no document to show — the library's condition.
    pub fn is_empty(&self) -> bool {
        matches!(
            self.content,
            DocumentContent::None | DocumentContent::Inline(_) | DocumentContent::FileError(_, _)
        )
    }

    /// The name to put in the header and the window title.
    pub fn display_name(&self) -> String {
        match &self.content {
            DocumentContent::File(path) | DocumentContent::FileError(path, _) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unnamed".to_string()),
            DocumentContent::Inline(_) => "Welcome".to_string(),
            DocumentContent::None => "Library".to_string(),
        }
    }

    /// Follow a link to another file, remembering where it came from.
    pub fn navigate_to(&mut self, file: impl Into<PathBuf>) {
        let file = file.into();
        self.history.push(file.clone());
        self.content = DocumentContent::File(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_window_holds_nothing() {
        let document = Document::default();
        assert_eq!(document.content, DocumentContent::None);
        assert!(document.is_empty());
        assert_eq!(document.file(), None);
        assert_eq!(document.display_name(), "Library");
    }

    #[test]
    fn a_file_is_its_own_first_history_entry() {
        let path = PathBuf::from("/test/file.md");
        let document = Document::new(path.clone());

        assert_eq!(document.content, DocumentContent::File(path.clone()));
        assert_eq!(document.file(), Some(path.as_path()));
        assert!(!document.is_empty());
        assert_eq!(document.history.current_path(), Some(path.as_path()));
    }

    #[test]
    fn inline_markdown_is_the_welcome_page() {
        let document = Document::with_inline_content("# Welcome");
        assert_eq!(
            document.content,
            DocumentContent::Inline("# Welcome".to_string())
        );
        assert!(document.is_empty());
        assert_eq!(document.file(), None);
        assert_eq!(document.display_name(), "Welcome");
    }

    #[test]
    fn a_file_that_cannot_be_shown_is_still_a_file() {
        let path = PathBuf::from("/path/to/binary.exe");
        let document = Document {
            content: DocumentContent::FileError(path.clone(), "Binary file".to_string()),
            ..Default::default()
        };
        assert!(document.is_empty());
        assert_eq!(document.file(), Some(path.as_path()));
        assert_eq!(document.display_name(), "binary.exe");
    }

    #[test]
    fn the_name_is_the_file_name_whatever_is_in_it() {
        assert_eq!(Document::new("/path/to/README").display_name(), "README");
        assert_eq!(
            Document::new("/path/to/日本語ファイル.md").display_name(),
            "日本語ファイル.md"
        );
        assert_eq!(
            Document::new("/path/to/notes_📝.md").display_name(),
            "notes_📝.md"
        );
        assert_eq!(
            Document::new("/path/to/.hidden.md").display_name(),
            ".hidden.md"
        );
    }

    #[test]
    fn a_path_with_no_name_is_unnamed() {
        let document = Document {
            content: DocumentContent::File(PathBuf::from("/")),
            ..Default::default()
        };
        assert_eq!(document.display_name(), "Unnamed");
    }

    #[test]
    fn following_a_link_records_where_it_came_from() {
        let mut document = Document::new("/test/first.md");
        document.navigate_to("/test/second.md");

        assert_eq!(
            document.content,
            DocumentContent::File(PathBuf::from("/test/second.md"))
        );
        assert!(document.history.can_go_back());
    }
}
