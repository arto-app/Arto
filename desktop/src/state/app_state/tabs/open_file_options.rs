//! Options for opening files in tabs.

/// Options for opening a file.
///
/// These options control how a file is opened and whether to jump to a specific line.
#[derive(Debug, Clone, Default)]
pub struct OpenFileOptions {
    /// Jump to this line number after opening the file.
    /// Line numbers are 1-indexed (first line is 1).
    pub line: Option<usize>,

    /// Force opening in the current tab instead of reusing existing tabs.
    /// Used for in-tab navigation (e.g., markdown links).
    pub in_current_tab: bool,
}

impl OpenFileOptions {
    /// Create options for jumping to a specific line.
    pub fn with_line(line: usize) -> Self {
        Self {
            line: Some(line),
            in_current_tab: false,
        }
    }

    /// Create options for opening in the current tab (markdown link navigation).
    #[allow(dead_code)]
    pub fn in_current_tab() -> Self {
        Self {
            line: None,
            in_current_tab: true,
        }
    }
}
