use dioxus::prelude::*;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;
use tokio::sync::oneshot;

use super::context_menu::{
    context_action_should_proceed, SidebarContextMenu, SidebarContextMenuData, SidebarItemKind,
};
use crate::components::icon::{Icon, IconName};
use crate::components::sidebar::reorder::{drop_class, drop_side, DragRow};
use crate::components::sidebar::row_actions::RowActions;
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
    // Where the window is when it has no folder of its own: inside one of the
    // places. Starring this window's folder promotes it into that list, which
    // is the right place for it — but the window did not stop being in it, and
    // a group that answered "nowhere" would be saying something untrue.
    let inside_place = use_memo(move || {
        let file = state.current_file()?;
        let sidebar = state.sidebar.read();
        let root = sidebar.roots.covering(&file)?;
        root.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    });

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

    rsx! {
        div {
            class: "left-sidebar-explorer",
            key: "{refresh_counter}",

            if !places().is_empty() {
                RootGroup {
                    label: "Places",
                    roots: places(),
                    closable: false,
                    refresh_counter,
                    reorderable: true,
                }
            }

            // Always drawn, and never more useful than with nothing above it:
            // a folder chosen through a dialog is a deliberate act, so it
            // becomes a place — bookmarking it and rooting the tree at it are
            // the same thing.
            div {
                class: "left-sidebar-add-root",
                onclick: move |_| {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        crate::bookmarks::toggle_bookmark(dir);
                    }
                },
                Icon { name: IconName::FolderPlus, size: 12 }
                span { "Add folder…" }
            }

            // Where this window is, which is one folder. Always drawn, even
            // when it is nowhere yet: the glyph beside the label is the only
            // way to say where, and a control that appears once the answer
            // exists cannot be the thing that answers.
            RootGroup {
                label: "This window",
                roots: temps(),
                closable: true,
                refresh_counter,
                inside_place: inside_place(),
                on_change: move |_| {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.add_root(dir);
                    }
                },
            }
        }
    }
}

/// Open the sidebar's menu on a root row.
///
/// The same menu the tree's own rows raise — a root is a directory like any
/// other — with the one entry that only a root has.
fn open_root_context_menu(
    mut state: AppState,
    root: &std::path::Path,
    closable: bool,
    evt: &Event<MouseData>,
) {
    let cursor = {
        let coords = evt.data().client_coordinates();
        (coords.x as i32, coords.y as i32)
    };
    let viewport = {
        let size = *state.size.read();
        (size.width as i32, size.height as i32)
    };
    state
        .sidebar_context_menu
        .set(Some(SidebarContextMenuData::new(
            cursor,
            viewport,
            root.to_path_buf(),
            SidebarItemKind::Directory,
            closable,
        )));
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
    /// Offered on the group whose root the window can be moved to another
    /// folder — which is the window's own, never the places.
    on_change: Option<EventHandler<()>>,
    /// The place the window is reading inside, when it has no folder of its
    /// own. Being inside a place is an answer to "where am I"; having no
    /// folder at all is a different one.
    #[props(default = None)]
    inside_place: Option<String>,
    /// Whether the rows of this group are a list somebody arranged. The places
    /// are; the one folder this window is in has no order to be in.
    #[props(default = false)]
    reorderable: bool,
) -> Element {
    let mut dragging = use_signal(|| None::<DragRow>);
    let mut drop_target = use_signal(|| None::<DragRow>);

    rsx! {
        div {
            class: "left-sidebar-root-group-label",
            span { "{label}" }
            if let Some(on_change) = on_change {
                button {
                    class: "left-sidebar-root-group-action",
                    title: "Change this window's folder",
                    onclick: move |_| on_change.call(()),
                    Icon { name: IconName::FolderOpen, size: 12 }
                }
            }
        }
        if roots.is_empty() {
            div {
                class: "left-sidebar-root-empty",
                if let Some(place) = inside_place.clone() {
                    "Reading inside {place}"
                } else {
                    "Not in a folder yet"
                }
            }
        }

        for (index, root) in roots.into_iter().enumerate() {
            RootSubtree {
                key: "{root.display()}",
                root: root.clone(),
                closable,
                refresh_counter,
                index,
                reorderable,
                is_dragging: dragging.read().as_ref().map(|(at, _)| *at) == Some(index),
                drop_side: drop_side(&dragging.read(), &drop_target.read(), index),
                on_drag_start: move |row| dragging.set(Some(row)),
                on_drag_over: move |row| {
                    if dragging.read().is_some() {
                        drop_target.set(Some(row));
                    }
                },
                on_drag_leave: move |_| drop_target.set(None),
                on_drag_end: move |_| {
                    if let (Some((from, moved)), Some((to, target))) =
                        (dragging.take(), drop_target.take())
                    {
                        crate::bookmarks::move_bookmark(&moved, &target, from < to);
                    }
                },
            }
        }
    }
}

/// A root and everything shown under it.
#[component]
fn RootSubtree(
    root: PathBuf,
    closable: bool,
    refresh_counter: Signal<u32>,
    /// Where this root is drawn in its group, which is what a drag reads to
    /// tell which way it is going.
    #[props(default = 0)]
    index: usize,
    #[props(default = false)] reorderable: bool,
    #[props(default = false)] is_dragging: bool,
    #[props(default = None)] drop_side: Option<bool>,
    #[props(default = None)] on_drag_start: Option<EventHandler<DragRow>>,
    #[props(default = None)] on_drag_over: Option<EventHandler<DragRow>>,
    #[props(default = None)] on_drag_leave: Option<EventHandler<()>>,
    #[props(default = None)] on_drag_end: Option<EventHandler<()>>,
) -> Element {
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
        // A root is a row of the tree it heads, drawn by the tree's own rules:
        // the same chevron in the same place, the same icon, the same size. It
        // differs only in where it starts, which is what makes it a root.
        div {
            class: "left-sidebar-tree-node-content left-sidebar-root",
            class: if is_dragging { "dragging" },
            class: "{drop_class(drop_side)}",
            draggable: reorderable,
            ondragstart: {
                let root = root.clone();
                move |evt: Event<DragData>| {
                    evt.stop_propagation();
                    if let Some(handler) = on_drag_start {
                        handler.call((index, root.clone()));
                    }
                }
            },
            ondragover: {
                let root = root.clone();
                move |evt: Event<DragData>| {
                    if on_drag_over.is_none() {
                        return;
                    }
                    evt.stop_propagation();
                    evt.prevent_default();
                    if let Some(handler) = on_drag_over {
                        handler.call((index, root.clone()));
                    }
                }
            },
            ondragleave: move |evt: Event<DragData>| {
                if let Some(handler) = on_drag_leave {
                    evt.stop_propagation();
                    handler.call(());
                }
            },
            ondragend: move |evt: Event<DragData>| {
                if let Some(handler) = on_drag_end {
                    evt.stop_propagation();
                    handler.call(());
                }
            },
            // Closing a root is rare and irreversible-looking, so it lives
            // where the other rare things live rather than as a cross drawn on
            // every root for the life of the window.
            oncontextmenu: {
                let root = root.clone();
                move |evt: Event<MouseData>| {
                    evt.prevent_default();
                    evt.stop_propagation();
                    open_root_context_menu(state, &root, closable, &evt);
                }
            },
            onclick: {
                let root = root.clone();
                move |_| state.toggle_directory_expansion(&root)
            },

            span {
                class: if is_expanded {
                    "left-sidebar-tree-chevron-wrapper expanded"
                } else {
                    "left-sidebar-tree-chevron-wrapper"
                },
                onclick: {
                    let root = root.clone();
                    move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        state.toggle_directory_expansion(&root);
                    }
                },
                Icon {
                    name: IconName::ChevronRight,
                    size: 16,
                    class: "left-sidebar-tree-chevron",
                }
            }

            span {
                class: "left-sidebar-tree-dir-link",
                Icon {
                    name: if is_expanded { IconName::FolderOpen } else { IconName::Folder },
                    size: 16,
                    class: "left-sidebar-tree-icon",
                }
                span { class: "left-sidebar-tree-label", "{name}" }
            }

            // A root is a folder like the ones under it, so it answers to the
            // same controls. `closable` is this window's own root, which is
            // already the folder the window is in — there is nowhere for it to
            // be moved to.
            RowActions {
                path: root.clone(),
                rootable: !closable,
                root: true,
                place: !closable,
                starred: !closable,
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
/// re-reading the filesystem when only unrelated state (the document, sidebar
/// toggles)
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

    let is_active = state.current_file().as_deref() == Some(path.as_path());

    let is_keyboard_focused = *state.focused_panel.read() == FocusedPanel::LeftSidebar
        && state
            .sidebar_cursor
            .read()
            .as_ref()
            .is_some_and(|p| p == &path);

    // How deep the row sits, not how far in it is drawn: the step is the
    // stylesheet's to choose.
    let indent_style = format!("--tree-depth: {depth}");

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

            let kind = if is_dir {
                SidebarItemKind::Directory
            } else {
                SidebarItemKind::File
            };
            let data = SidebarContextMenuData::new(cursor, viewport, path.clone(), kind, false);
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

                // The same controls, in the same order, at the same place as
                // every other row that offers a document. A tree row is not
                // history, so there is nothing here to forget — and a folder
                // has one thing a document does not: it can be stood in.
                RowActions { path: path.clone(), rootable: is_dir }
            }

            // Expanded directory children
            // DirectoryChildren is a separate component so that Dioxus's
            // component memoization skips re-rendering (and re-reading the
            // filesystem) when only unrelated state changes (the document, sidebar
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
            let (document, directory) = if is_dir {
                (crate::state::Document::default(), Some(path))
            } else {
                (
                    crate::state::Document::new(&path),
                    path.parent().map(|p| p.to_path_buf()),
                )
            };
            let params = crate::window::main::CreateMainWindowConfigParams {
                directory,
                ..Default::default()
            };
            crate::window::create_main_window_sync(&dioxus::desktop::window(), document, params);
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

    let handle_close_root = {
        let path = path.clone();
        move |_| {
            state.close_root(&path);
            state.close_sidebar_context_menu();
        }
    };

    rsx! {
        SidebarContextMenu {
            position: data.position,
            path: path.clone(),
            kind: data.kind,
            on_close: move |_| state.close_sidebar_context_menu(),
            on_open: handle_open,
            on_open_in_new_window: handle_open_in_new_window,
            on_change_root_directory: handle_change_root_directory,
            on_toggle_bookmark: handle_toggle_bookmark,
            on_copy_path: handle_copy_path,
            on_reveal_in_finder: handle_reveal_in_finder,
            on_reload: handle_reload,
            on_close_root: data
                .temp_root
                .then_some(EventHandler::new(handle_close_root)),
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
