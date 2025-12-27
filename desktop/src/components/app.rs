use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::event::{Event as TaoEvent, WindowEvent};
use dioxus::desktop::{use_muda_event_handler, use_wry_event_handler, window};
use dioxus::document;
use dioxus::html::HasFileData;
use dioxus::prelude::*;
use dioxus_core::use_drop;
use std::path::PathBuf;
use std::time::Duration;

use super::content::Content;
use super::header::Header;
use super::icon::{Icon, IconName};
use super::sidebar::Sidebar;
use super::tab_bar::drag_tracking;
use super::tab_bar::TabBar;
use crate::assets::MAIN_SCRIPT;
use crate::events::{DIRECTORY_OPEN_BROADCAST, FILE_OPEN_BROADCAST};
use crate::menu;
use crate::state::{AppState, PersistedState, Tab, LAST_FOCUSED_STATE};
use crate::theme::Theme;

const WINDOW_METRICS_DEBOUNCE_MS: u64 = 200;

struct DebouncedMetricsUpdater {
    pending: Signal<(LogicalPosition<i32>, LogicalSize<u32>)>,
    token: Signal<u64>,
}

impl DebouncedMetricsUpdater {
    fn schedule(&mut self, position: LogicalPosition<i32>, size: LogicalSize<u32>) {
        self.pending.set((position, size));
        let token = self.token.read().wrapping_add(1);
        self.token.set(token);
        let pending = self.pending;
        let token_signal = self.token;
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(WINDOW_METRICS_DEBOUNCE_MS)).await;
            if *token_signal.read() != token {
                return;
            }
            let (position, size) = *pending.read();
            let mut last_focused = LAST_FOCUSED_STATE.write();
            last_focused.window_position = position.into();
            last_focused.window_size = size.into();
        });
    }
}

#[component]
pub fn App(
    tab: Tab,           // Initial tab (always provided, preserves history)
    directory: PathBuf, // Directory (resolved in create_new_main_window)
    theme: Theme,       // The enum: Auto/Light/Dark
    sidebar_open: bool,
    sidebar_width: f64,
    sidebar_show_all_files: bool,
) -> Element {
    // Initialize application state with the provided tab
    let mut state = use_context_provider(|| {
        let mut app_state = AppState::default();

        // Initialize with provided tab (preserves history)
        app_state.tabs.write()[0] = tab;

        // Apply initial directory from params (resolved in create_new_main_window)
        *app_state.directory.write() = Some(directory.clone());
        // Update last focused state for "Last Focused" behavior
        LAST_FOCUSED_STATE.write().directory = Some(directory);

        // Set initial theme
        LAST_FOCUSED_STATE.write().theme = theme;

        // Apply initial sidebar settings from params
        {
            let mut sidebar = app_state.sidebar.write();
            sidebar.open = sidebar_open;
            sidebar.width = sidebar_width;
            sidebar.show_all_files = sidebar_show_all_files;
            // Update last focused state for "Last Focused" behavior
            let mut state = LAST_FOCUSED_STATE.write();
            state.sidebar_open = sidebar_open;
            state.sidebar_width = sidebar_width;
            state.sidebar_show_all_files = sidebar_show_all_files;
        }
        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        *app_state.position.write() = LogicalPosition::new(metrics.position.x, metrics.position.y);
        *app_state.size.write() = LogicalSize::new(metrics.size.width, metrics.size.height);
        app_state
    });

    let mut debounced_metrics = {
        let position = *state.position.read();
        let size = *state.size.read();
        let pending = use_signal(|| (position, size));
        let token = use_signal(|| 0_u64);
        DebouncedMetricsUpdater { pending, token }
    };
    // Track drag-and-drop hover state
    let mut is_dragging = use_signal(|| false);

    // Pre-calculate tab drop position constants (used in ondragover)
    let sidebar_width = state.sidebar.read().width;
    let sidebar_open = state.sidebar.read().open;
    let tabbar_start = if sidebar_open { sidebar_width } else { 0.0 };
    let tabbar_padding = 20.0; // .tab-bar { padding: 0 20px }
    let tabbar_content_start = tabbar_start + tabbar_padding;
    let tab_width = 120.0; // Fixed tab width estimate
    let marker_width = 88.0; // .tab-drop-indicator { min-width: 80px; margin: 0 4px }

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
                    &mut debounced_metrics,
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
                    &mut debounced_metrics,
                );
            }
        }
        _ => {}
    });

    // Listen for file open broadcasts from background process
    setup_file_open_listener(state);

    // Listen for directory open broadcasts from background process
    setup_directory_open_listener(state);

    // Update window title when active tab changes
    use_effect(move || {
        let active_index = *state.active_tab.read();
        let tabs = state.tabs.read();

        if let Some(tab) = tabs.get(active_index) {
            let title = crate::utils::window_title::generate_window_title(&tab.content);
            window().set_title(&title);
        }
    });

    // Listen for tab transfer requests (target-side handler)
    use_future(move || async move {
        let mut rx = crate::events::TAB_TRANSFER_REQUEST.subscribe();
        let current_window_id = window().id();

        while let Ok(request) = rx.recv().await {
            // Only process requests targeted to this window
            if request.target_window_id != current_window_id {
                continue;
            }

            tracing::debug!(?request, "Received tab transfer request");

            // Phase 1: Prepare - validate request
            let can_accept = {
                // Check if window still exists and is visible
                // Could add more checks here:
                // - Max tab limit
                // - Duplicate tab check
                // - etc.

                window().is_visible()
            };

            if can_accept {
                // Phase 2a: Commit - insert tab and send Ack
                let tabs_len = state.tabs.read().len();
                let insert_index = state.insert_tab(request.tab.clone(), tabs_len);
                state.switch_to_tab(insert_index);

                // Focus this window after receiving the tab
                window().set_focus();

                crate::events::TAB_TRANSFER_RESPONSE
                    .send(crate::events::TabTransferResponse::Ack {
                        request_id: request.request_id,
                        source_window_id: request.source_window_id,
                    })
                    .ok();

                tracing::info!("Tab transfer accepted and committed");
            } else {
                // Phase 2b: Rollback - send Nack
                crate::events::TAB_TRANSFER_RESPONSE
                    .send(crate::events::TabTransferResponse::Nack {
                        request_id: request.request_id,
                        source_window_id: request.source_window_id,
                        reason: "Window is not ready to accept tabs".to_string(),
                    })
                    .ok();

                tracing::warn!("Tab transfer rejected");
            }
        }
    });

    // Save state and close child windows when this window closes
    use_drop(move || {
        // Save last used state from this window
        // Read directly from state signals instead of global statics
        // to ensure we get the current window's values, not the global last-modified values
        let mut persisted = PersistedState::from(&state);
        let window_metrics = crate::window::metrics::capture_window_metrics(&window().window);
        persisted.window_position = window_metrics.position;
        persisted.window_size = window_metrics.size;
        {
            let mut last_focused = LAST_FOCUSED_STATE.write();
            last_focused.window_position = window_metrics.position;
            last_focused.window_size = window_metrics.size;
        }
        persisted.save();

        // Close child windows
        crate::window::close_child_windows_for_parent(window().id());
    });

    rsx! {
        div {
            class: "app-container",
            class: if is_dragging() { "drag-over" },
            ondragover: move |evt| {
                evt.prevent_default();

                // Handle tab dragging - calculate drop position from x coordinate
                if drag_tracking::is_tab_dragging() {
                    let x = evt.data().client_coordinates().x;
                    let tabs_len = state.tabs.read().len();
                    let current_target = *state.tab_drag_state.read().drop_target_index.read();

                    let drop_target = calculate_drop_target(
                        x,
                        tabs_len,
                        current_target,
                        tabbar_start,
                        tabbar_content_start,
                        tab_width,
                        marker_width,
                    );

                    // Only update if changed
                    if current_target != drop_target {
                        state.tab_drag_state.write().update_drop_target(drop_target);
                    }
                    return;
                }

                // Handle file drop preview
                is_dragging.set(true);
            },
            ondragleave: move |evt| {
                // Ignore tab drop during tab dragging - ondrop will handle it
                if drag_tracking::is_tab_dragging() {
                    return;
                }

                evt.prevent_default();
                is_dragging.set(false);
            },
            ondrop: move |evt| {
                evt.prevent_default();

                // Handle tab drop
                if let Some(dragged) = drag_tracking::get_dragged_tab() {
                    if dragged.source_window_id == window().id() {
                        // Same-window drop: check if we have a valid drop target
                        let drop_target = *state.tab_drag_state.read().drop_target_index.read();
                        if let Some(idx) = drop_target {
                            // Valid drop target: reorder tabs
                            if let Some(new_index) = super::tab_bar::handle_tab_reorder(&mut state, dragged.source_tab_index, idx) {
                                state.switch_to_tab(new_index);
                                state.tab_drag_state.write().trigger_animation();
                            }
                            // Mark as dropped in-window (ondragend will check this)
                            drag_tracking::mark_dropped_in_window();
                            state.tab_drag_state.write().end_drag();
                            return;
                        }
                        // No drop target: dragged outside window, let ondragend handle it
                        return;
                    }
                    // Different window: let ondragend handle it
                    return;
                }

                // Handle file drop
                is_dragging.set(false);
                spawn(async move {
                    handle_dropped_files(evt, state).await;
                });
            },

            Sidebar {},

            div {
                class: "main-area",
                Header {},
                TabBar {},
                Content {},
            }

            // Drag and drop overlay
            if is_dragging() {
                DragDropOverlay {}
            }
        }
    }
}

/// Calculate tab drop target index from mouse x coordinate
///
/// Takes into account:
/// - Sidebar position and width
/// - TabBar padding
/// - Current drop marker position (to compensate for marker-induced shifts)
fn calculate_drop_target(
    x: f64,
    tabs_len: usize,
    current_marker_idx: Option<usize>,
    tabbar_start: f64,
    tabbar_content_start: f64,
    tab_width: f64,
    marker_width: f64,
) -> Option<usize> {
    // Adjust x coordinate if marker is currently displayed
    let adjusted_x = match current_marker_idx {
        Some(marker_idx) => {
            let marker_position = tabbar_content_start + (marker_idx as f64 * tab_width);
            if x >= marker_position {
                // Mouse is after the marker - subtract marker width to compensate for shift
                x - marker_width
            } else {
                x
            }
        }
        None => x,
    };

    // Calculate drop target based on adjusted x coordinate
    if adjusted_x < tabbar_start {
        // Over sidebar - drop at leftmost (index 0)
        Some(0)
    } else if adjusted_x < tabbar_content_start {
        // Over TabBar padding - drop at leftmost
        Some(0)
    } else {
        // Over TabBar content area - calculate index from tab positions
        let relative_x = adjusted_x - tabbar_content_start;
        let index = (relative_x / tab_width).floor() as usize;

        // Clamp to valid range [0, tabs_len]
        Some(index.min(tabs_len))
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
    debounced_metrics: &mut DebouncedMetricsUpdater,
) {
    if let Some(position) = position {
        *state.position.write() = position;
    }
    if let Some(size) = size {
        *state.size.write() = size;
    }
    if position.is_some() || size.is_some() {
        let latest_position = *state.position.read();
        let latest_size = *state.size.read();
        debounced_metrics.schedule(latest_position, latest_size);
    }
}

/// Setup listener for file open broadcasts from the background process
fn setup_file_open_listener(mut state: AppState) {
    use_future(move || async move {
        let mut rx = FILE_OPEN_BROADCAST.subscribe();

        while let Ok(file) = rx.recv().await {
            // Only handle in the focused window
            if window().is_focused() {
                tracing::info!("Opening file from broadcast: {:?}", file);
                state.open_file(file);
            }
        }
    });
}

/// Setup listener for directory open broadcasts from the background process
fn setup_directory_open_listener(mut state: AppState) {
    use_future(move || async move {
        let mut rx = DIRECTORY_OPEN_BROADCAST.subscribe();

        while let Ok(dir) = rx.recv().await {
            // Only handle in the focused window
            if window().is_focused() {
                tracing::info!("Opening directory from broadcast: {:?}", dir);
                state.set_root_directory(dir.clone());
                // Optionally show the sidebar if it's hidden
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
