use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::event::{DeviceEvent, ElementState, Event as TaoEvent, WindowEvent};
use dioxus::desktop::{use_muda_event_handler, use_wry_event_handler, window};
use dioxus::document;
use dioxus::html::HasFileData;
use dioxus::prelude::*;
use dioxus_core::use_drop;
use mouse_position::mouse_position::Mouse;
use std::path::PathBuf;

use super::content::{
    close_context_menu, use_search_handler, Content, ContentContextMenu, CONTENT_CONTEXT_MENU,
};
use super::header::Header;
use super::icon::{Icon, IconName};
use super::right_sidebar::RightSidebar;
use super::right_sidebar::RightSidebarTab;
use super::search_bar::SearchBar;
use super::sidebar::Sidebar;
use super::tab::TabBar;
use crate::assets::MAIN_SCRIPT;
use crate::drag;
use crate::events::{
    ActiveDragUpdate, ACTIVE_DRAG_UPDATE, OPEN_DIRECTORY_IN_WINDOW, OPEN_FILE_IN_WINDOW,
};
use crate::menu;
use crate::state::{AppState, PersistedState, Tab};
use crate::theme::Theme;

/// Left mouse button ID for DeviceEvent::Button (platform-dependent raw value)
const MOUSE_BUTTON_LEFT: u32 = 0;

#[component]
pub fn App(
    tab: Tab,           // Initial tab (always provided, preserves history)
    directory: PathBuf, // Directory (resolved in create_new_main_window)
    theme: Theme,       // The enum: Auto/Light/Dark
    sidebar_open: bool,
    sidebar_width: f64,
    sidebar_show_all_files: bool,
    toc_open: bool,
    toc_width: f64,
    toc_tab: RightSidebarTab,
) -> Element {
    // Initialize application state with the provided tab
    let mut state = use_context_provider(|| {
        let mut app_state = AppState::new(theme);

        // Initialize with provided tab (preserves history)
        app_state.tabs.write()[0] = tab;

        // Apply initial sidebar settings from params (including directory)
        {
            let mut sidebar = app_state.sidebar.write();
            sidebar.root_directory = Some(directory.clone());
            sidebar.push_to_history(directory);
            sidebar.open = sidebar_open;
            sidebar.width = sidebar_width;
            sidebar.show_all_files = sidebar_show_all_files;
        }

        // Apply initial right sidebar settings from params
        {
            app_state.right_sidebar_open.set(toc_open);
            app_state.right_sidebar_width.set(toc_width);
            app_state.right_sidebar_tab.set(toc_tab);
        }

        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        *app_state.position.write() = LogicalPosition::new(metrics.position.x, metrics.position.y);
        *app_state.size.write() = LogicalSize::new(metrics.size.width, metrics.size.height);

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
                        const {{ init }} = await import("{MAIN_SCRIPT}");
                        init();
                    }} catch (error) {{
                        console.error("Failed to load main module:", error);
                    }}
                }})();
                "#
            ))
            .await;
        });
    });

    // Setup search handlers at App level (window-wide feature)
    use_search_handler(state);

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

            Sidebar {},

            div {
                class: "main-area",
                Header {},
                SearchBar {},
                TabBar {},
                Content {},
            }

            RightSidebar { headings: state.toc_headings.read().clone() }

            // Drag and drop overlay
            if is_dragging() {
                DragDropOverlay {}
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
                    on_close: move |_| close_context_menu(),
                }
            }
        }
    }
}

/// Handle dropped files/directories - opens markdown files or sets directory as root
async fn handle_dropped_files(evt: Event<DragData>, mut state: AppState) {
    let files = evt.files();
    if files.is_empty() {
        return;
    }

    for file_data in files {
        let path = file_data.path();

        // Resolve symlinks and canonicalize the path to handle Finder sidebar items
        let resolved_path = match std::fs::canonicalize(&path) {
            Ok(p) => {
                tracing::info!("Resolved path: {:?} -> {:?}", path, p);
                p
            }
            Err(e) => {
                tracing::warn!("Failed to canonicalize path {:?}: {}", path, e);
                path.clone()
            }
        };

        tracing::info!(
            "Processing dropped path: {:?}, is_dir: {}",
            resolved_path,
            resolved_path.is_dir()
        );

        if resolved_path.is_dir() {
            // If it's a directory, set it as root and show the sidebar
            tracing::info!("Setting dropped directory as root: {:?}", resolved_path);
            state.set_root_directory(resolved_path);
            // Show the sidebar if it's hidden so users can see the directory tree
            if !state.sidebar.read().open {
                state.toggle_sidebar();
            }
        } else {
            // Open any file (not just markdown)
            tracing::info!("Opening dropped file: {:?}", resolved_path);
            state.open_file(resolved_path);
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

/// Setup listeners for cross-window file/directory open events (from sidebar context menu)
fn setup_cross_window_open_listeners(mut state: AppState) {
    let current_window_id = window().id();

    // Listen for "Open in Window" file events
    use_future(move || async move {
        let mut rx = OPEN_FILE_IN_WINDOW.subscribe();

        while let Ok((target_window_id, path)) = rx.recv().await {
            // Only handle if this window is the target
            if target_window_id == current_window_id {
                tracing::info!(?path, "Opening file from cross-window request");
                state.open_file(path);
            }
        }
    });

    // Listen for "Open in Window" directory events
    use_future(move || async move {
        let mut rx = OPEN_DIRECTORY_IN_WINDOW.subscribe();

        while let Ok((target_window_id, path)) = rx.recv().await {
            // Only handle if this window is the target
            if target_window_id == current_window_id {
                tracing::info!(?path, "Opening directory from cross-window request");
                state.set_root_directory(path.clone());
                // Show the sidebar if it's hidden
                if !state.sidebar.read().open {
                    state.toggle_sidebar();
                }
            }
        }
    });
}

#[component]
fn DragDropOverlay() -> Element {
    rsx! {
        div {
            class: "drag-drop-overlay",
            div {
                class: "drag-drop-content",
                div {
                    class: "drag-drop-icon",
                    Icon { name: IconName::FileUpload, size: 64 }
                }
                div {
                    class: "drag-drop-text",
                    "Drop Markdown file or directory to open"
                }
            }
        }
    }
}

// ============================================================================
// DeviceEvent-based Tab Drag Handlers
// ============================================================================

/// Debounce delay before creating preview window (prevents flickering)
const DETACH_DEBOUNCE_MS: u64 = 50;

/// Handle mouse motion during tab drag (called from DeviceEvent handler)
///
/// Unified architecture: all windows are equal potential targets.
/// DetachState tracks whether cursor is in a tab bar (None) or detached (preview window).
/// target_window_id in GlobalActiveDrag tracks which window the cursor is in.
fn handle_drag_mouse_motion(state: AppState) {
    use crate::events::{ActiveDragUpdate, ACTIVE_DRAG_UPDATE};
    use crate::window;

    // Get current mouse position
    let (screen_x, screen_y) = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x as f64, y as f64),
        Mouse::Error => {
            tracing::debug!("Failed to get mouse position during active drag");
            return;
        }
    };

    let Some(active) = drag::get_active_drag() else {
        return;
    };

    // Two-phase hit testing for stable drag behavior:
    // Phase 1: Find which window cursor is over → update focus
    // Phase 2: Check if cursor is in focused window's tab bar → determine drag target

    let exclude_preview = window::get_preview_window_id();
    let is_single_tab = active.source_tab_count == 1;
    let source_window_id = drag::get_dragged_tab().map(|d| d.source_window_id);

    // Phase 1: Window hit test (ignore tab bar, just find window under cursor)
    // Prioritize currently focused window in overlapping areas for stable behavior
    let current_focus = window::main::get_last_focused_window();
    let window_under_cursor =
        drag::find_window_at_point(screen_x, screen_y, current_focus, exclude_preview);

    // Update focus if cursor moved to a different window
    // If no last focused window exists, focus the window under cursor
    if let Some(wid) = window_under_cursor {
        let last_focused = window::main::get_last_focused_window();
        if last_focused.is_none() || last_focused != Some(wid) {
            window::main::focus_window(wid);
        }
    }

    // Phase 2: Check if cursor is in the focused window's tab bar
    let focused_window = window::main::get_last_focused_window();
    let target_window = focused_window.and_then(|wid| {
        // For single-tab, skip source window (no local reordering possible)
        if is_single_tab && source_window_id == Some(wid) {
            return None;
        }
        // Exclude preview window from being a target
        if exclude_preview == Some(wid) {
            return None;
        }
        // Check if cursor is in this window's tab bar
        if drag::is_point_in_window_tab_bar(wid, screen_x, screen_y) {
            Some(wid)
        } else {
            None
        }
    });

    // Handle state transitions based on current state and target
    // Note: Focus is already handled in Phase 1, so no focus calls needed here
    let preview_position = calculate_window_position(&active, screen_x, screen_y);
    let transition = compute_detach_transition(&active, target_window, preview_position, &state);
    let new_detach_state = transition.new_state;
    let new_target_window_id = transition.target_window;

    // Calculate target_index if cursor is in a tab bar
    let new_target_index = new_target_window_id
        .and_then(|wid| drag::calculate_target_index_from_screen(wid, screen_x));

    // Optimization: Check if anything significant changed before broadcasting
    // This reduces unnecessary re-renders during drag
    let state_changed = active.target_window_id != new_target_window_id
        || active.detach_state != new_detach_state
        || new_target_index.is_some_and(|idx| idx != active.target_index)
        || (screen_x - active.screen_x).abs() > 1.0
        || (screen_y - active.screen_y).abs() > 1.0;

    // Update global state
    drag::update_active_drag(|d| {
        d.screen_x = screen_x;
        d.screen_y = screen_y;
        d.target_window_id = new_target_window_id;
        d.detach_state = new_detach_state;
        if let Some(idx) = new_target_index {
            d.target_index = idx;
        }
    });

    // Only notify windows when state actually changed (reduces re-renders)
    if state_changed {
        ACTIVE_DRAG_UPDATE.send(ActiveDragUpdate).ok();
    }
}

/// Handle mouse release during tab drag (called from DeviceEvent handler)
///
/// Unified architecture: drop inserts tab into target_window_id (if set) or commits preview.
///
/// State transitions:
/// - `DetachState::None` → Tab is in a window's tab bar, insert there
/// - `DetachState::Pending` → Drag cancelled during debounce, restore tab
/// - `DetachState::Creating` → Window creation in progress, cancel and restore
/// - `DetachState::Detached` → Preview visible, commit as new window
fn handle_drag_mouse_release(mut state: AppState) {
    use crate::drag::DetachState;
    use crate::events::{ActiveDragUpdate, ACTIVE_DRAG_UPDATE};

    let Some(active) = drag::get_active_drag() else {
        return;
    };

    let is_single_tab = active.source_tab_count == 1;

    match &active.detach_state {
        DetachState::None => {
            handle_drop_in_tab_bar(&mut state, &active, is_single_tab);
        }
        DetachState::Pending { .. } => {
            handle_drop_during_pending(&mut state, is_single_tab);
        }
        DetachState::Creating => {
            handle_drop_during_creating(&mut state, is_single_tab);
        }
        DetachState::Detached { .. } => {
            handle_drop_when_detached();
        }
    }

    // Notify all windows to clear drag UI
    ACTIVE_DRAG_UPDATE.send(ActiveDragUpdate).ok();

    // Clean up global drag state
    drag::end_active_drag();
    drag::end_drag();
}

/// Handle drop when cursor is in a window's tab bar (DetachState::None)
fn handle_drop_in_tab_bar(
    state: &mut AppState,
    active: &drag::GlobalActiveDrag,
    is_single_tab: bool,
) {
    let (Some(target_wid), Some(dragged)) = (active.target_window_id, drag::get_dragged_tab())
    else {
        return;
    };

    let current_wid = window().id();

    if target_wid == current_wid {
        // Drop in current window (source)
        if is_single_tab {
            // Single-tab: tab already there, just restore window state
            crate::window::close_preview_window();
        } else {
            // Multi-tab: insert and switch to the tab
            let insert_index = state.insert_tab(dragged.tab, active.target_index);
            state.switch_to_tab(insert_index);
            crate::window::close_preview_window();
        }
    } else {
        // Drop in another window - send transfer request
        crate::events::TRANSFER_TAB_TO_WINDOW
            .send((target_wid, Some(active.target_index), dragged.tab))
            .ok();
        crate::window::main::focus_window(target_wid);

        if is_single_tab {
            // Single-tab: discard preview and close source window
            // (tab was never removed, so closing window cleans up)
            crate::window::discard_preview_window();
            window().close();
        } else {
            // Multi-tab: just close preview window
            crate::window::close_preview_window();
        }
    }
}

/// Handle drop during debounce period (DetachState::Pending)
///
/// Restore tab to source window since no preview was created.
fn handle_drop_during_pending(state: &mut AppState, is_single_tab: bool) {
    if is_single_tab {
        // Single-tab: tab was never removed, nothing to restore
        return;
    }

    // Multi-tab: insert tab back to source position
    if let Some(dragged) = drag::get_dragged_tab() {
        state.insert_tab(dragged.tab, dragged.source_index);
    }
}

/// Handle drop during window creation (DetachState::Creating)
///
/// Cancel the creation and restore tab to source window.
fn handle_drop_during_creating(state: &mut AppState, is_single_tab: bool) {
    crate::window::close_preview_window();

    if !is_single_tab {
        // Multi-tab: insert tab back to source position
        if let Some(dragged) = drag::get_dragged_tab() {
            state.insert_tab(dragged.tab, dragged.source_index);
        }
    }
    // Single-tab: tab already in window, close_preview_window restored window state
}

/// Handle drop when preview window is visible (DetachState::Detached)
///
/// Commit the preview window as a permanent new window.
fn handle_drop_when_detached() {
    if let Some(preview_window_id) = crate::window::commit_preview_window() {
        crate::window::main::focus_window(preview_window_id);
    }
}

/// Calculate window position from screen coordinates and offsets
fn calculate_window_position(
    active: &drag::GlobalActiveDrag,
    screen_x: f64,
    screen_y: f64,
) -> LogicalPosition<i32> {
    let chrome = crate::window::get_chrome_inset();

    // Get tab bar position from source window
    let tab_bar = drag::get_dragged_tab()
        .and_then(|d| crate::components::tab::get_tab_bar_bounds(d.source_window_id))
        .map(|b| crate::window::Offset::new(b.left, b.top))
        .unwrap_or(crate::window::Offset::ZERO);

    // Position window so cursor stays at same relative position within tab
    let grab = &active.grab_offset;
    LogicalPosition::new(
        (screen_x - chrome.x - tab_bar.x - grab.x) as i32,
        (screen_y - chrome.y - tab_bar.y - grab.y) as i32,
    )
}

/// Result of a detach state transition
struct DetachTransition {
    new_state: drag::DetachState,
    target_window: Option<dioxus::desktop::tao::window::WindowId>,
}

/// Handle detach state transitions during drag
///
/// This function encapsulates the state machine logic for transitioning between:
/// - None (cursor in tab bar)
/// - Pending (waiting for debounce)
/// - Creating (preview window being created)
/// - Detached (preview window visible)
fn compute_detach_transition(
    active: &drag::GlobalActiveDrag,
    target_window: Option<dioxus::desktop::tao::window::WindowId>,
    preview_position: LogicalPosition<i32>,
    state: &AppState,
) -> DetachTransition {
    use crate::drag::DetachState;
    use crate::window;
    use std::time::{Duration, Instant};

    let (new_state, new_target) = match (&active.detach_state, target_window) {
        // Any state + cursor in tab bar → transition to None (with hide/close if needed)
        (DetachState::None | DetachState::Pending { .. }, Some(wid)) => {
            (DetachState::None, Some(wid))
        }
        (DetachState::Creating, Some(wid)) => {
            window::close_preview_window();
            (DetachState::None, Some(wid))
        }
        (DetachState::Detached { .. }, Some(wid)) => {
            window::hide_preview_window();
            (DetachState::None, Some(wid))
        }

        // None + cursor outside → start pending
        (DetachState::None, None) => (
            DetachState::Pending {
                entered_at: Instant::now(),
            },
            None,
        ),

        // Pending + debounce not elapsed → stay pending
        (DetachState::Pending { entered_at }, None)
            if entered_at.elapsed() < Duration::from_millis(DETACH_DEBOUNCE_MS) =>
        {
            (active.detach_state.clone(), None)
        }

        // Pending + debounce elapsed → create or show preview
        (DetachState::Pending { .. }, None) => {
            if window::has_preview_window() {
                window::show_preview_window();
                window::update_preview_position(preview_position);
                window::get_preview_window_id()
                    .map(|id| {
                        (
                            DetachState::Detached {
                                preview_window_id: id,
                            },
                            None,
                        )
                    })
                    .unwrap_or((active.detach_state.clone(), None))
            } else if let Some(dragged) = drag::get_dragged_tab() {
                let win = dioxus::desktop::window();
                spawn_preview_window_creation(PreviewWindowParams {
                    tab: dragged.tab.clone(),
                    position: preview_position,
                    is_single_tab: active.source_tab_count == 1,
                    directory: state.sidebar.read().root_directory.clone(),
                    sidebar: state.sidebar.read().clone(),
                    theme: *state.current_theme.read(),
                    size: win.inner_size().to_logical::<u32>(win.scale_factor()),
                });
                (DetachState::Creating, None)
            } else {
                (active.detach_state.clone(), None)
            }
        }

        // Creating → wait for preview, update position when ready
        (DetachState::Creating, None) => {
            if let Some(id) = window::get_preview_window_id() {
                window::update_preview_position(preview_position);
                (
                    DetachState::Detached {
                        preview_window_id: id,
                    },
                    None,
                )
            } else {
                (DetachState::Creating, None)
            }
        }

        // Detached → update position
        (DetachState::Detached { preview_window_id }, None) => {
            window::update_preview_position(preview_position);
            (
                DetachState::Detached {
                    preview_window_id: *preview_window_id,
                },
                None,
            )
        }
    };

    DetachTransition {
        new_state,
        target_window: new_target,
    }
}

/// Parameters for spawning a preview window
struct PreviewWindowParams {
    tab: crate::state::Tab,
    position: LogicalPosition<i32>,
    is_single_tab: bool,
    directory: Option<std::path::PathBuf>,
    sidebar: crate::state::Sidebar,
    theme: crate::theme::Theme,
    size: dioxus::desktop::tao::dpi::LogicalSize<u32>,
}

/// Spawn async task to create preview window
///
/// Note: This task is not tracked/cancellable. If drag is cancelled (e.g., via Escape)
/// before completion, the window may still be created. This is safe because:
/// - `cancel_active_drag_on_escape` calls `window::close_preview_window()` which closes
///   any existing preview window regardless of when it was created
/// - The preview window checks drag state on focus events
fn spawn_preview_window_creation(params: PreviewWindowParams) {
    use crate::window::{create_preview_window, CreateMainWindowConfigParams};

    spawn(async move {
        let config = CreateMainWindowConfigParams {
            directory: params.directory,
            sidebar_open: params.sidebar.open,
            sidebar_width: params.sidebar.width,
            sidebar_show_all_files: params.sidebar.show_all_files,
            theme: params.theme,
            size: params.size,
            ..Default::default()
        };
        create_preview_window(params.tab, params.position, config, params.is_single_tab).await;
    });
}
