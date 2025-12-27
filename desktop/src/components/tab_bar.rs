pub mod drag_tracking;

use dioxus::desktop::{tao::window::WindowId, window};
use dioxus::prelude::*;
use std::time::Duration;

use crate::components::icon::{Icon, IconName};
use crate::components::tab_context_menu::TabContextMenu;
use crate::events::{
    TabTransferRequest, TabTransferResponse, TAB_TRANSFER_REQUEST, TAB_TRANSFER_RESPONSE,
};
use crate::state::{AppState, TabDragState};

/// Handle tab reordering within the same window
/// Returns the new index of the moved tab, or None if no move occurred
pub fn handle_tab_reorder(
    state: &mut AppState,
    from_index: usize,
    to_index: usize,
) -> Option<usize> {
    if from_index == to_index {
        return None; // Same position - no change needed
    }

    let mut tabs = state.tabs.write();

    // Allow to_index to be tabs.len() (after the last tab)
    if from_index >= tabs.len() || to_index > tabs.len() {
        return None; // Invalid indices
    }

    // Remove tab from source position
    let tab = tabs.remove(from_index);

    // Calculate insert position (account for the removal)
    let insert_index = if from_index < to_index {
        to_index - 1
    } else {
        to_index
    };

    // Insert tab at target position
    tabs.insert(insert_index, tab);

    // Update active tab index to follow the moved tab
    let current_active = *state.active_tab.read();
    let new_active = match current_active {
        idx if idx == from_index => insert_index,
        idx if from_index < idx && idx <= insert_index => idx - 1,
        idx if insert_index <= idx && idx < from_index => idx + 1,
        idx => idx,
    };

    if new_active != current_active {
        state.active_tab.set(new_active);
    }

    Some(insert_index)
}

/// Extract display name from a tab's content
fn get_tab_display_name(tab: &crate::state::Tab) -> String {
    use crate::state::TabContent;
    match &tab.content {
        TabContent::File(path) | TabContent::FileError(path, _) => path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unnamed file".to_string()),
        TabContent::Inline(_) => "Welcome".to_string(),
        TabContent::Preferences => "Preferences".to_string(),
        TabContent::None => "No file".to_string(),
    }
}

#[component]
pub fn TabBar() -> Element {
    let state = use_context::<AppState>();
    let tabs = state.tabs.read().clone();
    let active_tab_index = *state.active_tab.read();

    // Use drag state from AppState
    let drag_state = state.tab_drag_state;

    // Read drop target once for the entire render
    let drop_target = *drag_state.read().drop_target_index.read();
    let is_animating = *drag_state.read().animating.read();

    rsx! {
        div {
            class: "tab-bar",
            class: if is_animating { "animating" },

            // Render existing tabs
            for (index, tab) in tabs.iter().enumerate() {
                // Drop indicator before this tab
                if drop_target == Some(index) {
                    div { class: "tab-drop-indicator" }
                }

                TabItem {
                    index,
                    tab: tab.clone(),
                    is_active: index == active_tab_index,
                    drag_state,
                }
            }

            // Drop indicator after the last tab (for rightmost position)
            if drop_target == Some(tabs.len()) {
                div { class: "tab-drop-indicator" }
            }

            // New tab button
            NewTabButton {}

            // Preferences button
            PreferencesButton {}
        }
    }
}

#[component]
fn TabItem(
    index: usize,
    tab: crate::state::Tab,
    is_active: bool,
    drag_state: Signal<TabDragState>,
) -> Element {
    let mut state = use_context::<AppState>();
    let tab_name = get_tab_display_name(&tab);

    // Check if this tab can be transferred
    // - Only File tabs (not None/Inline/Preferences)
    // - Not the last remaining tab (prevents empty window)
    let tabs_count = state.tabs.read().len();
    let is_transferable = matches!(
        tab.content,
        crate::state::TabContent::File(_) | crate::state::TabContent::FileError(_, _)
    ) && tabs_count > 1;

    let mut show_context_menu = use_signal(|| false);
    let mut context_menu_position = use_signal(|| (0, 0));
    let mut other_windows = use_signal(Vec::new);

    // Handle right-click to show context menu
    let handle_context_menu = move |evt: Event<MouseData>| {
        evt.prevent_default();
        let mouse_data = evt.data();
        context_menu_position.set((
            mouse_data.client_coordinates().x as i32,
            mouse_data.client_coordinates().y as i32,
        ));

        // Refresh window list
        let windows = crate::window::main::list_main_window_ids();
        let current_id = window().id();
        other_windows.set(
            windows
                .into_iter()
                .filter(|(id, _)| *id != current_id)
                .collect(),
        );

        show_context_menu.set(true);
    };

    // Handler for "Open in New Window" (simple fire-and-forget)
    let handle_open_in_new_window = move |_| {
        if let Some(tab) = state.get_tab(index) {
            let directory = state.directory.read().clone();

            spawn(async move {
                let params = crate::window::main::CreateMainWindowConfigParams {
                    directory,
                    ..Default::default()
                };
                crate::window::main::create_new_main_window(tab, params).await;
            });

            // Close tab in source window
            state.close_tab(index);
        }
        show_context_menu.set(false);
    };

    // Handler for "Move to Window" (Two-Phase Commit)
    let handle_move_to_window = move |target_id: WindowId| {
        use uuid::Uuid;

        // Phase 1: Prepare - get tab copy (don't close yet)
        if let Some(tab) = state.get_tab(index) {
            let current_directory = state.directory.read().clone();

            let request = TabTransferRequest {
                source_window_id: window().id(),
                target_window_id: target_id,
                tab: tab.clone(),
                source_directory: current_directory,
                request_id: Uuid::new_v4(),
            };

            // Wait for response (spawned task)
            let request_id = request.request_id;

            spawn(async move {
                // Subscribe BEFORE sending request to avoid race condition
                let mut rx = TAB_TRANSFER_RESPONSE.subscribe();

                // Send prepare request AFTER subscribing
                if TAB_TRANSFER_REQUEST.send(request.clone()).is_err() {
                    tracing::error!("Failed to send tab transfer request");
                    return;
                }

                tracing::debug!(?request_id, tab_index = index, "Sent tab transfer request");

                let timeout = tokio::time::sleep(Duration::from_secs(3));
                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        // Timeout - rollback
                        _ = &mut timeout => {
                            tracing::warn!(?request_id, "Tab transfer timeout, rolling back");
                            break;
                        }
                        // Receive response
                        Ok(response) = rx.recv() => {
                            tracing::debug!(?response, ?request_id, "Received tab transfer response");
                            match response {
                                TabTransferResponse::Ack { request_id: id, .. } if id == request_id => {
                                    // Phase 2: Commit - close tab (remove from source)
                                    tracing::info!(?request_id, tab_index = index, "Closing tab in source window");
                                    state.close_tab(index);
                                    tracing::info!(?request_id, "Tab transferred successfully");
                                    break;
                                }
                                TabTransferResponse::Nack { request_id: id, reason, .. } if id == request_id => {
                                    // Phase 2: Rollback (tab remains in source)
                                    tracing::warn!(?request_id, %reason, "Tab transfer rejected");
                                    break;
                                }
                                _ => {
                                    tracing::debug!(?response, ?request_id, "Ignoring unrelated response");
                                    continue;
                                }
                            }
                        }
                    }
                }
            });
        }
        show_context_menu.set(false);
    };

    // Check if this tab is being dragged
    let is_dragging = drag_state
        .read()
        .dragging_tab_index
        .read()
        .is_some_and(|i| i == index);

    rsx! {
        div {
            class: "tab",
            class: if is_active { "active" },
            class: if is_dragging { "dragging" },
            draggable: "{is_transferable}",
            onclick: move |_| {
                state.switch_to_tab(index);
            },
            oncontextmenu: handle_context_menu,

            // Drag event handlers
            ondragstart: move |evt| {
                // Record mouse offset within the tab element
                let offset_x = evt.data().element_coordinates().x;
                let offset_y = evt.data().element_coordinates().y;

                // Set global drag state
                drag_tracking::start_tab_drag(window().id(), index, offset_x, offset_y);

                // Set local drag state
                drag_state.write().start_drag(index);
            },

            ondragend: move |evt| {
                // Check if the tab was dropped in-window (set by app.rs ondrop)
                if let Some(dragged) = drag_tracking::get_dragged_tab() {
                    if !drag_tracking::was_dropped_in_window() {
                        // Not dropped in-window: create a new window at cursor position
                        let screen_x = evt.data().screen_coordinates().x;
                        let screen_y = evt.data().screen_coordinates().y;

                        if let Some(tab) = state.get_tab(dragged.source_tab_index) {
                            let directory = state.directory.read().clone();
                            let source_tab_index = dragged.source_tab_index;

                            spawn(async move {
                                // Position window at cursor (subtract offset for accurate placement)
                                let params = crate::window::main::CreateMainWindowConfigParams {
                                    directory,
                                    position: dioxus::desktop::tao::dpi::LogicalPosition::new(
                                        (screen_x - dragged.offset_x).round() as i32,
                                        (screen_y - dragged.offset_y).round() as i32,
                                    ),
                                    skip_position_shift: true,
                                    ..Default::default()
                                };

                                // Create window first, then close source tab
                                crate::window::main::create_new_main_window(tab, params).await;
                                state.close_tab(source_tab_index);
                            });
                        }
                    }
                }

                drag_tracking::end_tab_drag();
                drag_state.write().end_drag();
            },

            span {
                class: "tab-name",
                "{tab_name}"
            }

            button {
                class: "tab-close",
                onclick: move |evt| {
                    evt.stop_propagation();
                    state.close_tab(index);
                },
                Icon { name: IconName::Close, size: 14 }
            }
        }

        if *show_context_menu.read() {
            TabContextMenu {
                position: *context_menu_position.read(),
                on_close: move |_| show_context_menu.set(false),
                on_open_in_new_window: handle_open_in_new_window,
                on_move_to_window: handle_move_to_window,
                other_windows: other_windows.read().clone(),
                disabled: !is_transferable,
            }
        }
    }
}

#[component]
fn NewTabButton() -> Element {
    let mut state = use_context::<AppState>();

    rsx! {
        button {
            class: "tab-new",
            onclick: move |_| {
                state.add_empty_tab(true);
            },
            Icon { name: IconName::Add, size: 16 }
        }
    }
}

#[component]
fn PreferencesButton() -> Element {
    let mut state = use_context::<AppState>();
    let current_tab = state.current_tab();
    let is_preferences_active = current_tab
        .as_ref()
        .is_some_and(|tab| matches!(tab.content, crate::state::TabContent::Preferences));

    rsx! {
        button {
            class: "tab-preferences",
            class: if is_preferences_active { "active" },
            title: "Preferences",
            onclick: move |_| {
                state.toggle_preferences();
            },
            Icon { name: IconName::Gear, size: 16 }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::state::Tab;
    use std::path::PathBuf;

    // Helper function to test tab reordering logic without Dioxus runtime
    fn test_reorder_logic(
        tabs: &mut Vec<Tab>,
        active_tab: usize,
        from_index: usize,
        to_index: usize,
    ) -> usize {
        if from_index == to_index || from_index >= tabs.len() || to_index > tabs.len() {
            return active_tab;
        }

        let tab = tabs.remove(from_index);
        let insert_index = if from_index < to_index {
            to_index - 1
        } else {
            to_index
        };
        tabs.insert(insert_index, tab);

        // Calculate new active index
        match active_tab {
            idx if idx == from_index => insert_index,
            idx if from_index < idx && idx <= insert_index => idx - 1,
            idx if insert_index <= idx && idx < from_index => idx + 1,
            idx => idx,
        }
    }

    #[test]
    fn test_handle_tab_reorder_basic() {
        let mut tabs = vec![
            Tab::new(PathBuf::from("/a.md")),
            Tab::new(PathBuf::from("/b.md")),
            Tab::new(PathBuf::from("/c.md")),
        ];

        // Move tab from index 0 to index 2
        test_reorder_logic(&mut tabs, 1, 0, 2);

        // After removing index 0: [b, c]
        // Insert at index (2-1=1): [b, a] -> wait, insert(1) should be after b
        // Actually: [b, c] -> insert(1, a) -> [b, a, c]
        assert_eq!(
            tabs[0].file().unwrap().to_str(),
            Some("/b.md"),
            "First tab should be /b.md"
        );
        assert_eq!(
            tabs[1].file().unwrap().to_str(),
            Some("/a.md"),
            "Second tab should be /a.md"
        );
        assert_eq!(
            tabs[2].file().unwrap().to_str(),
            Some("/c.md"),
            "Third tab should be /c.md"
        );
    }

    #[test]
    fn test_handle_tab_reorder_same_position() {
        let mut tabs = vec![Tab::new(PathBuf::from("/a.md"))];

        test_reorder_logic(&mut tabs, 0, 0, 0);

        // No change
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].file().unwrap().to_str(), Some("/a.md"));
    }

    #[test]
    fn test_handle_tab_reorder_preserves_active() {
        let mut tabs = vec![
            Tab::new(PathBuf::from("/a.md")),
            Tab::new(PathBuf::from("/b.md")),
            Tab::new(PathBuf::from("/c.md")),
        ];
        let active_tab = 2;

        // Move tab from index 0 to index 1
        let new_active = test_reorder_logic(&mut tabs, active_tab, 0, 1);

        // Active tab (index 2) is not affected by reorder between 0 and 1
        // After reorder: [b, a, c]
        // active_tab=2 stays at 2
        assert_eq!(new_active, 2);
    }

    #[test]
    fn test_handle_tab_reorder_active_tab_moved() {
        let mut tabs = vec![
            Tab::new(PathBuf::from("/a.md")),
            Tab::new(PathBuf::from("/b.md")),
            Tab::new(PathBuf::from("/c.md")),
        ];
        let active_tab = 0; // Active is /a.md

        let new_active = test_reorder_logic(&mut tabs, active_tab, 0, 2);

        // Active tab should follow the moved tab to index 1 (after adjustment)
        assert_eq!(new_active, 1);
    }

    #[test]
    fn test_handle_tab_reorder_backward() {
        let mut tabs = vec![
            Tab::new(PathBuf::from("/a.md")),
            Tab::new(PathBuf::from("/b.md")),
            Tab::new(PathBuf::from("/c.md")),
        ];

        test_reorder_logic(&mut tabs, 0, 2, 0);

        assert_eq!(
            tabs[0].file().unwrap().to_str(),
            Some("/c.md"),
            "First tab should be /c.md"
        );
        assert_eq!(
            tabs[1].file().unwrap().to_str(),
            Some("/a.md"),
            "Second tab should be /a.md"
        );
        assert_eq!(
            tabs[2].file().unwrap().to_str(),
            Some("/b.md"),
            "Third tab should be /b.md"
        );
    }

    #[test]
    fn test_handle_tab_reorder_invalid_indices() {
        let mut tabs = vec![
            Tab::new(PathBuf::from("/a.md")),
            Tab::new(PathBuf::from("/b.md")),
        ];

        // Invalid from_index
        test_reorder_logic(&mut tabs, 0, 5, 0);
        assert_eq!(tabs.len(), 2);

        // Invalid to_index
        test_reorder_logic(&mut tabs, 0, 0, 5);
        assert_eq!(tabs.len(), 2);
    }
}
