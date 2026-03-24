use dioxus::prelude::*;
use std::fmt;

const TABLER_SPRITE: Asset = asset!("/assets/dist/icons/tabler-sprite.svg");

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
        };
        write!(f, "{}", name)
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct IconProps {
    pub name: IconName,
    #[props(default = 20)]
    pub size: u32,
    #[props(default = "")]
    pub class: &'static str,
}

#[component]
pub fn Icon(props: IconProps) -> Element {
    #[cfg(windows)]
    let sprite_url = crate::assets::windows_safe_asset_url(&TABLER_SPRITE);
    #[cfg(not(windows))]
    let sprite_url = TABLER_SPRITE.to_string();

    let icon_id = format!("tabler-{}", props.name);

    rsx! {
        svg {
            class: "icon {props.class}",
            width: "{props.size}",
            height: "{props.size}",
            "aria-hidden": "true",
            r#use {
                href: "{sprite_url}#{icon_id}"
            }
        }
    }
}
