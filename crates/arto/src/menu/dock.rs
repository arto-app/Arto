//! The menu the Dock shows on a right click of the app icon.
//!
//! Tao's application delegate does not answer `applicationDockMenu:`, so
//! [`install`] adds that method to the delegate's class at runtime. AppKit
//! asks for the menu every time it is about to show it, which is what lets
//! the recent documents be read fresh rather than kept in step.
//!
//! The items are plain muda items, so their clicks arrive through the same
//! `use_muda_event_handler` callbacks as the menu bar's and are handled by
//! [`handle_menu_event`] from the global handler. They carry ids of their
//! own instead of reusing the menu bar's, because the menu bar's Open items
//! act on the focused window, and an app chosen from the Dock may have none.

use std::cell::RefCell;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use dioxus_desktop::muda::{ContextMenu, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use dioxus_desktop::window;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{msg_send, sel, MainThreadMarker};
use objc2_app_kit::NSApplication;
use percent_encoding::{percent_decode_str, percent_encode, NON_ALPHANUMERIC};
use rust_i18n::t;

use crate::visits::{documents, Visit, VISITS};
use crate::window::{self as app_window, CreateMainWindowConfigParams};

/// How many documents the Open Recent submenu lists.
const RECENT_LIMIT: usize = 10;

const RECENT_PREFIX: &str = "dock.recent:";

#[derive(Debug, Clone, PartialEq, Eq)]
enum DockItem {
    NewWindow,
    OpenFile,
    OpenDirectory,
    Recent(PathBuf),
}

impl DockItem {
    fn id(&self) -> String {
        match self {
            Self::NewWindow => "dock.new_window".to_string(),
            Self::OpenFile => "dock.open".to_string(),
            Self::OpenDirectory => "dock.open_directory".to_string(),
            // The path's bytes, percent-encoded: an id is a `String`, and a
            // file name need not be UTF-8, so its display form would name a
            // different file once read back.
            Self::Recent(path) => format!(
                "{RECENT_PREFIX}{}",
                percent_encode(path.as_os_str().as_bytes(), NON_ALPHANUMERIC)
            ),
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "dock.new_window" => Some(Self::NewWindow),
            "dock.open" => Some(Self::OpenFile),
            "dock.open_directory" => Some(Self::OpenDirectory),
            _ => id
                .strip_prefix(RECENT_PREFIX)
                .filter(|encoded| !encoded.is_empty())
                .map(|encoded| {
                    let bytes: Vec<u8> = percent_decode_str(encoded).collect();
                    Self::Recent(PathBuf::from(OsString::from_vec(bytes)))
                }),
        }
    }

    fn menu_item(&self, label: &str) -> MenuItem {
        MenuItem::with_id(self.id(), label, true, None)
    }
}

/// The most recently read documents, newest first.
///
/// Whether each still exists is not checked: the menu is built synchronously
/// on the main thread, and a stat of a path on an unresponsive network share
/// would freeze the app. A document that has gone is dropped by
/// `ipc::validate_path` when chosen; the other history lists do not stat
/// their rows either.
fn recent_documents(visits: &[Visit], limit: usize) -> Vec<PathBuf> {
    documents(visits)
        .map(|visit| visit.path.clone())
        .take(limit)
        .collect()
}

/// A label per document: its file name, followed by as many of its parent
/// directories as it takes to tell it apart from the other documents in the
/// list.
fn recent_labels(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let parents = path.parent().map_or(0, |parent| parent.iter().count());
            let mut depth = 0;
            loop {
                let label = label_at(path, depth);
                let clashes = paths
                    .iter()
                    .enumerate()
                    .any(|(j, other)| j != i && label_at(other, depth) == label);
                if !clashes || depth >= parents {
                    return label;
                }
                depth += 1;
            }
        })
        .collect()
}

/// The file name, followed by the last `depth` components of its parent.
fn label_at(path: &Path, depth: usize) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let parents: Vec<_> = path
        .parent()
        .map(|parent| parent.iter().map(|c| c.to_string_lossy()).collect())
        .unwrap_or_default();
    let tail = &parents[parents.len().saturating_sub(depth)..];
    if tail.is_empty() {
        name
    } else {
        format!("{name} — {}", tail.join("/"))
    }
}

thread_local! {
    /// The menu last handed to AppKit, kept alive for as long as the Dock may
    /// still be showing it. AppKit does not take ownership of the returned
    /// menu.
    static DOCK_MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

fn build_dock_menu() -> Menu {
    let menu = Menu::new();
    menu.append_items(&[
        &DockItem::NewWindow.menu_item(&t!("menu.dock.new_window")),
        &PredefinedMenuItem::separator(),
        &DockItem::OpenFile.menu_item(&t!("menu.dock.open")),
        &DockItem::OpenDirectory.menu_item(&t!("menu.dock.open_directory")),
    ])
    .unwrap();

    let recent = recent_documents(&VISITS.read().items, RECENT_LIMIT);
    if !recent.is_empty() {
        let submenu = Submenu::new(t!("menu.dock.open_recent"), true);
        for (path, label) in recent.iter().zip(recent_labels(&recent)) {
            submenu
                .append(&DockItem::Recent(path.clone()).menu_item(&label))
                .unwrap();
        }
        menu.append(&submenu).unwrap();
    }

    menu
}

/// `-[NSApplicationDelegate applicationDockMenu:]`
unsafe extern "C-unwind" fn application_dock_menu(
    _this: *mut AnyObject,
    _cmd: Sel,
    _sender: *mut AnyObject,
) -> *mut AnyObject {
    DOCK_MENU.with(|slot| {
        let menu = build_dock_menu();
        let ns_menu = menu.ns_menu() as *mut AnyObject;
        *slot.borrow_mut() = Some(menu);
        ns_menu
    })
}

/// Teach the application delegate to answer the Dock's request for a menu.
///
/// Must run on the main thread once the event loop has installed its
/// delegate, which is the case by the time any window's component renders.
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("Dock menu must be installed on the main thread");
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let delegate: Option<Retained<AnyObject>> = unsafe { msg_send![&app, delegate] };
    let Some(delegate) = delegate else {
        tracing::warn!("No application delegate to install the Dock menu on");
        return;
    };

    let class: &AnyClass = delegate.class();
    let imp: Imp = unsafe {
        std::mem::transmute::<
            unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> *mut AnyObject,
            Imp,
        >(application_dock_menu)
    };
    // `class_addMethod` refuses a selector the class already implements, so a
    // second call (another window's component) is a no-op.
    let added = unsafe {
        objc2::ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            sel!(applicationDockMenu:),
            imp,
            c"@@:@".as_ptr(),
        )
    };
    tracing::debug!(added = added.as_bool(), "Installed the Dock menu");
}

/// Bring the app forward, so what a Dock item opens (a window, a file
/// dialog) is not left behind the app that was frontmost.
fn activate_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        // `activate` replaces this from macOS 14 on, but this still has to run
        // on the releases before it.
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}

fn open_path(path: impl AsRef<Path>) {
    if let Some(event) = crate::ipc::validate_path(path) {
        crate::ipc::push_event(event);
        crate::ipc::process_main_thread_tasks();
    }
}

/// Handle a click on a Dock menu item.
///
/// Returns `true` if the event belonged to the Dock menu.
pub fn handle_menu_event(event: &MenuEvent) -> bool {
    let Some(item) = DockItem::from_id(event.id().0.as_ref()) else {
        return false;
    };
    tracing::debug!(?item, "Dock menu event");
    activate_app();

    match item {
        DockItem::NewWindow => {
            app_window::create_main_window_sync(
                &window(),
                crate::state::Document::default(),
                CreateMainWindowConfigParams::default(),
            );
        }
        DockItem::OpenFile => {
            if let Some(file) = crate::keybindings::dispatcher::pick_markdown_file() {
                open_path(file);
            }
        }
        DockItem::OpenDirectory => {
            if let Some(dir) = crate::keybindings::dispatcher::pick_directory() {
                open_path(dir);
            }
        }
        DockItem::Recent(path) => open_path(path),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    fn visit(path: &str, day: u32) -> Visit {
        Visit::new(
            path,
            Local.with_ymd_and_hms(2026, 9, day, 12, 0, 0).unwrap(),
        )
    }

    #[test]
    fn dock_item_ids_roundtrip() {
        for item in [
            DockItem::NewWindow,
            DockItem::OpenFile,
            DockItem::OpenDirectory,
            DockItem::Recent(PathBuf::from("/notes/a b/README.md")),
        ] {
            assert_eq!(DockItem::from_id(&item.id()), Some(item));
        }
    }

    #[test]
    fn recent_ids_keep_paths_that_are_not_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = PathBuf::from(OsStr::from_bytes(b"/notes/caf\xe9.md"));
        let item = DockItem::Recent(path);
        assert_eq!(DockItem::from_id(&item.id()), Some(item));
    }

    #[test]
    fn dock_item_ids_do_not_claim_menu_bar_ids() {
        assert_eq!(DockItem::from_id("file.new_window"), None);
        assert_eq!(DockItem::from_id("file.open"), None);
        assert_eq!(DockItem::from_id(RECENT_PREFIX), None);
    }

    #[test]
    fn recent_documents_are_distinct_and_capped() {
        let visits = vec![
            visit("/a.md", 3),
            visit("/b.md", 2),
            visit("/a.md", 2),
            visit("/c.md", 1),
        ];
        let recent = recent_documents(&visits, 2);
        assert_eq!(recent, vec![PathBuf::from("/a.md"), PathBuf::from("/b.md")]);
    }

    #[test]
    fn recent_labels_add_the_parent_only_to_shared_names() {
        let paths = vec![
            PathBuf::from("/work/arto/README.md"),
            PathBuf::from("/work/notes/today.md"),
            PathBuf::from("/work/site/README.md"),
        ];
        assert_eq!(
            recent_labels(&paths),
            vec!["README.md — arto", "today.md", "README.md — site"]
        );
    }

    #[test]
    fn recent_labels_climb_until_shared_parents_differ() {
        let paths = vec![
            PathBuf::from("/work/project-a/docs/README.md"),
            PathBuf::from("/work/project-b/docs/README.md"),
            PathBuf::from("/work/project-b/guide/README.md"),
        ];
        assert_eq!(
            recent_labels(&paths),
            vec![
                "README.md — project-a/docs",
                "README.md — project-b/docs",
                "README.md — guide",
            ]
        );
    }
}
