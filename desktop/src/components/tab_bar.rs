use dioxus::desktop::{tao::window::WindowId, window};
use dioxus::prelude::*;
use std::time::Duration;

use crate::components::icon::{Icon, IconName};
use crate::components::tab_context_menu::TabContextMenu;
use crate::events::{
    TabTransferRequest, TabTransferResponse, TAB_TRANSFER_REQUEST, TAB_TRANSFER_RESPONSE,
};
use crate::state::AppState;

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

    rsx! {
        div {
            class: "tab-bar",

            // Render existing tabs
            for (index, tab) in tabs.iter().enumerate() {
                TabItem {
                    key: "{index}",
                    index,
                    tab: tab.clone(),
                    is_active: index == active_tab_index,
                }
            }

            // New tab button
            NewTabButton {}

            // Preferences button
            PreferencesButton {}
        }
    }
}

#[component]
fn TabItem(index: usize, tab: crate::state::Tab, is_active: bool) -> Element {
    let mut state = use_context::<AppState>();
    let tab_name = get_tab_display_name(&tab);

    // Check if this tab can be transferred (only File tabs, not None/Inline/Preferences)
    let is_transferable = matches!(
        tab.content,
        crate::state::TabContent::File(_) | crate::state::TabContent::FileError(_, _)
    );

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

    rsx! {
        div {
            class: "tab",
            class: if is_active { "active" },
            onclick: move |_| {
                state.switch_to_tab(index);
            },
            oncontextmenu: handle_context_menu,

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
