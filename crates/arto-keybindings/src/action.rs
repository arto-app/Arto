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

    // Window (7)
    WindowNew,
    /// A copy of this window: its temporary roots, its document and the place
    /// in it the reader had reached.
    WindowDuplicate,
    /// Put the document down; the welcome page takes its place.
    WindowNewDocument,
    WindowClose,
    WindowCloseAllChildWindows,
    WindowCloseAllWindows,
    WindowToggleSidebar,

    // Reload (1)
    WindowReload,

    // Focus — keyboard-only. One per face of the panel, so that a reader can
    // reach the list they want rather than the one the panel happens to be on.
    FocusPlaces,
    FocusStarred,
    FocusRecent,
    FocusContent,

    // File (4) — MenuId: Open, OpenDirectory, Preferences, RevealInFinder
    FileOpen,
    FileOpenDirectory,
    FileSetParentAsRoot,
    FileToggleBookmark,
    FileOpenLink,
    FileOpenLinkInNewWindow,
    FileSaveImageAs,
    FilePreferences,
    FileRevealInFinder,
    FilePrint,

    // App (4) — MenuId: About, GoToHomepage + keyboard-only help overlay
    AppAbout,
    AppQuit,
    AppGoToHomepage,
    HelpShowKeyboardShortcuts,

    // Palette — the history, two keystrokes away, and moving through it
    PaletteOpen,
    PaletteNext,
    PalettePrev,
    PaletteConfirm,
    PaletteClose,

    // Contents — the gutter's list, and the keyboard's way through it
    ContentsToggle,
    ContentsNext,
    ContentsPrev,
    ContentsConfirm,
    ContentsClose,

    // Sidebar — the panel and which of its faces is showing
    SidebarToggleShowAllFiles,
    SidebarFacePlaces,
    SidebarFaceRecent,
    SidebarFaceStarred,
    SidebarFaceNext,
    SidebarFacePrev,

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

    // Directory — sidebar navigation (1) — keyboard-only
    DirectoryParent,

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
            Action::WindowDuplicate,
            Action::WindowNewDocument,
            Action::WindowClose,
            Action::WindowCloseAllChildWindows,
            Action::WindowCloseAllWindows,
            Action::WindowToggleSidebar,
            Action::WindowReload,
        ],
    ),
    (
        "Focus",
        &[
            Action::FocusPlaces,
            Action::FocusStarred,
            Action::FocusRecent,
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
            Action::FileOpenLinkInNewWindow,
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
    (
        "Palette",
        &[
            Action::PaletteOpen,
            Action::PaletteNext,
            Action::PalettePrev,
            Action::PaletteConfirm,
            Action::PaletteClose,
        ],
    ),
    (
        "Contents",
        &[
            Action::ContentsToggle,
            Action::ContentsNext,
            Action::ContentsPrev,
            Action::ContentsConfirm,
            Action::ContentsClose,
        ],
    ),
    (
        "Sidebar",
        &[
            Action::SidebarToggleShowAllFiles,
            Action::SidebarFacePlaces,
            Action::SidebarFaceRecent,
            Action::SidebarFaceStarred,
            Action::SidebarFaceNext,
            Action::SidebarFacePrev,
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
    ("Directory", &[Action::DirectoryParent]),
    ("Cancel", &[Action::Cancel]),
];

impl Action {
    /// The name this action answers to when it is typed rather than pressed.
    ///
    /// `None` means the action is a motion — a scroll, a cursor step, a focus
    /// move — or is only meaningful with something already under the cursor.
    /// Naming those would fill a search with rows that do nothing when the
    /// list they act on is not the thing being looked at, so they stay on the
    /// keyboard where they belong.
    ///
    /// The labels match the menu wherever the menu has the same item, so a
    /// reader who learned a name in one place finds it in the other.
    pub fn command_label(&self) -> Option<&'static str> {
        let label = match self {
            // History
            Self::HistoryBack => "Back",
            Self::HistoryForward => "Forward",

            // Search
            Self::SearchOpen => "Find in Page",

            // Zoom
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomReset => "Actual Size",

            // Clipboard — the two that act on the document rather than on
            // whatever the content cursor happens to be standing on.
            Self::CopyFilePath => "Copy File Path",
            Self::CopyAsMarkdown => "Copy Document as Markdown",

            // Window
            Self::WindowNew => "New Window",
            Self::WindowDuplicate => "Duplicate Window",
            Self::WindowNewDocument => "New Document",
            Self::WindowClose => "Close Window",
            Self::WindowCloseAllChildWindows => "Close All Child Windows",
            Self::WindowCloseAllWindows => "Close All Windows",
            Self::WindowToggleSidebar => "Toggle Sidebar",
            Self::WindowReload => "Reload Document",

            // File
            Self::FileOpen => "Open File\u{2026}",
            Self::FileOpenDirectory => "Open Directory\u{2026}",
            Self::FileSetParentAsRoot => "Make This the Window's Folder",
            Self::FileToggleBookmark => "Toggle Star",
            Self::FilePreferences => "Preferences\u{2026}",
            Self::FileRevealInFinder => "Reveal in Finder",
            Self::FilePrint => "Print\u{2026}",

            // App
            Self::AppAbout => "About Arto",
            Self::AppQuit => "Quit Arto",
            Self::AppGoToHomepage => "Go to Homepage",

            // Contents
            Self::ContentsToggle => "Contents",

            // Sidebar
            Self::SidebarToggleShowAllFiles => "Show All Files",
            Self::SidebarFacePlaces => "Show Places",
            Self::SidebarFaceRecent => "Show Recent",
            Self::SidebarFaceStarred => "Show Starred",
            Self::SidebarFaceNext => "Next Face",
            Self::SidebarFacePrev => "Previous Face",

            // Theme
            Self::ThemeSetLight => "Light Theme",
            Self::ThemeSetDark => "Dark Theme",
            Self::ThemeSetAuto => "Match System Theme",

            _ => return None,
        };
        Some(label)
    }
}

/// Every action that answers to a name, in the order the palette lists them.
///
/// The order is the enum's own, which groups related commands together; the
/// palette does not re-sort, so a query that leaves several commands standing
/// shows them in a stable order rather than one that moves as the list is
/// narrowed.
pub const COMMAND_ACTIONS: &[Action] = &[
    Action::HistoryBack,
    Action::HistoryForward,
    Action::SearchOpen,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::ZoomReset,
    Action::CopyFilePath,
    Action::CopyAsMarkdown,
    Action::WindowNew,
    Action::WindowDuplicate,
    Action::WindowNewDocument,
    Action::WindowClose,
    Action::WindowCloseAllChildWindows,
    Action::WindowCloseAllWindows,
    Action::WindowToggleSidebar,
    Action::WindowReload,
    Action::FileOpen,
    Action::FileOpenDirectory,
    Action::FileSetParentAsRoot,
    Action::FileToggleBookmark,
    Action::FilePreferences,
    Action::FileRevealInFinder,
    Action::FilePrint,
    Action::AppAbout,
    Action::AppQuit,
    Action::AppGoToHomepage,
    Action::ContentsToggle,
    Action::SidebarToggleShowAllFiles,
    Action::SidebarFacePlaces,
    Action::SidebarFaceRecent,
    Action::SidebarFaceStarred,
    Action::SidebarFaceNext,
    Action::SidebarFacePrev,
    Action::ThemeSetLight,
    Action::ThemeSetDark,
    Action::ThemeSetAuto,
];

/// Actions that have a corresponding menu item and can therefore be bound as a
/// native menu shortcut.
///
/// Kept in sync with the desktop app's menu module, which maps menu items to
/// these actions and has a drift-guard test asserting every menu item's
/// action appears here.
pub const MENU_ACTIONS: &[Action] = &[
    Action::WindowNew,
    Action::WindowDuplicate,
    Action::WindowNewDocument,
    Action::FileOpen,
    Action::FileOpenDirectory,
    Action::CopyFilePath,
    Action::FileRevealInFinder,
    Action::WindowClose,
    Action::FilePrint,
    Action::FilePreferences,
    Action::AppAbout,
    Action::SearchOpen,
    Action::SearchNext,
    Action::SearchPrev,
    Action::WindowToggleSidebar,
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
    WindowDuplicate => "window.duplicate",
    WindowNewDocument => "window.new_document",
    WindowClose => "window.close",
    WindowCloseAllChildWindows => "window.close_all_child_windows",
    WindowCloseAllWindows => "window.close_all_windows",
    WindowToggleSidebar => "window.toggle_sidebar",
    WindowReload => "window.reload",
    FocusPlaces => "focus.places",
    FocusStarred => "focus.starred",
    FocusRecent => "focus.recent",
    FocusContent => "focus.content",
    FileOpen => "file.open",
    FileOpenDirectory => "file.open_directory",
    FileSetParentAsRoot => "file.set_parent_as_root",
    FileToggleBookmark => "file.toggle_bookmark",
    FileOpenLink => "file.open_link",
    FileOpenLinkInNewWindow => "file.open_link_in_new_window",
    FileSaveImageAs => "file.save_image_as",
    FilePreferences => "file.preferences",
    FileRevealInFinder => "file.reveal_in_finder",
    FilePrint => "file.print",
    AppAbout => "app.about",
    AppQuit => "app.quit",
    AppGoToHomepage => "app.go_to_homepage",
    HelpShowKeyboardShortcuts => "help.show_keyboard_shortcuts",
    PaletteOpen => "palette.open",
    PaletteNext => "palette.next",
    PalettePrev => "palette.prev",
    PaletteConfirm => "palette.confirm",
    PaletteClose => "palette.close",
    ContentsToggle => "contents.toggle",
    ContentsNext => "contents.next",
    ContentsPrev => "contents.prev",
    ContentsConfirm => "contents.confirm",
    ContentsClose => "contents.close",
    SidebarToggleShowAllFiles => "sidebar.toggle_show_all_files",
    SidebarFacePlaces => "sidebar.face_places",
    SidebarFaceRecent => "sidebar.face_recent",
    SidebarFaceStarred => "sidebar.face_starred",
    SidebarFaceNext => "sidebar.face_next",
    SidebarFacePrev => "sidebar.face_prev",
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
        assert_eq!(all_actions().len(), 89);
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
        assert_eq!(Action::WindowNew.to_string(), "window.new");
        assert_eq!(Action::CopyFilePath.to_string(), "clipboard.copy_file_path");
        assert_eq!(Action::Cancel.to_string(), "cancel");
        assert_eq!(Action::FocusPlaces.to_string(), "focus.places");
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
    fn every_named_command_is_listed() {
        for action in all_actions() {
            let listed = COMMAND_ACTIONS.contains(&action);
            assert_eq!(
                action.command_label().is_some(),
                listed,
                "{action} is named but not listed in COMMAND_ACTIONS, or the other way round"
            );
        }
    }

    #[test]
    fn motions_have_no_name() {
        for action in [
            Action::ScrollDown,
            Action::CursorDown,
            Action::ContentNext,
            Action::FocusContent,
            Action::PaletteOpen,
            Action::Cancel,
        ] {
            assert_eq!(action.command_label(), None, "{action} should be unnamed");
        }
    }

    #[test]
    fn no_duplicate_command_labels() {
        let mut seen = std::collections::HashSet::new();
        for action in COMMAND_ACTIONS {
            let label = action.command_label().expect("listed command has a label");
            assert!(seen.insert(label), "duplicate command label: {label:?}");
        }
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
