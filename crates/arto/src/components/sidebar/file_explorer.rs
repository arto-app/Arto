use dioxus::desktop::window;
use dioxus::prelude::*;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;
use tokio::sync::oneshot;

use super::context_menu::{
    context_action_should_proceed, SidebarContextMenu, SidebarContextMenuData, SidebarItemKind,
};
use crate::components::bookmark_button::BookmarkButton;
use crate::components::icon::{Icon, IconName};
use crate::state::{AppState, FocusedPanel};
use crate::utils::{file::is_markdown_file, file_operations};
use crate::watcher::FILE_WATCHER;

/// A directory entry with pre-computed file type from `readdir()`.
///
/// On macOS/APFS, `DirEntry::file_type()` reads the `d_type` field from the
/// `readdir()` result without issuing a `stat()` syscall. This avoids triggering
/// macOS TCC permission dialogs for protected directories (e.g. ~/Music).
struct FileEntry {
    path: PathBuf,
    is_dir: bool,
}

// Sort entries: directories first, then files, both alphabetically
fn sort_entries(items: &mut [FileEntry]) {
    items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.path.file_name().cmp(&b.path.file_name()),
    });
}

// Read and sort directory entries using DirEntry::file_type() to avoid stat() calls
fn read_sorted_entries(path: &PathBuf) -> Vec<FileEntry> {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut items: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let file_type = match e.file_type() {
                        Ok(ft) => ft,
                        Err(err) => {
                            tracing::debug!(?err, path = ?e.path(), "Skipping inaccessible entry");
                            return None;
                        }
                    };
                    // For symlinks, follow with metadata() to resolve actual type
                    let is_dir = if file_type.is_symlink() {
                        match fs::metadata(e.path()) {
                            Ok(m) => m.is_dir(),
                            Err(err) => {
                                tracing::debug!(?err, path = ?e.path(), "Failed to resolve symlink");
                                false
                            }
                        }
                    } else {
                        file_type.is_dir()
                    };
                    Some(FileEntry {
                        path: e.path(),
                        is_dir,
                    })
                })
                .collect();
            sort_entries(&mut items);
            items
        }
        Err(err) => {
            tracing::error!("Failed to read directory {:?}: {}", path, err);
            vec![]
        }
    }
}

#[component]
pub fn FileExplorer() -> Element {
    let mut state = use_context::<AppState>();
    // Memos rather than plain reads: each root's watcher must restart only when
    // the set of roots changes, not on every other sidebar field update.
    let places = use_memo(move || state.sidebar.read().roots.places().to_vec());
    let temps = use_memo(move || state.sidebar.read().roots.temps().to_vec());

    // Refresh counter to force DirectoryTree re-render. Sourced from AppState
    // (not a local signal) so the hoisted context menu's "Reload" action can
    // trigger a refresh from outside this subtree.
    let refresh_counter = state.sidebar_refresh_counter;

    // Bookmarking a folder is what makes it a place, so the two lists are one
    // list seen twice; this is where the tree picks the change up.
    use_future(move || async move {
        let mut rx = crate::bookmarks::BOOKMARKS_CHANGED.subscribe();
        while rx.recv().await.is_ok() {
            state.sync_places();
        }
    });

    let has_roots = !places().is_empty() || !temps().is_empty();

    rsx! {
        div {
            class: "left-sidebar-explorer",
            key: "{refresh_counter}",

            if has_roots {
                if !places().is_empty() {
                    RootGroup {
                        label: "Places",
                        roots: places(),
                        closable: false,
                        refresh_counter,
                    }
                }
                div {
                    class: "left-sidebar-add-root",
                    // A folder chosen through a dialog is a deliberate act, so it
                    // becomes a place: bookmarking it and rooting the tree at it
                    // are the same thing.
                    onclick: move |_| {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            crate::bookmarks::toggle_bookmark(dir);
                        }
                    },
                    Icon { name: IconName::FolderPlus, size: 12 }
                    span { "Add folder…" }
                }
                if !temps().is_empty() {
                    RootGroup {
                        label: "This window",
                        roots: temps(),
                        closable: true,
                        refresh_counter,
                    }
                }
            } else {
                div {
                    class: "left-sidebar-explorer-empty",
                    "No directory open"
                }
            }
        }
    }
}

/// One labelled group of roots — the places, or this window's temporaries.
///
/// The label is a hairline with a word on it, the same device the history uses
/// for its date groups: two groups of the same surface meeting is the one place
/// left where a line is the only thing that can separate them.
#[component]
fn RootGroup(
    label: &'static str,
    roots: Vec<PathBuf>,
    closable: bool,
    refresh_counter: Signal<u32>,
) -> Element {
    rsx! {
        div {
            class: "left-sidebar-root-group-label",
            span { "{label}" }
        }
        for root in roots {
            RootSubtree {
                key: "{root.display()}",
                root: root.clone(),
                closable,
                refresh_counter,
            }
        }
    }
}

/// A root and everything shown under it.
#[component]
fn RootSubtree(root: PathBuf, closable: bool, refresh_counter: Signal<u32>) -> Element {
    let mut state = use_context::<AppState>();
    let watched = use_memo({
        let root = root.clone();
        move || Some(root.clone())
    });
    use_directory_watcher(watched.into(), refresh_counter);

    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| root.display().to_string());
    let is_expanded = state.sidebar.read().expanded_dirs.contains(&root);
    let subtree_root = root.clone();

    rsx! {
        div {
            class: "left-sidebar-root",
            title: "{root.display()}",

            span {
                class: "left-sidebar-tree-chevron",
                onclick: {
                    let root = root.clone();
                    move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        state.toggle_directory_expansion(&root);
                    }
                },
                Icon {
                    name: if is_expanded { IconName::ChevronDown } else { IconName::ChevronRight },
                    size: 12,
                }
            }
            span {
                class: "left-sidebar-root-name",
                onclick: {
                    let root = root.clone();
                    move |_| state.toggle_directory_expansion(&root)
                },
                Icon { name: IconName::Folder, size: 12 }
                span { "{name}" }
            }
            if closable {
                button {
                    class: "left-sidebar-root-close",
                    title: "Close this root",
                    onclick: {
                        let root = root.clone();
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            state.close_root(&root);
                        }
                    },
                    Icon { name: IconName::Close, size: 12 }
                }
            }
        }
        if is_expanded {
            DirectoryTree { path: subtree_root, refresh_counter }
        }
    }
}

#[component]
fn DirectoryTree(path: PathBuf, refresh_counter: Signal<u32>) -> Element {
    let entries = read_sorted_entries(&path);

    rsx! {
        div {
            class: "left-sidebar-tree",
            key: "{refresh_counter}",
            for entry in entries {
                FileTreeNode { path: entry.path, is_dir: entry.is_dir, depth: 0, refresh_counter }
            }
        }
    }
}

/// Renders the children of an expanded directory.
///
/// Separated from `FileTreeNode` so that Dioxus component memoization prevents
/// re-reading the filesystem when only unrelated state (tabs, sidebar toggles)
/// changes — `DirectoryChildren` only re-renders when `path` or
/// `refresh_counter` actually change.
///
/// **Invalidation triggers:**
/// - `path` changes (user navigates to a different directory)
/// - `refresh_counter` increments (file watcher detects filesystem changes)
#[component]
fn DirectoryChildren(
    path: ReadSignal<PathBuf>,
    depth: usize,
    refresh_counter: Signal<u32>,
) -> Element {
    // Watch expanded directories only (non-recursive) to avoid broad permission access.
    let watched = use_memo(move || Some(path()));
    use_directory_watcher(watched.into(), refresh_counter);

    // Subscribe to the signal so Dioxus re-runs this component when the
    // counter increments (file watcher detected filesystem changes).
    let _ = refresh_counter.read();
    let path = path();
    let children = read_sorted_entries(&path);
    rsx! {
        for child in children {
            FileTreeNode { path: child.path, is_dir: child.is_dir, depth: depth + 1, refresh_counter }
        }
    }
}

#[component]
fn FileTreeNode(
    path: PathBuf,
    is_dir: bool,
    depth: usize,
    refresh_counter: Signal<u32>,
) -> Element {
    let mut state = use_context::<AppState>();

    let is_expanded = state.sidebar.read().expanded_dirs.contains(&path);
    let show_all_files = state.sidebar.read().show_all_files;

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let is_markdown = !is_dir && is_markdown_file(&path);

    // Hide non-markdown files if show_all_files is disabled
    if !show_all_files && !is_dir && !is_markdown {
        return rsx! {};
    }

    let current_tab = state.current_tab();
    let is_active = current_tab
        .and_then(|tab| tab.file().map(|f| f == path))
        .unwrap_or(false);

    let is_keyboard_focused = *state.focused_panel.read() == FocusedPanel::LeftSidebar
        && state
            .sidebar_cursor
            .read()
            .as_ref()
            .is_some_and(|p| p == &path);

    let indent_style = format!("padding-left: {}px", depth * 20);

    // Copy feedback state
    let mut is_copied = use_signal(|| false);

    // Right-click opens the shared, hoisted context menu. The node only *sets*
    // the menu state in AppState; `SidebarContextMenuHost` (rendered at the
    // app-container root) owns the action handlers and the rendering. Because
    // the menu lives outside this `refresh_counter`-keyed subtree, a watcher
    // remount can no longer unmount an open menu.
    let handle_context_menu = {
        let path = path.clone();
        move |evt: Event<MouseData>| {
            evt.prevent_default();
            evt.stop_propagation();

            // The menu renders at the (unzoomed) app root, so client coordinates
            // are already in unscaled viewport pixels — no zoom compensation.
            let cursor = {
                let coords = evt.data().client_coordinates();
                (coords.x as i32, coords.y as i32)
            };
            let viewport = {
                let size = *state.size.read();
                (size.width as i32, size.height as i32)
            };

            // Collect the other visible windows for the "Open in Window" submenu.
            let current_id = window().id();
            let other_windows = crate::window::main::list_visible_main_windows()
                .iter()
                .filter(|w| w.window.id() != current_id)
                .map(|w| (w.window.id(), w.window.title()))
                .collect();

            let kind = if is_dir {
                SidebarItemKind::Directory
            } else {
                SidebarItemKind::File
            };
            let data =
                SidebarContextMenuData::new(cursor, viewport, path.clone(), kind, other_windows);
            state.sidebar_context_menu.set(Some(data));
            tracing::trace!(?path, "Sidebar context menu opened");
        }
    };

    rsx! {
        div {
            class: "left-sidebar-tree-node",
            class: if is_active { "active" },
            class: if is_keyboard_focused { "keyboard-focused" },

            // Full-row clickable design:
            // - Chevron: Expand/collapse (stops propagation)
            // - Folder/File icon+label: Expand/open (stops propagation)
            // This allows the entire row to be interactive while providing distinct
            // click areas for different actions.
            div {
                class: "left-sidebar-tree-node-content",
                style: "{indent_style}",
                oncontextmenu: handle_context_menu,
                onclick: {
                    let path = path.clone();
                    move |_| {
                        // Click anywhere on the row: open file (files) or toggle expansion (directories)
                        if is_dir {
                            state.toggle_directory_expansion(&path);
                        } else {
                            state.open_file(&path);
                        }
                    }
                },

                // Directory: chevron and folder+label both toggle expansion
                if is_dir {
                    // Chevron: click to expand/collapse
                    span {
                        class: if is_expanded {
                            "left-sidebar-tree-chevron-wrapper expanded"
                        } else {
                            "left-sidebar-tree-chevron-wrapper"
                        },
                        onclick: {
                            let path = path.clone();
                            move |evt| {
                                evt.stop_propagation();
                                state.toggle_directory_expansion(&path);
                            }
                        },
                        Icon {
                            name: IconName::ChevronRight,
                            size: 16,
                            class: "left-sidebar-tree-chevron",
                        }
                    }

                    // Folder icon + label: click to expand/collapse
                    span {
                        class: "left-sidebar-tree-dir-link",
                        onclick: {
                            let path = path.clone();
                            move |evt| {
                                evt.stop_propagation();
                                state.toggle_directory_expansion(&path);
                            }
                        },
                        Icon {
                            name: if is_expanded { IconName::FolderOpen } else { IconName::Folder },
                            size: 16,
                            class: "left-sidebar-tree-icon",
                        }
                        span {
                            class: "left-sidebar-tree-label",
                            "{name}"
                        }
                    }
                } else {
                    // File: spacer + icon + label, click to open
                    span { class: "left-sidebar-tree-spacer" }
                    span {
                        class: "left-sidebar-tree-file-link",
                        onclick: {
                            let path = path.clone();
                            move |evt| {
                                evt.stop_propagation();
                                state.open_file(&path);
                            }
                        },
                        Icon {
                            name: IconName::File,
                            size: 16,
                            class: "left-sidebar-tree-icon",
                        }
                        span {
                            class: "left-sidebar-tree-label",
                            class: if !is_markdown { "disabled" },
                            "{name}"
                        }
                    }
                }

                // Bookmark button
                BookmarkButton { path: path.clone(), size: 12 }

                // Copy path button
                button {
                    class: "left-sidebar-tree-copy-button",
                    class: if *is_copied.read() { "copied" },
                    title: "Copy full path",
                    onclick: move |evt| {
                        evt.stop_propagation();
                        crate::utils::clipboard::copy_text(path.to_string_lossy());
                        // Show success feedback
                        is_copied.set(true);
                        spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            is_copied.set(false);
                        });
                    },
                    Icon {
                        name: if *is_copied.read() { IconName::Check } else { IconName::Copy },
                        size: 12,
                    }
                }
            }

            // Expanded directory children
            // DirectoryChildren is a separate component so that Dioxus's
            // component memoization skips re-rendering (and re-reading the
            // filesystem) when only unrelated state changes (tabs, sidebar
            // toggles, etc.).
            if is_dir && is_expanded {
                DirectoryChildren { path: path.clone(), depth, refresh_counter }
            }
        }
    }
}

/// Renders the left-sidebar file-tree context menu once, at the app-container
/// root, driven by `AppState::sidebar_context_menu`.
///
/// # Why hoisted here
///
/// The file tree is keyed on `AppState::sidebar_refresh_counter`, which the
/// directory watcher bumps on every filesystem event. Rendering the menu inside
/// a tree node meant a watcher-driven remount destroyed the node — and the open
/// menu with it. Hosting the menu here, outside every keyed subtree, makes it
/// independent of those remounts: tree nodes only *set* the menu state, and this
/// host owns every action handler.
#[component]
pub fn SidebarContextMenuHost() -> Element {
    let mut state = use_context::<AppState>();

    // Subscribe to the menu state; render nothing while the menu is closed.
    let Some(data) = state.sidebar_context_menu.read().clone() else {
        return rsx! {};
    };

    let path = data.path.clone();
    let is_dir = data.kind.is_dir();

    // Handler for "Open File" / "Open Directory"
    let handle_open = {
        let path = path.clone();
        move |_| {
            // The snapshotted target may have been deleted/renamed while the menu
            // stayed open; close as a no-op instead of acting on a stale path.
            if !context_action_should_proceed(&path) {
                state.close_sidebar_context_menu();
                return;
            }
            if is_dir {
                state.add_root(&path);
            } else {
                state.open_file(&path);
            }
            state.close_sidebar_context_menu();
        }
    };

    // Handler for "Change Root Directory"
    let handle_change_root_directory = {
        let path = path.clone();
        move |_| {
            if !context_action_should_proceed(&path) {
                state.close_sidebar_context_menu();
                return;
            }
            state.add_root(&path);
            state.close_sidebar_context_menu();
        }
    };

    // Handler for "Open in New Window"
    let handle_open_in_new_window = {
        let path = path.clone();
        move |_| {
            if !context_action_should_proceed(&path) {
                state.close_sidebar_context_menu();
                return;
            }
            let path = path.clone();
            spawn(async move {
                let (tab, directory) = if is_dir {
                    (crate::state::Tab::default(), Some(path))
                } else {
                    (
                        crate::state::Tab::new(&path),
                        path.parent().map(|p| p.to_path_buf()),
                    )
                };

                let params = crate::window::main::CreateMainWindowConfigParams {
                    directory,
                    ..Default::default()
                };
                crate::window::main::create_main_window(tab, params).await;
            });
            state.close_sidebar_context_menu();
        }
    };

    // Handler for "Open in Window" (open in an existing window)
    let handle_open_in_window = {
        let path = path.clone();
        move |target_id: dioxus::desktop::tao::window::WindowId| {
            if !context_action_should_proceed(&path) {
                state.close_sidebar_context_menu();
                return;
            }
            let path = path.clone();
            let result = if is_dir {
                // For directories, broadcast to change root directory
                crate::events::OPEN_DIRECTORY_IN_WINDOW.send((target_id, path))
            } else {
                // For files, broadcast to open file
                crate::events::OPEN_FILE_IN_WINDOW.send((target_id, path))
            };
            if result.is_err() {
                tracing::warn!(
                    ?target_id,
                    "Failed to open in window: target window may be closed"
                );
                state.close_sidebar_context_menu();
                return;
            }
            // Focus the target window
            crate::window::main::focus_window(target_id);
            state.close_sidebar_context_menu();
        }
    };

    // Handler for "Copy File Path" / "Copy Directory Path"
    let handle_copy_path = {
        let path = path.clone();
        move |_| {
            crate::utils::clipboard::copy_text(path.to_string_lossy());
            state.close_sidebar_context_menu();
        }
    };

    // Handler for "Reveal in Finder"
    let handle_reveal_in_finder = {
        let path = path.clone();
        move |_| {
            if !context_action_should_proceed(&path) {
                state.close_sidebar_context_menu();
                return;
            }
            file_operations::reveal_in_finder(&path);
            state.close_sidebar_context_menu();
        }
    };

    // Handler for "Reload"
    let handle_reload = move |_| {
        state.bump_sidebar_refresh();
        state.close_sidebar_context_menu();
    };

    // Handler for "Toggle Bookmark"
    let handle_toggle_bookmark = {
        let path = path.clone();
        move |_| {
            crate::bookmarks::toggle_bookmark(&path);
            state.close_sidebar_context_menu();
        }
    };

    rsx! {
        SidebarContextMenu {
            position: data.position,
            path: path.clone(),
            kind: data.kind,
            submenu_left: data.submenu_left,
            submenu_offset_y: data.submenu_offset_y,
            on_close: move |_| state.close_sidebar_context_menu(),
            on_open: handle_open,
            on_open_in_new_window: handle_open_in_new_window,
            on_move_to_window: handle_open_in_window,
            on_change_root_directory: handle_change_root_directory,
            on_toggle_bookmark: handle_toggle_bookmark,
            on_copy_path: handle_copy_path,
            on_reveal_in_finder: handle_reveal_in_finder,
            on_reload: handle_reload,
            other_windows: data.other_windows.clone(),
        }
    }
}

/// Hook to watch a directory for file system changes and trigger refresh
fn use_directory_watcher(directory: ReadSignal<Option<PathBuf>>, mut refresh_counter: Signal<u32>) {
    // Cancellation signal for the currently active watcher task.
    let mut stop_tx = use_signal(|| None::<oneshot::Sender<()>>);

    use_effect(move || {
        let directory = directory();
        // Cancel previous watcher when target directory changes.
        if let Some(tx) = stop_tx.write().take() {
            let _ = tx.send(());
        }

        spawn(async move {
            let Some(dir) = directory else {
                return;
            };

            let (tx, mut stop_rx) = oneshot::channel();
            stop_tx.set(Some(tx));

            // Start watching the directory (direct children only)
            let Ok(mut watcher) = FILE_WATCHER
                .watch_directory_non_recursive(dir.clone())
                .await
            else {
                tracing::error!("Failed to start directory watcher for {:?}", dir);
                return;
            };

            tracing::debug!("Directory watcher started (non-recursive) for {:?}", dir);

            // Listen for changes and trigger refresh
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    changed = watcher.recv() => {
                        if changed.is_none() {
                            break;
                        }
                        tracing::trace!(?dir, "Directory changed, triggering refresh");
                        refresh_counter.set(refresh_counter() + 1);
                    }
                }
            }

            let _ = FILE_WATCHER.unwatch_directory_non_recursive(dir).await;
        });
    });

    // Ensure watcher task gets cancelled when component unmounts.
    use_drop(move || {
        if let Some(tx) = stop_tx.write().take() {
            let _ = tx.send(());
        }
    });
}
