use std::time::Duration;

use dioxus::desktop::{tao::window::WindowId, window};
use dioxus::prelude::*;

use super::calculations::{calculate_grab_offset, is_tab_transferable};
use super::context_menu::TabContextMenu;
use super::tab_bar::PendingDrag;
use crate::components::icon::{Icon, IconName};
use crate::drag;
use crate::events::{
    TabTransferRequest, TabTransferResponse, TAB_TRANSFER_REQUEST, TAB_TRANSFER_RESPONSE,
};
use crate::state::AppState;

#[component]
pub fn TabItem(
    index: usize,
    tab: crate::state::Tab,
    is_active: bool,
    shift_class: Option<&'static str>,
    on_drag_start: EventHandler<PendingDrag>,
) -> Element {
    let mut state = use_context::<AppState>();
    let tab_name = tab.display_name();
    let transferable = is_tab_transferable(&tab.content);

    let mut show_context_menu = use_signal(|| false);
    let mut context_menu_position = use_signal(|| (0, 0));
    let mut other_windows = use_signal(Vec::new);

    // Store mounted element for accurate grab_offset calculation
    let mut tab_element: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);

    // Handle pointer down for drag initiation
    // Uses PointerData for setPointerCapture compatibility (window-external drag)
    // Uses async to get accurate grab_offset via getBoundingClientRect
    let handle_pointerdown = move |evt: Event<PointerData>| async move {
        // Only start drag on left button
        if evt.data().trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
            return;
        }

        let pointer_id = evt.data().pointer_id();
        let client_coords = evt.client_coordinates();

        // Calculate grab_offset using getBoundingClientRect for accuracy
        // Clone signal data before await to avoid holding GenerationalRef across await point
        let mounted_data = tab_element.read().clone();
        let (grab_x, grab_y) = if let Some(ref mounted) = mounted_data {
            if let Ok(rect) = mounted.get_client_rect().await {
                calculate_grab_offset(
                    client_coords.x,
                    client_coords.y,
                    rect.origin.x,
                    rect.origin.y,
                )
            } else {
                // Fallback to element_coordinates
                let element_coords = evt.element_coordinates();
                (element_coords.x, element_coords.y)
            }
        } else {
            // Fallback to element_coordinates
            let element_coords = evt.element_coordinates();
            (element_coords.x, element_coords.y)
        };

        on_drag_start.call(PendingDrag {
            index,
            start_x: client_coords.x,
            start_y: client_coords.y,
            grab_offset: crate::window::Offset::new(grab_x, grab_y),
            pointer_id,
        });
    };

    // Handle right-click to show context menu
    let handle_context_menu = move |evt: Event<MouseData>| {
        evt.prevent_default();
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
    };

    // Handler for "Open in New Window"
    // Create new window first, then close tab (in case it's the last tab)
    let handle_open_in_new_window = move |_| {
        if let Some(tab) = state.get_tab(index) {
            let directory = state.directory.read().clone();

            spawn(async move {
                let params = crate::window::main::CreateMainWindowConfigParams {
                    directory,
                    ..Default::default()
                };
                crate::window::main::create_new_main_window(tab, params).await;

                // Close tab in source window after new window is created
                state.close_tab(index);
            });
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
                target_index: None, // Context menu always appends at end
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

    // Tab is always rendered normally - no placeholder needed since
    // dragged tab is removed at drag start (unified approach)
    let shift_class_str = shift_class.unwrap_or("");
    rsx! {
        div {
            class: "tab {shift_class_str}",
            class: if is_active { "active" },
            onpointerdown: handle_pointerdown,
            onclick: move |_| {
                // Only switch tab if not in a drag operation
                if !drag::is_tab_dragging() {
                    state.switch_to_tab(index);
                }
            },
            oncontextmenu: handle_context_menu,
            onmounted: move |evt| {
                // Store mounted data for accurate grab_offset calculation
                tab_element.set(Some(evt.data()));
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
                disabled: !transferable,
            }
        }
    }
}
