mod drag_drop_overlay;
mod drag_handlers;
mod drop_handlers;
mod keybinding_engine;
mod listeners;
mod shortcut_overlay;

use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::event::{DeviceEvent, ElementState, Event as TaoEvent, WindowEvent};
use dioxus::desktop::{use_muda_event_handler, use_wry_event_handler, window};
use dioxus::document;
use dioxus::prelude::*;
use dioxus_core::use_drop;
use std::path::PathBuf;

use super::content::{
    close_context_menu, use_search_handler, Content, ContentContextMenu, CONTENT_CONTEXT_MENU,
};
use super::header::Header;
use super::right_sidebar::RightSidebar;
use super::right_sidebar::RightSidebarTab;
use super::search_bar::SearchBar;
use super::sidebar::Sidebar;
use super::tab::TabBar;
use crate::assets::get_main_script_path;
use crate::drag;
use crate::events::{ActiveDragUpdate, ACTIVE_DRAG_UPDATE};
use crate::menu;
use crate::state::{AppState, PersistedState, Tab};
use crate::theme::Theme;

use drag_drop_overlay::DragDropOverlay;
use drag_handlers::{handle_drag_mouse_motion, handle_drag_mouse_release};
use drop_handlers::handle_dropped_files;
use keybinding_engine::setup_keybinding_engine;
use listeners::setup_cross_window_open_listeners;
use shortcut_overlay::{
    build_shortcut_help_items, close_shortcut_overlay, split_shortcut_help_columns,
    ShortcutHelpOverlay, ShortcutOverlayVisibility,
};

/// Left mouse button ID for DeviceEvent::Button (platform-dependent raw value)
const MOUSE_BUTTON_LEFT: u32 = 0;

#[component]
pub fn App(
    tabs: Vec<Tab>,     // Initial tabs (at least one tab must be present)
    directory: PathBuf, // Directory (resolved in create_main_window or MainApp)
    theme: Theme,       // The enum: Auto/Light/Dark
    sidebar_pinned: bool,
    sidebar_width: f64,
    sidebar_show_all_files: bool,
    sidebar_zoom_level: f64,
    right_sidebar_pinned: bool,
    right_sidebar_width: f64,
    right_sidebar_tab: RightSidebarTab,
    right_sidebar_zoom_level: f64,
    zoom_level: f64,
) -> Element {
    // Initialize application state with the provided tab
    let mut state = use_context_provider(|| {
        let mut app_state = AppState::new(theme);
        let initial_tabs = if tabs.is_empty() {
            vec![Tab::default()]
        } else {
            tabs
        };

        // Initialize with provided tabs (preserves ordering)
        app_state.tabs.set(initial_tabs);
        app_state.active_tab.set(0);

        // Apply initial sidebar settings from params (including directory)
        {
            let mut sidebar = app_state.sidebar.write();
            sidebar.root_directory = Some(directory.clone());
            sidebar.push_to_history(directory);
            sidebar.pinned = sidebar_pinned;
            sidebar.width = sidebar_width;
            sidebar.show_all_files = sidebar_show_all_files;
        }

        // Apply initial right sidebar settings from params
        {
            app_state.right_sidebar_pinned.set(right_sidebar_pinned);
            app_state.right_sidebar_width.set(right_sidebar_width);
            app_state.right_sidebar_tab.set(right_sidebar_tab);
        }

        // Apply initial zoom levels from params (already normalized in window::settings)
        {
            app_state.sidebar.write().zoom_level = sidebar_zoom_level;
            app_state
                .right_sidebar_zoom_level
                .set(right_sidebar_zoom_level);
            app_state.zoom_level.set(zoom_level);
        }

        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        *app_state.position.write() = LogicalPosition::new(metrics.position.x, metrics.position.y);
        *app_state.size.write() = LogicalSize::new(metrics.size.width, metrics.size.height);

        // Register this window in MAIN_WINDOWS list for cross-window access.
        // This enables fire-and-forget window creation (no need to await new_window()).
        crate::window::register_main_window(std::rc::Rc::downgrade(&window()));

        // Register this window's state for cross-window access
        crate::window::register_window_state(window().id(), app_state);

        app_state
    });

    // Track drag-and-drop hover state
    let mut is_dragging = use_signal(|| false);

    // Initialize JavaScript main module (theme listeners, etc.)
    use_hook(|| {
        spawn(async move {
            let _ = document::eval(&format!(
                r#"
                (async () => {{
                    try {{
                        const {{ init }} = await import("{}");
                        init();
                    }} catch (error) {{
                        console.error("Failed to load main module:", error);
                    }}
                }})();
                "#,
                get_main_script_path()
            ))
            .await;
        });
    });

    // Setup search handlers at App level (window-wide feature)
    use_search_handler(state);

    // Toggle for keyboard shortcut help overlay (which-key style)
    let shortcut_overlay_visibility = use_signal(|| ShortcutOverlayVisibility::Hidden);

    // Set up keybinding engine (keyboard shortcut processing)
    setup_keybinding_engine(state, shortcut_overlay_visibility);

    // Handle menu events (only state-dependent events, not global ones)
    use_muda_event_handler(move |event| {
        // Only handle state-dependent events
        menu::handle_menu_event_with_state(event, &mut state);
    });

    // Handle window events
    use_wry_event_handler(move |event, _| match event {
        TaoEvent::WindowEvent {
            event: WindowEvent::Resized(size),
            window_id,
            ..
        } => {
            let window = window();
            if window_id == &window.id() {
                sync_window_metrics(
                    state,
                    None,
                    Some(size.to_logical::<u32>(window.scale_factor())),
                );
            }
        }
        TaoEvent::WindowEvent {
            event: WindowEvent::Moved(position),
            window_id,
            ..
        } => {
            let window = window();
            if window_id == &window.id() {
                sync_window_metrics(
                    state,
                    Some(position.to_logical::<i32>(window.scale_factor())),
                    None,
                );
            }
        }
        // DeviceEvent: Global mouse tracking for tab drag
        // These events are delivered regardless of window focus, enabling cross-window drag
        TaoEvent::DeviceEvent {
            event: DeviceEvent::MouseMotion { .. },
            ..
        } => {
            // Only process if we're the source window of an active drag
            if let Some(dragged) = drag::get_dragged_tab() {
                if dragged.source_window_id == window().id() && drag::is_active_drag() {
                    handle_drag_mouse_motion(state);
                }
            }
        }
        TaoEvent::DeviceEvent {
            event:
                DeviceEvent::Button {
                    state: ElementState::Released,
                    button,
                    ..
                },
            ..
        } => {
            if *button == MOUSE_BUTTON_LEFT {
                if let Some(dragged) = drag::get_dragged_tab() {
                    if dragged.source_window_id == window().id() && drag::is_active_drag() {
                        handle_drag_mouse_release(state);
                    }
                }
            }
        }
        _ => {}
    });

    // Listen for cross-window file/directory open events (from sidebar context menu)
    setup_cross_window_open_listeners(state);

    // Update window title when active tab changes
    use_effect(move || {
        let active_index = *state.active_tab.read();
        let tabs = state.tabs.read();

        if let Some(tab) = tabs.get(active_index) {
            let title = crate::utils::window_title::generate_window_title(&tab.content);
            window().set_title(&title);
        }
    });

    // Listen for tab transfer events (from drag-and-drop and context menu "Move to Window")
    use_future(move || async move {
        let mut rx = crate::events::TRANSFER_TAB_TO_WINDOW.subscribe();
        let current_window_id = window().id();

        while let Ok((target_window_id, target_index, tab)) = rx.recv().await {
            // Only process transfers targeted to this window
            if target_window_id != current_window_id {
                continue;
            }

            tracing::debug!(?target_window_id, ?target_index, "Received tab transfer");

            // Insert the tab at the specified position
            let tabs_len = state.tabs.read().len();
            let insert_index = target_index.unwrap_or(tabs_len);
            let new_tab_index = state.insert_tab(tab, insert_index);
            state.switch_to_tab(new_tab_index);

            // Focus this window after receiving the tab
            window().set_focus();

            tracing::info!("Tab transfer completed");
        }
    });

    // Save state and close child windows when this window closes
    use_drop(move || {
        let window_id = window().id();

        // Clean up drag state if this window was the drag source
        // This prevents orphaned tabs when source window closes during drag
        if let Some(dragged) = drag::get_dragged_tab() {
            if dragged.source_window_id == window_id {
                if let Some(active) = drag::get_active_drag() {
                    let is_single_tab = active.source_tab_count == 1;

                    match &active.detach_state {
                        drag::DetachState::None => {
                            // Multi-tab: tab was removed, restore it to this window
                            if !is_single_tab {
                                state.insert_tab(dragged.tab.clone(), dragged.source_index);
                            }
                            // Single-tab: tab is still in window, nothing to restore
                        }
                        drag::DetachState::Pending { .. } | drag::DetachState::Creating => {
                            // Multi-tab: tab was removed, restore it to this window
                            if !is_single_tab {
                                state.insert_tab(dragged.tab.clone(), dragged.source_index);
                            }
                            // Single-tab: tab is still in window, nothing to restore
                        }
                        drag::DetachState::Detached { .. } => {
                            // Preview window exists - commit it as permanent window
                            crate::window::commit_preview_window();
                        }
                    }
                }
                // Clear global drag state
                drag::end_active_drag();
                drag::end_drag();
                // Notify other windows to clear drag UI
                ACTIVE_DRAG_UPDATE.send(ActiveDragUpdate).ok();
            }
        }

        // Unregister this window's state from the global mapping
        crate::window::unregister_window_state(window_id);

        // Save last used state from this window to disk for next app launch
        let mut persisted = PersistedState::from(&state);
        let window_metrics = crate::window::metrics::capture_window_metrics(&window().window);
        persisted.window_position = window_metrics.position;
        persisted.window_size = window_metrics.size;
        persisted.save();

        // Close child windows
        crate::window::close_child_windows_for_parent(window_id);
    });

    // Hover state for overlay sidebars is stored in AppState
    // so that dispatcher.rs (keybinding focus actions) can access it.
    let mut left_hover_active = state.left_hover_active;
    let mut right_hover_active = state.right_hover_active;
    // Generation counters for auto-hide timer cancellation
    let mut left_hide_gen = use_signal(|| 0u32);
    let mut right_hide_gen = use_signal(|| 0u32);
    // Track whether mouse is physically inside the overlay wrapper.
    // Used by on_resize_change to decide whether to start a hide timer:
    // if mouse is inside, onmouseleave will handle hiding naturally.
    let mut left_mouse_inside = use_signal(|| false);
    let mut right_mouse_inside = use_signal(|| false);

    let left_pinned = state.sidebar.read().pinned;
    let right_pinned = *state.right_sidebar_pinned.read();

    // Disable sidebar hover on Preferences (full-screen settings page)
    let enable_sidebar_hover = state
        .current_tab()
        .is_none_or(|tab| !matches!(tab.content, crate::state::TabContent::Preferences));

    // Delay in milliseconds before auto-hiding the overlay sidebar.
    // 300ms is the standard "Hover Intent" delay (Nielsen Norman Group),
    // balancing responsiveness with prevention of flickering at boundaries.
    const OVERLAY_HIDE_DELAY_MS: u64 = 300;

    let focused_panel = *state.focused_panel.read();
    let focused_context = focused_panel.key_context();
    let shortcut_help_columns = if !matches!(
        *shortcut_overlay_visibility.read(),
        ShortcutOverlayVisibility::Hidden
    ) {
        split_shortcut_help_columns(
            build_shortcut_help_items(focused_context),
            state.size.read().width,
        )
    } else {
        Vec::new()
    };

    rsx! {
        div {
            class: "app-container",
            class: if is_dragging() { "drag-over" },
            ondragover: move |evt| {
                evt.prevent_default();
                is_dragging.set(true);
            },
            ondragleave: move |evt| {
                evt.prevent_default();
                is_dragging.set(false);
            },
            ondrop: move |evt| {
                evt.prevent_default();
                is_dragging.set(false);

                spawn(async move {
                    handle_dropped_files(evt, state).await;
                });
            },

            // Left sidebar: pinned → flex layout, unpinned → overlay with animation
            if left_pinned {
                Sidebar {
                    on_pin_toggle: move |_| {
                        state.sidebar.write().pinned = false;
                        left_hover_active.set(true);
                    },
                }
            }

            div {
                class: "main-area",
                Header {},
                SearchBar {},
                TabBar {},
                Content {},
            }

            // Right sidebar: pinned → flex layout, unpinned → overlay with animation
            if right_pinned {
                RightSidebar {
                    headings: state.right_sidebar_headings.read().clone(),
                    on_pin_toggle: move |_| {
                        state.right_sidebar_pinned.set(false);
                        right_hover_active.set(true);
                    },
                }
            }

            // Hover triggers and overlays (only when unpinned, on content-display tabs)
            if !left_pinned && enable_sidebar_hover {
                div {
                    class: "sidebar-hover-trigger left",
                    onmouseenter: move |_| {
                        left_hover_active.set(true);
                        left_hide_gen.set(left_hide_gen() + 1);
                    },
                }
            }

            if !right_pinned && enable_sidebar_hover {
                div {
                    class: "sidebar-hover-trigger right",
                    onmouseenter: move |_| {
                        right_hover_active.set(true);
                        right_hide_gen.set(right_hide_gen() + 1);
                    },
                }
            }

            // Overlay wrappers (rendered when unpinned, animated via .visible class)
            if !left_pinned && enable_sidebar_hover {
                div {
                    class: "sidebar-overlay-wrapper left",
                    class: if left_hover_active() { "visible" },
                    onmouseenter: move |_| {
                        left_mouse_inside.set(true);
                        left_hide_gen.set(left_hide_gen() + 1);
                    },
                    onmouseleave: move |evt| {
                        left_mouse_inside.set(false);
                        // Don't auto-hide while mouse button is held (e.g., resize drag)
                        if evt.data().held_buttons().contains(dioxus::html::input_data::MouseButton::Primary) {
                            return;
                        }
                        let gen = left_hide_gen() + 1;
                        left_hide_gen.set(gen);
                        spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(OVERLAY_HIDE_DELAY_MS)).await;
                            if left_hide_gen() == gen {
                                left_hover_active.set(false);
                            }
                        });
                    },
                    Sidebar {
                        on_pin_toggle: move |_| {
                            state.sidebar.write().pinned = true;
                            left_hover_active.set(false);
                        },
                        on_resize_change: move |resizing: bool| {
                            if resizing {
                                // Cancel any pending hide timer
                                left_hide_gen.set(left_hide_gen() + 1);
                            } else if !left_mouse_inside() {
                                // Resize ended with mouse outside: start hide timer
                                let gen = left_hide_gen() + 1;
                                left_hide_gen.set(gen);
                                spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(OVERLAY_HIDE_DELAY_MS)).await;
                                    if left_hide_gen() == gen {
                                        left_hover_active.set(false);
                                    }
                                });
                            }
                            // Resize ended with mouse inside: do nothing,
                            // onmouseleave will handle hiding when mouse leaves.
                        },
                    }
                }
            }

            if !right_pinned && enable_sidebar_hover {
                div {
                    class: "sidebar-overlay-wrapper right",
                    class: if right_hover_active() { "visible" },
                    onmouseenter: move |_| {
                        right_mouse_inside.set(true);
                        right_hide_gen.set(right_hide_gen() + 1);
                    },
                    onmouseleave: move |evt| {
                        right_mouse_inside.set(false);
                        // Don't auto-hide while mouse button is held (e.g., resize drag)
                        if evt.data().held_buttons().contains(dioxus::html::input_data::MouseButton::Primary) {
                            return;
                        }
                        let gen = right_hide_gen() + 1;
                        right_hide_gen.set(gen);
                        spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(OVERLAY_HIDE_DELAY_MS)).await;
                            if right_hide_gen() == gen {
                                right_hover_active.set(false);
                            }
                        });
                    },
                    RightSidebar {
                        headings: state.right_sidebar_headings.read().clone(),
                        on_pin_toggle: move |_| {
                            state.right_sidebar_pinned.set(true);
                            right_hover_active.set(false);
                        },
                        on_resize_change: move |resizing: bool| {
                            if resizing {
                                // Cancel any pending hide timer
                                right_hide_gen.set(right_hide_gen() + 1);
                            } else if !right_mouse_inside() {
                                // Resize ended with mouse outside: start hide timer
                                let gen = right_hide_gen() + 1;
                                right_hide_gen.set(gen);
                                spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(OVERLAY_HIDE_DELAY_MS)).await;
                                    if right_hide_gen() == gen {
                                        right_hover_active.set(false);
                                    }
                                });
                            }
                            // Resize ended with mouse inside: do nothing,
                            // onmouseleave will handle hiding when mouse leaves.
                        },
                    }
                }
            }

            // Drag and drop overlay
            if is_dragging() {
                DragDropOverlay {}
            }

            if !matches!(
                *shortcut_overlay_visibility.read(),
                ShortcutOverlayVisibility::Hidden
            ) {
                ShortcutHelpOverlay {
                    columns: shortcut_help_columns,
                    is_closing: matches!(
                        *shortcut_overlay_visibility.read(),
                        ShortcutOverlayVisibility::Closing
                    ),
                    on_close: move |_| close_shortcut_overlay(shortcut_overlay_visibility),
                }
            }

            // Content context menu (rendered at App level to prevent FileViewer re-renders)
            if let Some(menu_state) = CONTENT_CONTEXT_MENU.read().as_ref() {
                ContentContextMenu {
                    position: (menu_state.data.x, menu_state.data.y),
                    context: menu_state.data.context.clone(),
                    has_selection: menu_state.data.has_selection,
                    selected_text: menu_state.data.selected_text.clone(),
                    current_file: menu_state.current_file.clone(),
                    base_dir: menu_state.base_dir.clone(),
                    source_line: menu_state.data.source_line,
                    source_line_end: menu_state.data.source_line_end,
                    table_csv: menu_state.data.table_csv.clone(),
                    table_tsv: menu_state.data.table_tsv.clone(),
                    table_markdown: menu_state.data.table_markdown.clone(),
                    table_source_line: menu_state.data.table_source_line,
                    table_source_line_end: menu_state.data.table_source_line_end,
                    on_close: move |_| {
                        close_context_menu();
                        crate::keybindings::dispatcher::content_cursor_eval("clearCursorDeferred");
                    },
                }
            }
        }
    }
}

fn sync_window_metrics(
    mut state: AppState,
    position: Option<LogicalPosition<i32>>,
    size: Option<LogicalSize<u32>>,
) {
    if let Some(position) = position {
        *state.position.write() = position;
    }
    if let Some(size) = size {
        *state.size.write() = size;
    }
}
