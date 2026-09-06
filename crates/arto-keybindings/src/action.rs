use std::fmt;
use std::str::FromStr;

/// All actions that can be triggered by keyboard shortcuts.
///
/// Each variant maps to a dot-separated string (e.g., `ScrollDown` ↔ `"scroll.down"`).
/// This enum covers existing menu functionality (MenuId 1:1 mapping) plus keyboard-only actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // Scroll (8) — keyboard-only
    ScrollDown,
    ScrollUp,
    ScrollPageDown,
    ScrollPageUp,
    ScrollHalfPageDown,
    ScrollHalfPageUp,
    ScrollTop,
    ScrollBottom,

    // Tab — MenuId: NewTab, CloseTab, CloseAllTabs + keyboard navigation
    TabNew,
    TabClose,
    TabCloseAll,
    TabCloseOthers,
    TabTogglePin,
    TabOpenInNewWindow,
    TabNext,
    TabPrev,

    // History (2) — MenuId: GoBack, GoForward
    HistoryBack,
    HistoryForward,

    // Search (3) — MenuId: Find, FindNext, FindPrevious
    SearchOpen,
    SearchNext,
    SearchPrev,
    SearchClear,
    SearchPinCurrent,

    // Zoom (3) — MenuId: ZoomIn, ZoomOut, ActualSize
    ZoomIn,
    ZoomOut,
    ZoomReset,

    // Clipboard — path variants (3)
    CopyFilePath,
    CopyFilePathWithLine,
    CopyFilePathWithRange,

    // Clipboard — content copy (9)
    CopyAsMarkdown,
    CopyCode,
    CopyCodeAsMarkdown,
    CopyTableAsTsv,
    CopyTableAsCsv,
    CopyTableAsMarkdown,
    CopyImage,
    CopyImageWithBackground,
    CopyImagePath,
    CopyImageAsMarkdown,
    CopyLinkPath,

    // Window (4)
    WindowNew,
    WindowClose,
    WindowCloseAllChildWindows,
    WindowCloseAllWindows,
    WindowToggleSidebar,
    WindowToggleRightSidebar,

    // Reload (1)
    WindowReload,

    // Focus (4) — keyboard-only
    FocusLeftSidebar,
    FocusRightSidebar,
    FocusQuickAccess,
    FocusContent,

    // File (4) — MenuId: Open, OpenDirectory, Preferences, RevealInFinder
    FileOpen,
    FileOpenDirectory,
    FileSetParentAsRoot,
    FileToggleBookmark,
    FileOpenLink,
    FileOpenLinkInNewTab,
    FileSaveImageAs,
    FilePreferences,
    FileRevealInFinder,
    FilePrint,

    // App (4) — MenuId: About, GoToHomepage + keyboard-only help overlay
    AppAbout,
    AppQuit,
    AppGoToHomepage,
    HelpShowKeyboardShortcuts,

    // Sidebar (1)
    SidebarToggleShowAllFiles,

    // Right sidebar (2)
    RightSidebarShowContents,
    RightSidebarShowSearch,

    // Theme (3)
    ThemeSetLight,
    ThemeSetDark,
    ThemeSetAuto,

    // Cursor — sidebar/panel navigation (5) — keyboard-only
    CursorDown,
    CursorUp,
    CursorEnter,
    CursorOpen,
    CursorCollapse,

    // Content cursor — block element navigation (4) — keyboard-only
    ContentNext,
    ContentPrev,
    ContentNextHeading,
    ContentPrevHeading,
    ContentOpenViewer,

    // Directory — sidebar navigation (3) — keyboard-only
    DirectoryParent,
    DirectoryBack,
    DirectoryForward,

    // Cancel (1) — keyboard-only
    Cancel,
}

/// Action groups for the preferences UI dropdown (`<optgroup>`).
///
/// Each entry is `(group_label, &[Action])`. The order matches the enum definition.
pub const ACTION_GROUPS: &[(&str, &[Action])] = &[
    (
        "Scroll",
        &[
            Action::ScrollDown,
            Action::ScrollUp,
            Action::ScrollPageDown,
            Action::ScrollPageUp,
            Action::ScrollHalfPageDown,
            Action::ScrollHalfPageUp,
            Action::ScrollTop,
            Action::ScrollBottom,
        ],
    ),
    (
        "Tab",
        &[
            Action::TabNew,
            Action::TabClose,
            Action::TabCloseAll,
            Action::TabCloseOthers,
            Action::TabTogglePin,
            Action::TabOpenInNewWindow,
            Action::TabNext,
            Action::TabPrev,
        ],
    ),
    ("History", &[Action::HistoryBack, Action::HistoryForward]),
    (
        "Search",
        &[
            Action::SearchOpen,
            Action::SearchNext,
            Action::SearchPrev,
            Action::SearchClear,
            Action::SearchPinCurrent,
        ],
    ),
    (
        "Zoom",
        &[Action::ZoomIn, Action::ZoomOut, Action::ZoomReset],
    ),
    (
        "Clipboard",
        &[
            Action::CopyFilePath,
            Action::CopyFilePathWithLine,
            Action::CopyFilePathWithRange,
            Action::CopyAsMarkdown,
            Action::CopyCode,
            Action::CopyCodeAsMarkdown,
            Action::CopyTableAsTsv,
            Action::CopyTableAsCsv,
            Action::CopyTableAsMarkdown,
            Action::CopyImage,
            Action::CopyImageWithBackground,
            Action::CopyImagePath,
            Action::CopyImageAsMarkdown,
            Action::CopyLinkPath,
        ],
    ),
    (
        "Window",
        &[
            Action::WindowNew,
            Action::WindowClose,
            Action::WindowCloseAllChildWindows,
            Action::WindowCloseAllWindows,
            Action::WindowToggleSidebar,
            Action::WindowToggleRightSidebar,
            Action::WindowReload,
        ],
    ),
    (
        "Focus",
        &[
            Action::FocusLeftSidebar,
            Action::FocusRightSidebar,
            Action::FocusQuickAccess,
            Action::FocusContent,
        ],
    ),
    (
        "File",
        &[
            Action::FileOpen,
            Action::FileOpenDirectory,
            Action::FileSetParentAsRoot,
            Action::FileToggleBookmark,
            Action::FileOpenLink,
            Action::FileOpenLinkInNewTab,
            Action::FileSaveImageAs,
            Action::FilePreferences,
            Action::FileRevealInFinder,
            Action::FilePrint,
        ],
    ),
    (
        "App",
        &[
            Action::AppAbout,
            Action::AppQuit,
            Action::AppGoToHomepage,
            Action::HelpShowKeyboardShortcuts,
        ],
    ),
    ("Sidebar", &[Action::SidebarToggleShowAllFiles]),
    (
        "Right Sidebar",
        &[
            Action::RightSidebarShowContents,
            Action::RightSidebarShowSearch,
        ],
    ),
    (
        "Theme",
        &[
            Action::ThemeSetLight,
            Action::ThemeSetDark,
            Action::ThemeSetAuto,
        ],
    ),
    (
        "Cursor",
        &[
            Action::CursorDown,
            Action::CursorUp,
            Action::CursorEnter,
            Action::CursorOpen,
            Action::CursorCollapse,
        ],
    ),
    (
        "Content",
        &[
            Action::ContentNext,
            Action::ContentPrev,
            Action::ContentNextHeading,
            Action::ContentPrevHeading,
            Action::ContentOpenViewer,
        ],
    ),
    (
        "Directory",
        &[
            Action::DirectoryParent,
            Action::DirectoryBack,
            Action::DirectoryForward,
        ],
    ),
    ("Cancel", &[Action::Cancel]),
];

/// Actions that have a corresponding menu item and can therefore be bound as a
/// native menu shortcut.
///
/// Kept in sync with the desktop app's menu module, which maps menu items to
/// these actions and has a drift-guard test asserting every menu item's
/// action appears here.
pub const MENU_ACTIONS: &[Action] = &[
    Action::WindowNew,
    Action::TabNew,
    Action::FileOpen,
    Action::FileOpenDirectory,
    Action::CopyFilePath,
    Action::FileRevealInFinder,
    Action::TabClose,
    Action::TabCloseAll,
    Action::WindowClose,
    Action::FilePrint,
    Action::FilePreferences,
    Action::AppAbout,
    Action::SearchOpen,
    Action::SearchNext,
    Action::SearchPrev,
    Action::WindowToggleSidebar,
    Action::WindowToggleRightSidebar,
    Action::ZoomReset,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::HistoryBack,
    Action::HistoryForward,
    Action::WindowCloseAllChildWindows,
    Action::WindowCloseAllWindows,
    Action::AppGoToHomepage,
];

/// Whether an action string corresponds to a menu item (native-menu eligible).
pub fn is_menu_action(action: &str) -> bool {
    Action::from_str(action)
        .map(|a| MENU_ACTIONS.contains(&a))
        .unwrap_or(false)
}

/// Generate `Display` and `FromStr` impls from a single mapping table.
///
/// This ensures the string representation is always consistent between
/// serialization and deserialization — no risk of updating one but not the other.
macro_rules! action_strings {
    ($($variant:ident => $str:literal),* $(,)?) => {
        impl fmt::Display for Action {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let s = match self {
                    $(Self::$variant => $str,)*
                };
                f.write_str(s)
            }
        }

        impl FromStr for Action {
            type Err = ActionParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($str => Ok(Self::$variant),)*
                    _ => Err(ActionParseError(s.to_string())),
                }
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionParseError(pub(crate) String);

impl fmt::Display for ActionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown action: {:?}", self.0)
    }
}

impl std::error::Error for ActionParseError {}

action_strings! {
    ScrollDown => "scroll.down",
    ScrollUp => "scroll.up",
    ScrollPageDown => "scroll.page_down",
    ScrollPageUp => "scroll.page_up",
    ScrollHalfPageDown => "scroll.half_page_down",
    ScrollHalfPageUp => "scroll.half_page_up",
    ScrollTop => "scroll.top",
    ScrollBottom => "scroll.bottom",
    TabNew => "tab.new",
    TabClose => "tab.close",
    TabCloseAll => "tab.close_all",
    TabCloseOthers => "tab.close_others",
    TabTogglePin => "tab.toggle_pin",
    TabOpenInNewWindow => "tab.open_in_new_window",
    TabNext => "tab.next",
    TabPrev => "tab.prev",
    HistoryBack => "history.back",
    HistoryForward => "history.forward",
    SearchOpen => "search.open",
    SearchNext => "search.next",
    SearchPrev => "search.prev",
    SearchClear => "search.clear",
    SearchPinCurrent => "search.pin_current",
    ZoomIn => "zoom.in",
    ZoomOut => "zoom.out",
    ZoomReset => "zoom.reset",
    CopyFilePath => "clipboard.copy_file_path",
    CopyFilePathWithLine => "clipboard.copy_file_path_with_line",
    CopyFilePathWithRange => "clipboard.copy_file_path_with_range",
    CopyAsMarkdown => "clipboard.copy_as_markdown",
    CopyCode => "clipboard.copy_code",
    CopyCodeAsMarkdown => "clipboard.copy_code_as_markdown",
    CopyTableAsTsv => "clipboard.copy_table_as_tsv",
    CopyTableAsCsv => "clipboard.copy_table_as_csv",
    CopyTableAsMarkdown => "clipboard.copy_table_as_markdown",
    CopyImage => "clipboard.copy_image",
    CopyImageWithBackground => "clipboard.copy_image_with_background",
    CopyImagePath => "clipboard.copy_image_path",
    CopyImageAsMarkdown => "clipboard.copy_image_as_markdown",
    CopyLinkPath => "clipboard.copy_link_path",
    WindowNew => "window.new",
    WindowClose => "window.close",
    WindowCloseAllChildWindows => "window.close_all_child_windows",
    WindowCloseAllWindows => "window.close_all_windows",
    WindowToggleSidebar => "window.toggle_sidebar",
    WindowToggleRightSidebar => "window.toggle_right_sidebar",
    WindowReload => "window.reload",
    FocusLeftSidebar => "focus.left_sidebar",
    FocusRightSidebar => "focus.right_sidebar",
    FocusQuickAccess => "focus.quick_access",
    FocusContent => "focus.content",
    FileOpen => "file.open",
    FileOpenDirectory => "file.open_directory",
    FileSetParentAsRoot => "file.set_parent_as_root",
    FileToggleBookmark => "file.toggle_bookmark",
    FileOpenLink => "file.open_link",
    FileOpenLinkInNewTab => "file.open_link_in_new_tab",
    FileSaveImageAs => "file.save_image_as",
    FilePreferences => "file.preferences",
    FileRevealInFinder => "file.reveal_in_finder",
    FilePrint => "file.print",
    AppAbout => "app.about",
    AppQuit => "app.quit",
    AppGoToHomepage => "app.go_to_homepage",
    HelpShowKeyboardShortcuts => "help.show_keyboard_shortcuts",
    SidebarToggleShowAllFiles => "sidebar.toggle_show_all_files",
    RightSidebarShowContents => "right_sidebar.show_contents",
    RightSidebarShowSearch => "right_sidebar.show_search",
    ThemeSetLight => "theme.set_light",
    ThemeSetDark => "theme.set_dark",
    ThemeSetAuto => "theme.set_auto",
    CursorDown => "cursor.down",
    CursorUp => "cursor.up",
    CursorEnter => "cursor.enter",
    CursorOpen => "cursor.open",
    CursorCollapse => "cursor.collapse",
    ContentNext => "content.next",
    ContentPrev => "content.prev",
    ContentNextHeading => "content.next_heading",
    ContentPrevHeading => "content.prev_heading",
    ContentOpenViewer => "content.open_viewer",
    DirectoryParent => "directory.parent",
    DirectoryBack => "directory.back",
    DirectoryForward => "directory.forward",
    Cancel => "cancel",
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect all actions from ACTION_GROUPS into a flat Vec.
    fn all_actions() -> Vec<Action> {
        ACTION_GROUPS
            .iter()
            .flat_map(|(_, actions)| actions.iter().copied())
            .collect()
    }

    #[test]
    fn all_actions_count() {
        assert_eq!(all_actions().len(), 85);
    }

    #[test]
    fn display_roundtrip() {
        for action in all_actions() {
            let s = action.to_string();
            let parsed: Action = s
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse {s:?} back to Action: {e}"));
            assert_eq!(action, parsed, "roundtrip failed for {s:?}");
        }
    }

    #[test]
    fn display_format() {
        assert_eq!(Action::ScrollDown.to_string(), "scroll.down");
        assert_eq!(Action::TabNew.to_string(), "tab.new");
        assert_eq!(Action::CopyFilePath.to_string(), "clipboard.copy_file_path");
        assert_eq!(Action::Cancel.to_string(), "cancel");
        assert_eq!(Action::FocusLeftSidebar.to_string(), "focus.left_sidebar");
    }

    #[test]
    fn parse_valid() {
        assert_eq!("scroll.down".parse::<Action>().unwrap(), Action::ScrollDown);
        assert_eq!("cancel".parse::<Action>().unwrap(), Action::Cancel);
        assert_eq!(
            "clipboard.copy_code".parse::<Action>().unwrap(),
            Action::CopyCode
        );
        assert_eq!(
            "file.toggle_bookmark".parse::<Action>().unwrap(),
            Action::FileToggleBookmark
        );
        assert_eq!(
            "content.open_viewer".parse::<Action>().unwrap(),
            Action::ContentOpenViewer
        );
    }

    #[test]
    fn action_groups_include_new_actions() {
        let actions = all_actions();
        assert!(actions.contains(&Action::FileToggleBookmark));
        assert!(actions.contains(&Action::ContentOpenViewer));
    }

    #[test]
    fn parse_invalid() {
        assert!("unknown.action".parse::<Action>().is_err());
        assert!("".parse::<Action>().is_err());
        assert!("scroll".parse::<Action>().is_err());
    }

    #[test]
    fn no_duplicate_display_strings() {
        let mut seen = std::collections::HashSet::new();
        for action in all_actions() {
            let s = action.to_string();
            assert!(seen.insert(s.clone()), "duplicate display string: {s:?}");
        }
    }
}
