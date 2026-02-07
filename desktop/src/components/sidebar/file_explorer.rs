use dioxus::desktop::window;
use dioxus::prelude::*;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use super::context_menu::{SidebarContextMenu, SidebarItemKind};
use super::quick_access::QuickAccess;
use crate::components::bookmark_button::BookmarkButton;
use crate::components::icon::{Icon, IconName};
use crate::state::AppState;
use crate::utils::{file::is_markdown_file, file_operations};
use crate::watcher::FILE_WATCHER;

/// Sort entries: directories first, then files, both alphabetically.
///
/// This ordering is shared with `sidebar_cursor` to ensure the keyboard
/// cursor visits items in the same visual order as the file tree.
pub(crate) fn sort_entries(items: &mut [PathBuf]) {
    items.sort_by(|a, b| {
        let a_is_dir = a.is_dir();
        let b_is_dir = b.is_dir();

        match (a_is_dir, b_is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
}

// Read and sort directory entries
fn read_sorted_entries(path: &PathBuf) -> Vec<PathBuf> {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut items: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
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
    let state = use_context::<AppState>();
    let root_directory = state.sidebar.read().root_directory.clone();

    // Refresh counter to force DirectoryTree re-render
    let refresh_counter = use_signal(|| 0u32);

    // Watch directory for file system changes
    use_directory_watcher(root_directory.clone(), refresh_counter);

    rsx! {
        div {
            class: "left-sidebar-explorer",
            key: "{refresh_counter}",

            if let Some(root) = root_directory {
                DirectoryNavigation { current_dir: root.clone(), refresh_counter }
                DirectoryTree { path: root, refresh_counter }
            } else {
                div {
                    class: "left-sidebar-explorer-empty",
                    "No directory open"
                }
            }

            // Quick Access section (fixed at bottom)
            QuickAccess {}
        }
    }
}

#[component]
fn DirectoryNavigation(current_dir: PathBuf, mut refresh_counter: Signal<u32>) -> Element {
    let mut state = use_context::<AppState>();
    let sidebar = state.sidebar.read();
    let show_all_files = sidebar.show_all_files;
    let can_go_back = sidebar.can_go_back();
    let can_go_forward = sidebar.can_go_forward();
    drop(sidebar);

    let has_parent = current_dir.parent().is_some();

    // Get current directory name
    let dir_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("..")
        .to_string();

    // Copy feedback state
    let mut is_copied = use_signal(|| false);

    // Reload state for animation
    let is_reloading = use_signal(|| false);
    let mut is_reloading_write = is_reloading;

    let on_reload = {
        let current_dir = current_dir.clone();
        move |evt: Event<MouseData>| {
            evt.stop_propagation();

            // Set reloading state for animation
            is_reloading_write.set(true);

            // Increment counter to force DirectoryTree re-render
            refresh_counter.set(refresh_counter() + 1);

            // Reset reloading state after animation
            let current_dir = current_dir.clone();
            spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                is_reloading_write.set(false);
                tracing::trace!(?current_dir, "Directory reloaded");
            });
        }
    };

    rsx! {
        div {
            class: "left-sidebar-header",

            // History navigation buttons
            div {
                class: "left-sidebar-header-history",

                // Go back button
                button {
                    class: "left-sidebar-header-history-button",
                    class: if !can_go_back { "disabled" },
                    disabled: !can_go_back,
                    title: "Go back",
                    onclick: move |_| {
                        state.go_back_directory();
                    },
                    Icon {
                        name: IconName::ChevronLeft,
                        size: 16,
                    }
                }

                // Go forward button
                button {
                    class: "left-sidebar-header-history-button",
                    class: if !can_go_forward { "disabled" },
                    disabled: !can_go_forward,
                    title: "Go forward",
                    onclick: move |_| {
                        state.go_forward_directory();
                    },
                    Icon {
                        name: IconName::ChevronRight,
                        size: 16,
                    }
                }
            }

            // Parent directory navigation or root indicator
            if has_parent {
                div {
                    class: "left-sidebar-header-nav",
                    onclick: move |_| {
                        state.go_to_parent_directory();
                    },

                    div {
                        class: "left-sidebar-header-content",
                        span {
                            class: "left-sidebar-header-label",
                            "{dir_name}"
                        }

                        // Action buttons (bookmark, copy & reload) - shown on hover
                        div {
                            class: "left-sidebar-header-actions",

                            // Bookmark button
                            BookmarkButton { path: current_dir.clone() }

                            // Copy path button
                            button {
                                class: "left-sidebar-action-button copy-button",
                                class: if *is_copied.read() { "copied" },
                                title: "Copy directory path",
                                onclick: {
                                    let current_dir = current_dir.clone();
                                    move |evt: Event<MouseData>| {
                                        evt.stop_propagation();
                                        crate::utils::clipboard::copy_text(current_dir.to_string_lossy());
                                        is_copied.set(true);
                                        spawn(async move {
                                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                            is_copied.set(false);
                                        });
                                    }
                                },
                                Icon {
                                    name: if *is_copied.read() { IconName::Check } else { IconName::Copy },
                                    size: 14,
                                }
                            }

                            // Reload button
                            button {
                                class: "left-sidebar-action-button reload-button",
                                class: if *is_reloading.read() { "reloading" },
                                title: "Reload file explorer",
                                onclick: on_reload,
                                Icon {
                                    name: IconName::Refresh,
                                    size: 14,
                                }
                            }
                        }
                    }
                }
            } else {
                // Show root indicator when at filesystem root
                div {
                    class: "left-sidebar-header-nav root-indicator",

                    div {
                        class: "left-sidebar-header-content",
                        Icon {
                            name: IconName::Server,
                            size: 16,
                            class: "left-sidebar-header-icon",
                        }
                        span {
                            class: "left-sidebar-header-label",
                            "/"
                        }

                        // Action buttons (bookmark, copy & reload) - shown on hover
                        div {
                            class: "left-sidebar-header-actions",

                            // Bookmark button
                            BookmarkButton { path: current_dir.clone() }

                            // Copy path button
                            button {
                                class: "left-sidebar-action-button copy-button",
                                class: if *is_copied.read() { "copied" },
                                title: "Copy directory path",
                                onclick: {
                                    let current_dir = current_dir.clone();
                                    move |evt: Event<MouseData>| {
                                        evt.stop_propagation();
                                        crate::utils::clipboard::copy_text(current_dir.to_string_lossy());
                                        is_copied.set(true);
                                        spawn(async move {
                                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                            is_copied.set(false);
                                        });
                                    }
                                },
                                Icon {
                                    name: if *is_copied.read() { IconName::Check } else { IconName::Copy },
                                    size: 14,
                                }
                            }

                            // Reload button
                            button {
                                class: "left-sidebar-action-button reload-button",
                                class: if *is_reloading.read() { "reloading" },
                                title: "Reload file explorer",
                                onclick: on_reload,
                                Icon {
                                    name: IconName::Refresh,
                                    size: 14,
                                }
                            }
                        }
                    }
                }
            }

            // Toolbar buttons container (visibility toggle only)
            div {
                class: "left-sidebar-header-toolbar",

                // File visibility toggle button
                button {
                    class: "left-sidebar-header-toolbar-button",
                    title: if show_all_files { "Hide non-markdown files" } else { "Show all files" },
                    onclick: move |_| {
                        state.sidebar.write().show_all_files = !show_all_files;
                    },
                    Icon {
                        name: if show_all_files { IconName::Eye } else { IconName::EyeOff },
                        size: 20,
                    }
                }
            }
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
                FileTreeNode { path: entry, depth: 0, refresh_counter }
            }
        }
    }
}

#[component]
fn FileTreeNode(path: PathBuf, depth: usize, mut refresh_counter: Signal<u32>) -> Element {
    let mut state = use_context::<AppState>();

    let is_dir = path.is_dir();
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

    let is_keyboard_focused = state
        .sidebar_cursor
        .read()
        .as_ref()
        .map(|c| c == &path)
        .unwrap_or(false);

    let indent_style = format!("padding-left: {}px", depth * 20);

    // Copy feedback state
    let mut is_copied = use_signal(|| false);

    // Context menu state
    let mut show_context_menu = use_signal(|| false);
    let mut context_menu_position = use_signal(|| (0, 0));
    let mut other_windows = use_signal(Vec::new);

    // Handle right-click to show context menu
    let handle_context_menu = {
        let path = path.clone();
        move |evt: Event<MouseData>| {
            evt.prevent_default();
            evt.stop_propagation();
            let mouse_data = evt.data();
            context_menu_position.set((
                mouse_data.client_coordinates().x as i32,
                mouse_data.client_coordinates().y as i32,
            ));

            // Refresh window list
            let windows = crate::window::main::list_visible_main_windows();
            let current_id = window().id();
            other_windows.set(
                windows
                    .iter()
                    .filter(|w| w.window.id() != current_id)
                    .map(|w| (w.window.id(), w.window.title()))
                    .collect(),
            );

            show_context_menu.set(true);
            tracing::trace!(?path, "Context menu opened");
        }
    };

    // Handler for "Open File" or "Open Directory"
    let handle_open = {
        let path = path.clone();
        move |_| {
            if is_dir {
                state.set_root_directory(&path);
            } else {
                state.open_file(&path);
            }
            show_context_menu.set(false);
        }
    };

    // Handler for "Open in New Window"
    let handle_open_in_new_window = {
        let path = path.clone();
        move |_| {
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
            show_context_menu.set(false);
        }
    };

    // Handler for "Open in Window" (open in existing window)
    let handle_open_in_window = {
        let path = path.clone();
        move |target_id: dioxus::desktop::tao::window::WindowId| {
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
                show_context_menu.set(false);
                return;
            }
            // Focus the target window
            crate::window::main::focus_window(target_id);
            show_context_menu.set(false);
        }
    };

    // Handler for "Copy File Path" / "Copy Directory Path"
    let handle_copy_path = {
        let path = path.clone();
        move |_| {
            crate::utils::clipboard::copy_text(path.to_string_lossy());
            show_context_menu.set(false);
        }
    };

    // Handler for "Reveal in Finder"
    let handle_reveal_in_finder = {
        let path = path.clone();
        move |_| {
            file_operations::reveal_in_finder(&path);
            show_context_menu.set(false);
        }
    };

    // Handler for "Reload"
    let handle_reload = move |_| {
        refresh_counter.set(refresh_counter() + 1);
        show_context_menu.set(false);
    };

    // Handler for "Toggle Bookmark"
    let handle_toggle_bookmark = {
        let path = path.clone();
        move |_| {
            crate::bookmarks::toggle_bookmark(&path);
            show_context_menu.set(false);
        }
    };

    rsx! {
        div {
            class: "left-sidebar-tree-node",
            class: if is_active { "active" },
            class: if is_keyboard_focused { "keyboard-focused" },

            // Full-row clickable design:
            // - Parent row (this div): Fallback handler for empty space clicks
            // - Chevron: Expand/collapse (stops propagation)
            // - Folder/File icon+label: Navigate/open (stops propagation)
            // This allows the entire row to be interactive while providing distinct
            // click areas for different actions.
            div {
                class: "left-sidebar-tree-node-content",
                style: "{indent_style}",
                oncontextmenu: handle_context_menu,
                onclick: {
                    let path = path.clone();
                    move |_| {
                        // Click anywhere on the row: open file (files) or set as root (directories)
                        if is_dir {
                            state.set_root_directory(&path);
                        } else {
                            state.open_file(&path);
                        }
                    }
                },

                // Directory: chevron toggles expansion, folder+label changes root
                if is_dir {
                    // Chevron: click to expand/collapse
                    span {
                        class: "left-sidebar-tree-chevron-wrapper",
                        onclick: {
                            let path = path.clone();
                            move |evt| {
                                evt.stop_propagation();
                                state.toggle_directory_expansion(&path);
                            }
                        },
                        Icon {
                            name: if is_expanded { IconName::ChevronDown } else { IconName::ChevronRight },
                            size: 16,
                            class: "left-sidebar-tree-chevron",
                        }
                    }

                    // Folder icon + label: click to set as root directory
                    span {
                        class: "left-sidebar-tree-dir-link",
                        onclick: {
                            let path = path.clone();
                            move |evt| {
                                evt.stop_propagation();
                                state.set_root_directory(&path);
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
            if is_dir && is_expanded {
                {
                    let children = read_sorted_entries(&path);
                    rsx! {
                        div {
                            key: "{refresh_counter}",
                            for child in children {
                                FileTreeNode { path: child, depth: depth + 1, refresh_counter }
                            }
                        }
                    }
                }
            }
        }

        // Context menu
        if *show_context_menu.read() {
            SidebarContextMenu {
                position: *context_menu_position.read(),
                path: path.clone(),
                kind: if is_dir { SidebarItemKind::Directory } else { SidebarItemKind::File },
                on_close: move |_| show_context_menu.set(false),
                on_open: handle_open,
                on_open_in_new_window: handle_open_in_new_window,
                on_move_to_window: handle_open_in_window,
                on_toggle_bookmark: handle_toggle_bookmark,
                on_copy_path: handle_copy_path,
                on_reveal_in_finder: handle_reveal_in_finder,
                on_reload: handle_reload,
                other_windows: other_windows.read().clone(),
            }
        }
    }
}

/// Hook to watch a directory for file system changes and trigger refresh
fn use_directory_watcher(directory: Option<PathBuf>, mut refresh_counter: Signal<u32>) {
    use_effect(use_reactive!(|directory| {
        spawn(async move {
            let Some(dir) = directory else {
                return;
            };

            // Start watching the directory
            let Ok(mut watcher) = FILE_WATCHER.watch_directory(dir.clone()).await else {
                tracing::error!("Failed to start directory watcher for {:?}", dir);
                return;
            };

            tracing::debug!("Directory watcher started for {:?}", dir);

            // Listen for changes and trigger refresh
            while watcher.recv().await.is_some() {
                tracing::trace!(?dir, "Directory changed, triggering refresh");
                refresh_counter.set(refresh_counter() + 1);
            }

            // Cleanup when effect is re-run or component unmounts
            let _ = FILE_WATCHER.unwatch_directory(dir).await;
        });
    }));
}
