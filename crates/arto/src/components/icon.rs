use dioxus::prelude::*;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconName {
    Add,
    AlertCircle,
    AlertTriangle,
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
    Command,
    Copy,
    Download,
    ExternalLink,
    Eye,
    EyeOff,
    File,
    FileUpload,
    Folder,
    FolderOpen,
    Gear,
    InfoCircle,
    List,
    Moon,
    Menu2,
    Photo,
    Pin,
    PinFilled,
    PinnedOff,
    Refresh,
    Search,
    SelectAll,
    Server,
    Sidebar,
    Star,
    StarFilled,
    Sun,
    SunMoon,
    Trash,
    ViewportNarrow,
    ViewportWide,
}

impl fmt::Display for IconName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            IconName::Add => "plus",
            IconName::AlertCircle => "alert-circle",
            IconName::AlertTriangle => "alert-triangle",
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
            IconName::Command => "command",
            IconName::Copy => "copy",
            IconName::Download => "download",
            IconName::ExternalLink => "external-link",
            IconName::Eye => "eye",
            IconName::EyeOff => "eye-off",
            IconName::File => "file",
            IconName::FileUpload => "file-upload",
            IconName::Folder => "folder",
            IconName::FolderOpen => "folder-open",
            IconName::Gear => "settings",
            IconName::InfoCircle => "info-circle",
            IconName::List => "list",
            IconName::Moon => "moon",
            IconName::Menu2 => "menu-2",
            IconName::Photo => "photo",
            IconName::Pin => "pin",
            IconName::PinFilled => "pin-filled",
            IconName::PinnedOff => "pinned-off",
            IconName::Refresh => "refresh",
            IconName::Search => "search",
            IconName::SelectAll => "select-all",
            IconName::Server => "server",
            IconName::Sidebar => "layout-sidebar",
            IconName::Star => "star",
            IconName::StarFilled => "star-filled",
            IconName::Sun => "sun",
            IconName::SunMoon => "sun-moon",
            IconName::Trash => "trash",
            IconName::ViewportNarrow => "viewport-narrow",
            IconName::ViewportWide => "viewport-wide",
        };
        write!(f, "{}", name)
    }
}

#[component]
pub fn Icon(
    name: IconName,
    #[props(default = 20)] size: u32,
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
