use std::fmt;
use std::str::FromStr;

/// All available actions that keybindings can map to.
///
/// Action string identifiers use dot-separated hierarchical names
/// (e.g., `"scroll.down"`, `"tab.go.1"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    // Scroll
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollHalfPageUp,
    ScrollHalfPageDown,
    ScrollTop,
    ScrollBottom,

    // Tab
    TabNext,
    TabPrev,
    TabClose,
    TabCloseAll,
    TabNew,
    TabUndoClose,
    /// Go to tab N (1-indexed, 9 = last tab)
    TabGo(usize),

    // History
    HistoryBack,
    HistoryForward,

    // Search
    SearchOpen,
    SearchNext,
    SearchPrev,
    SearchClose,

    // Zoom
    ZoomIn,
    ZoomOut,
    ZoomReset,

    // Clipboard
    ClipboardCopyUrl,
    ClipboardCopySelection,

    // Window
    WindowNew,
    WindowClose,
    WindowCloseAllChildren,
    WindowCloseAll,
    WindowToggleSidebar,
    WindowToggleRightSidebar,

    // Focus
    FocusLeftSidebar,
    FocusRightSidebar,
    FocusQuickAccess,
    FocusContent,

    // File
    FileOpen,
    FileOpenDirectory,
    FileRevealInFinder,
    FileCopyPath,

    // App
    AppAbout,
    AppPreferences,
    AppHomepage,

    // Cursor (panel navigation)
    CursorDown,
    CursorUp,
    CursorEnter,
    CursorCollapse,

    // Directory (sidebar navigation)
    DirectoryParent,
    DirectoryBack,
    DirectoryForward,

    // General
    Cancel,
}

/// Error type for parsing action strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseActionError(pub String);

impl fmt::Display for ParseActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown action: {}", self.0)
    }
}

impl std::error::Error for ParseActionError {}

impl FromStr for Action {
    type Err = ParseActionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle tab.go.N pattern
        if let Some(n_str) = s.strip_prefix("tab.go.") {
            if let Ok(n) = n_str.parse::<usize>() {
                if (1..=9).contains(&n) {
                    return Ok(Action::TabGo(n));
                }
            }
            return Err(ParseActionError(s.to_string()));
        }

        match s {
            "scroll.up" => Ok(Action::ScrollUp),
            "scroll.down" => Ok(Action::ScrollDown),
            "scroll.page_up" => Ok(Action::ScrollPageUp),
            "scroll.page_down" => Ok(Action::ScrollPageDown),
            "scroll.half_page_up" => Ok(Action::ScrollHalfPageUp),
            "scroll.half_page_down" => Ok(Action::ScrollHalfPageDown),
            "scroll.top" => Ok(Action::ScrollTop),
            "scroll.bottom" => Ok(Action::ScrollBottom),

            "tab.next" => Ok(Action::TabNext),
            "tab.prev" => Ok(Action::TabPrev),
            "tab.close" => Ok(Action::TabClose),
            "tab.close_all" => Ok(Action::TabCloseAll),
            "tab.new" => Ok(Action::TabNew),
            "tab.undo_close" => Ok(Action::TabUndoClose),

            "history.back" => Ok(Action::HistoryBack),
            "history.forward" => Ok(Action::HistoryForward),

            "search.open" => Ok(Action::SearchOpen),
            "search.next" => Ok(Action::SearchNext),
            "search.prev" => Ok(Action::SearchPrev),
            "search.close" => Ok(Action::SearchClose),

            "zoom.in" => Ok(Action::ZoomIn),
            "zoom.out" => Ok(Action::ZoomOut),
            "zoom.reset" => Ok(Action::ZoomReset),

            "clipboard.copy_url" => Ok(Action::ClipboardCopyUrl),
            "clipboard.copy_selection" => Ok(Action::ClipboardCopySelection),

            "window.new" => Ok(Action::WindowNew),
            "window.close" => Ok(Action::WindowClose),
            "window.close_all_children" => Ok(Action::WindowCloseAllChildren),
            "window.close_all" => Ok(Action::WindowCloseAll),
            "window.toggle_sidebar" => Ok(Action::WindowToggleSidebar),
            "window.toggle_right_sidebar" => Ok(Action::WindowToggleRightSidebar),

            "focus.left_sidebar" => Ok(Action::FocusLeftSidebar),
            "focus.right_sidebar" => Ok(Action::FocusRightSidebar),
            "focus.quick_access" => Ok(Action::FocusQuickAccess),
            "focus.content" => Ok(Action::FocusContent),

            "file.open" => Ok(Action::FileOpen),
            "file.open_directory" => Ok(Action::FileOpenDirectory),
            "file.reveal_in_finder" => Ok(Action::FileRevealInFinder),
            "file.copy_path" => Ok(Action::FileCopyPath),

            "app.about" => Ok(Action::AppAbout),
            "app.preferences" => Ok(Action::AppPreferences),
            "app.homepage" => Ok(Action::AppHomepage),

            "cursor.down" => Ok(Action::CursorDown),
            "cursor.up" => Ok(Action::CursorUp),
            "cursor.enter" => Ok(Action::CursorEnter),
            "cursor.collapse" => Ok(Action::CursorCollapse),

            "directory.parent" => Ok(Action::DirectoryParent),
            "directory.back" => Ok(Action::DirectoryBack),
            "directory.forward" => Ok(Action::DirectoryForward),

            "action.cancel" => Ok(Action::Cancel),

            _ => Err(ParseActionError(s.to_string())),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::ScrollUp => write!(f, "scroll.up"),
            Action::ScrollDown => write!(f, "scroll.down"),
            Action::ScrollPageUp => write!(f, "scroll.page_up"),
            Action::ScrollPageDown => write!(f, "scroll.page_down"),
            Action::ScrollHalfPageUp => write!(f, "scroll.half_page_up"),
            Action::ScrollHalfPageDown => write!(f, "scroll.half_page_down"),
            Action::ScrollTop => write!(f, "scroll.top"),
            Action::ScrollBottom => write!(f, "scroll.bottom"),

            Action::TabNext => write!(f, "tab.next"),
            Action::TabPrev => write!(f, "tab.prev"),
            Action::TabClose => write!(f, "tab.close"),
            Action::TabCloseAll => write!(f, "tab.close_all"),
            Action::TabNew => write!(f, "tab.new"),
            Action::TabUndoClose => write!(f, "tab.undo_close"),
            Action::TabGo(n) => write!(f, "tab.go.{n}"),

            Action::HistoryBack => write!(f, "history.back"),
            Action::HistoryForward => write!(f, "history.forward"),

            Action::SearchOpen => write!(f, "search.open"),
            Action::SearchNext => write!(f, "search.next"),
            Action::SearchPrev => write!(f, "search.prev"),
            Action::SearchClose => write!(f, "search.close"),

            Action::ZoomIn => write!(f, "zoom.in"),
            Action::ZoomOut => write!(f, "zoom.out"),
            Action::ZoomReset => write!(f, "zoom.reset"),

            Action::ClipboardCopyUrl => write!(f, "clipboard.copy_url"),
            Action::ClipboardCopySelection => write!(f, "clipboard.copy_selection"),

            Action::WindowNew => write!(f, "window.new"),
            Action::WindowClose => write!(f, "window.close"),
            Action::WindowCloseAllChildren => write!(f, "window.close_all_children"),
            Action::WindowCloseAll => write!(f, "window.close_all"),
            Action::WindowToggleSidebar => write!(f, "window.toggle_sidebar"),
            Action::WindowToggleRightSidebar => write!(f, "window.toggle_right_sidebar"),

            Action::FocusLeftSidebar => write!(f, "focus.left_sidebar"),
            Action::FocusRightSidebar => write!(f, "focus.right_sidebar"),
            Action::FocusQuickAccess => write!(f, "focus.quick_access"),
            Action::FocusContent => write!(f, "focus.content"),

            Action::FileOpen => write!(f, "file.open"),
            Action::FileOpenDirectory => write!(f, "file.open_directory"),
            Action::FileRevealInFinder => write!(f, "file.reveal_in_finder"),
            Action::FileCopyPath => write!(f, "file.copy_path"),

            Action::AppAbout => write!(f, "app.about"),
            Action::AppPreferences => write!(f, "app.preferences"),
            Action::AppHomepage => write!(f, "app.homepage"),

            Action::CursorDown => write!(f, "cursor.down"),
            Action::CursorUp => write!(f, "cursor.up"),
            Action::CursorEnter => write!(f, "cursor.enter"),
            Action::CursorCollapse => write!(f, "cursor.collapse"),

            Action::DirectoryParent => write!(f, "directory.parent"),
            Action::DirectoryBack => write!(f, "directory.back"),
            Action::DirectoryForward => write!(f, "directory.forward"),

            Action::Cancel => write!(f, "action.cancel"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scroll_actions() {
        assert_eq!("scroll.up".parse::<Action>().unwrap(), Action::ScrollUp);
        assert_eq!("scroll.down".parse::<Action>().unwrap(), Action::ScrollDown);
        assert_eq!("scroll.top".parse::<Action>().unwrap(), Action::ScrollTop);
        assert_eq!(
            "scroll.bottom".parse::<Action>().unwrap(),
            Action::ScrollBottom
        );
        assert_eq!(
            "scroll.half_page_up".parse::<Action>().unwrap(),
            Action::ScrollHalfPageUp
        );
    }

    #[test]
    fn test_parse_tab_go() {
        assert_eq!("tab.go.1".parse::<Action>().unwrap(), Action::TabGo(1));
        assert_eq!("tab.go.9".parse::<Action>().unwrap(), Action::TabGo(9));
    }

    #[test]
    fn test_parse_tab_go_invalid() {
        assert!("tab.go.0".parse::<Action>().is_err());
        assert!("tab.go.10".parse::<Action>().is_err());
        assert!("tab.go.abc".parse::<Action>().is_err());
    }

    #[test]
    fn test_parse_unknown_action() {
        assert!("foo.bar".parse::<Action>().is_err());
    }

    #[test]
    fn test_display_roundtrip() {
        let actions = [
            Action::ScrollUp,
            Action::ScrollDown,
            Action::ScrollPageUp,
            Action::ScrollPageDown,
            Action::ScrollHalfPageUp,
            Action::ScrollHalfPageDown,
            Action::ScrollTop,
            Action::ScrollBottom,
            Action::TabNext,
            Action::TabPrev,
            Action::TabClose,
            Action::TabCloseAll,
            Action::TabNew,
            Action::TabUndoClose,
            Action::TabGo(1),
            Action::TabGo(9),
            Action::HistoryBack,
            Action::HistoryForward,
            Action::SearchOpen,
            Action::SearchNext,
            Action::SearchPrev,
            Action::SearchClose,
            Action::ZoomIn,
            Action::ZoomOut,
            Action::ZoomReset,
            Action::ClipboardCopyUrl,
            Action::ClipboardCopySelection,
            Action::WindowNew,
            Action::WindowClose,
            Action::WindowCloseAllChildren,
            Action::WindowCloseAll,
            Action::WindowToggleSidebar,
            Action::WindowToggleRightSidebar,
            Action::FocusLeftSidebar,
            Action::FocusRightSidebar,
            Action::FocusQuickAccess,
            Action::FocusContent,
            Action::FileOpen,
            Action::FileOpenDirectory,
            Action::FileRevealInFinder,
            Action::FileCopyPath,
            Action::AppAbout,
            Action::AppPreferences,
            Action::AppHomepage,
            Action::CursorDown,
            Action::CursorUp,
            Action::CursorEnter,
            Action::CursorCollapse,
            Action::DirectoryParent,
            Action::DirectoryBack,
            Action::DirectoryForward,
            Action::Cancel,
        ];

        for action in &actions {
            let s = action.to_string();
            let parsed: Action = s.parse().unwrap();
            assert_eq!(&parsed, action, "roundtrip failed for {s}");
        }
    }
}
