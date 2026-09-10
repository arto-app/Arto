use dioxus::prelude::*;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconName {
    Add,
    AlertCircle,
    AlertTriangle,
    AppWindow,
    ArrowsDiagonal,
    ArrowsMove,
    BrandGithub,
    Bug,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    Click,
    Close,
    Code,
    Command,
    Copy,
    CopyPlus,
    Download,
    Edit,
    ExternalLink,
    Eye,
    EyeOff,
    File,
    FileSearch,
    FileUpload,
    Bookmark,
    Folder,
    FolderOpen,
    FolderPlus,
    FolderUp,
    Gear,
    HelpCircle,
    History,
    InfoCircle,
    LetterT,
    Link,
    Markdown,
    Moon,
    Menu2,
    Photo,
    Pin,
    Power,
    Printer,
    Refresh,
    Search,
    SelectAll,
    Sidebar,
    Star,
    StarFilled,
    Sun,
    SunMoon,
    Table,
    Trash,
    ViewportNarrow,
    ViewportWide,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl fmt::Display for IconName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            IconName::Add => "plus",
            IconName::AlertCircle => "alert-circle",
            IconName::AlertTriangle => "alert-triangle",
            IconName::AppWindow => "app-window",
            IconName::ArrowsDiagonal => "arrows-diagonal",
            IconName::ArrowsMove => "arrows-move",
            IconName::BrandGithub => "brand-github",
            IconName::Bug => "bug",
            IconName::Check => "check",
            IconName::ChevronDown => "chevron-down",
            IconName::ChevronLeft => "chevron-left",
            IconName::ChevronRight => "chevron-right",
            IconName::ChevronUp => "chevron-up",
            IconName::Click => "click",
            IconName::Close => "x",
            IconName::Code => "code",
            IconName::Command => "command",
            IconName::Copy => "copy",
            IconName::CopyPlus => "copy-plus",
            IconName::Download => "download",
            IconName::Edit => "edit",
            IconName::ExternalLink => "external-link",
            IconName::Eye => "eye",
            IconName::EyeOff => "eye-off",
            IconName::File => "file",
            IconName::FileSearch => "file-search",
            IconName::FileUpload => "file-upload",
            IconName::Bookmark => "bookmark",
            IconName::Folder => "folder",
            IconName::FolderOpen => "folder-open",
            IconName::FolderPlus => "folder-plus",
            IconName::FolderUp => "folder-up",
            IconName::Gear => "settings",
            IconName::HelpCircle => "help-circle",
            IconName::History => "history",
            IconName::InfoCircle => "info-circle",
            IconName::LetterT => "letter-t",
            IconName::Link => "link",
            IconName::Markdown => "markdown",
            IconName::Moon => "moon",
            IconName::Menu2 => "menu-2",
            IconName::Photo => "photo",
            IconName::Pin => "pin",
            IconName::Power => "power",
            IconName::Printer => "printer",
            IconName::Refresh => "refresh",
            IconName::Search => "search",
            IconName::SelectAll => "select-all",
            IconName::Sidebar => "layout-sidebar",
            IconName::Star => "star",
            IconName::StarFilled => "star-filled",
            IconName::Sun => "sun",
            IconName::SunMoon => "sun-moon",
            IconName::Table => "table",
            IconName::Trash => "trash",
            IconName::ViewportNarrow => "viewport-narrow",
            IconName::ViewportWide => "viewport-wide",
            IconName::ZoomIn => "zoom-in",
            IconName::ZoomOut => "zoom-out",
            IconName::ZoomReset => "zoom-reset",
        };
        write!(f, "{}", name)
    }
}

/// One glyph from the sprite.
///
/// The default is the chrome's size: small enough to read past, in a target
/// that is not (`--hit-size`). What is drawn and what can be hit are separate
/// numbers, so the ink can shrink without the control getting harder to press.
#[component]
pub fn Icon(
    name: IconName,
    #[props(default = 14)] size: u32,
    #[props(default = "")] class: &'static str,
) -> Element {
    let icon_id = format!("tabler-{}", name);

    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            "aria-hidden": "true",
            // A bare fragment, because the sprite is in this very document:
            // `crate::window::index` writes it into the body. A `<use>` that
            // names another origin is refused, and unlike a stylesheet or a
            // script no header makes it allowed.
            r#use {
                href: "#{icon_id}"
            }
        }
    }
}
