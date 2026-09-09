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
    /// Nothing yet.
    #[default]
    None,
    /// A file from the filesystem.
    File(PathBuf),
    /// Markdown the app itself supplies, such as the welcome text.
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
    pub fn with_inline_content(content: impl Into<String>) -> Self {
        Self {
            content: DocumentContent::Inline(content.into()),
            history: HistoryManager::new(),
        }
    }

    pub fn new(file: impl Into<PathBuf>) -> Self {
        let file = file.into();
        let mut history = HistoryManager::new();
        history.push(file.clone());
        Self {
            content: DocumentContent::File(file),
            history,
        }
    }

    /// The file being read, if what is shown came from one.
    pub fn file(&self) -> Option<&Path> {
        match &self.content {
            DocumentContent::File(path) | DocumentContent::FileError(path, _) => Some(path),
            _ => None,
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
        assert_eq!(document.file(), None);
    }

    #[test]
    fn a_file_is_its_own_first_history_entry() {
        let path = PathBuf::from("/test/file.md");
        let document = Document::new(path.clone());

        assert_eq!(document.content, DocumentContent::File(path.clone()));
        assert_eq!(document.file(), Some(path.as_path()));
        assert_eq!(document.history.current_path(), Some(path.as_path()));
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
