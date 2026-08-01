use dioxus::desktop::{tao::window::WindowId, window};
use dioxus::prelude::*;

use super::calculations::{calculate_grab_offset, is_tab_transferable};
use super::context_menu::TabContextMenu;
use super::tab_bar::PendingDrag;
use crate::components::icon::{Icon, IconName};
use crate::drag;
use crate::state::AppState;
use crate::utils::file_operations;

#[component]
pub fn TabItem(
    index: usize,
    tab: crate::state::Tab,
    is_active: bool,
    shift_class: Option<&'static str>,
    on_drag_start: EventHandler<PendingDrag>,
    /// Delivers the precise grab offset once the tab's client rect resolves.
    /// See `handle_pointerdown` for why this cannot be awaited inline.
    on_grab_offset_refined: EventHandler<crate::window::Offset>,
) -> Element {
    let mut state = use_context::<AppState>();
    let tab_name = tab.display_name();
    let transferable = is_tab_transferable(&tab.content);
    let file_path = tab.file().map(|p| p.to_path_buf());
    let is_pinned = tab.pinned;

    let mut show_tooltip = use_signal(|| false);
    let mut tooltip_position = use_signal(|| (0i32, 0i32));
    let mut is_hovered = use_signal(|| false);

    let mut show_context_menu = use_signal(|| false);
    let mut context_menu_position = use_signal(|| (0, 0));
    let mut other_windows = use_signal(Vec::new);

    // Store mounted element for accurate grab_offset calculation
    let mut tab_element: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);

    // Handle pointer down for drag initiation
    // Uses PointerData for setPointerCapture compatibility (window-external drag)
    //
    // The pending drag MUST be registered synchronously. `get_client_rect()` is an
    // async round trip through the webview IPC bridge, and awaiting it before
    // registering meant every `pointermove` that arrived in the meantime was
    // ignored. On Windows that round trip is slow enough to outlast the whole
    // gesture: the drag was registered from a stale position only after the user
    // had already released the button, so it started on mouse-up and then never
    // ended (the release that would have ended it was already gone). macOS hides
    // this because the round trip resolves fast enough to win the race.
    //
    // So: register immediately with the offset within the event target, then
    // refine it once the exact element rect arrives.
    let handle_pointerdown = move |evt: Event<PointerData>| {
        // Only start drag on left button
        if evt.data().trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
            return;
        }

        let pointer_id = evt.data().pointer_id();
        let client_coords = evt.client_coordinates();
        let element_coords = evt.element_coordinates();

        on_drag_start.call(PendingDrag {
            index,
            start_x: client_coords.x,
            start_y: client_coords.y,
            grab_offset: crate::window::Offset::new(element_coords.x, element_coords.y),
            pointer_id,
        });

        // `element_coordinates()` is relative to whatever child node the pointer
        // landed on (label, icon, close button), so refine it against the tab
        // element itself as soon as the rect is available.
        let mounted_data = tab_element.read().clone();
        spawn(async move {
            let Some(mounted) = mounted_data else {
                return;
            };
            let Ok(rect) = mounted.get_client_rect().await else {
                return;
            };
            let (grab_x, grab_y) = calculate_grab_offset(
                client_coords.x,
                client_coords.y,
                rect.origin.x,
                rect.origin.y,
            );
            on_grab_offset_refined.call(crate::window::Offset::new(grab_x, grab_y));
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

    // Handler for "Close"
    let handle_close_tab = move |_| {
        state.close_tab(index);
        show_context_menu.set(false);
    };

    // Handler for "Close Others"
    let handle_close_others = move |_| {
        state.close_others(index);
        show_context_menu.set(false);
    };

    // Handler for "Close All"
    let handle_close_all = move |_| {
        state.close_all_unpinned();
        show_context_menu.set(false);
    };

    // Handler for "Pin Tab" / "Unpin Tab"
    let handle_toggle_pin = move |_| {
        state.toggle_pin(index);
        show_context_menu.set(false);
    };

    // Handler for "Open in New Window"
    // Create new window first, then close tab (in case it's the last tab)
    let handle_open_in_new_window = move |_| {
        if let Some(tab) = state.get_tab(index) {
            let directory = state.sidebar.read().root_directory.clone();

            spawn(async move {
                let params = crate::window::main::CreateMainWindowConfigParams {
                    directory,
                    ..Default::default()
                };
                crate::window::main::create_main_window(tab, params).await;

                // Close tab in source window after new window is created
                state.close_tab(index);
            });
        }
        show_context_menu.set(false);
    };

    // Handler for "Copy File Path"
    let handle_copy_path = {
        let file_path = file_path.clone();
        move |_| {
            if let Some(ref path) = file_path {
                crate::utils::clipboard::copy_text(path.to_string_lossy());
            }
            show_context_menu.set(false);
        }
    };

    // Handler for "Reload"
    let handle_reload = move |_| {
        state.reload_current_tab();
        show_context_menu.set(false);
    };

    // Handler for "Set Parent as Root"
    let handle_set_parent_as_root = {
        let file_path = file_path.clone();
        move |_| {
            if let Some(ref path) = file_path {
                if let Some(parent) = path.parent() {
                    state.set_root_directory(parent.to_path_buf());
                }
            }
            show_context_menu.set(false);
        }
    };

    // Handler for "Reveal in Finder"
    let handle_reveal_in_finder = {
        let file_path = file_path.clone();
        move |_| {
            if let Some(ref path) = file_path {
                file_operations::reveal_in_finder(path);
            }
            show_context_menu.set(false);
        }
    };

    // Handler for "Move to Window"
    // Uses TRANSFER_TAB_TO_WINDOW to preserve tab history when moving between windows
    let handle_move_to_window = move |target_id: WindowId| {
        if let Some(tab) = state.get_tab(index) {
            // Send tab transfer request to target window (preserves history)
            if crate::events::TRANSFER_TAB_TO_WINDOW
                .send((target_id, None, tab.clone()))
                .is_err()
            {
                tracing::warn!(
                    ?target_id,
                    "Failed to move tab: target window may be closed"
                );
                show_context_menu.set(false);
                return;
            }
            // Close tab in source window
            state.close_tab(index);
            // Focus the target window
            crate::window::main::focus_window(target_id);
            tracing::info!(?target_id, "Moved tab to window");
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
            class: if is_pinned { "pinned" },
            onpointerdown: handle_pointerdown,
            onclick: move |_| {
                // Only switch tab if not in a drag operation
                if !drag::is_tab_dragging() {
                    state.switch_to_tab(index);
                }
            },
            oncontextmenu: handle_context_menu,
            onmouseenter: move |_| {
                is_hovered.set(true);
                spawn(async move {
                    let mounted_data = tab_element.read().clone();
                    if let Some(ref mounted) = mounted_data {
                        if let Ok(rect) = mounted.get_client_rect().await {
                            // Only show tooltip if still hovered after async operation
                            if *is_hovered.read() {
                                let x = (rect.origin.x + rect.size.width / 2.0) as i32;
                                let y = (rect.origin.y + rect.size.height) as i32 + 4;
                                tooltip_position.set((x, y));
                                show_tooltip.set(true);
                            }
                        }
                    }
                });
            },
            onmouseleave: move |_| {
                is_hovered.set(false);
                show_tooltip.set(false);
            },
            onmounted: move |evt| {
                // Store mounted data for accurate grab_offset calculation
                tab_element.set(Some(evt.data()));
            },

            // Pin indicator
            if is_pinned {
                Icon { name: IconName::Pin, size: 12, class: "tab-pin-icon" }
            }

            span {
                class: "tab-name",
                "{tab_name}"
            }

            // Close button (hidden for pinned tabs)
            if !is_pinned {
                button {
                    class: "tab-close",
                    onclick: move |evt| {
                        evt.stop_propagation();
                        state.close_tab(index);
                    },
                    Icon { name: IconName::Close, size: 14 }
                }
            }
        }

        // Tooltip (position: fixed to escape tab-bar overflow clipping)
        if *show_tooltip.read() && !*show_context_menu.read() {
            {
                let (tx, ty) = *tooltip_position.read();
                rsx! {
                    div {
                        class: "tab-tooltip",
                        style: "left: {tx}px; top: {ty}px;",
                        "{tab_name}"
                    }
                }
            }
        }

        if *show_context_menu.read() {
            TabContextMenu {
                position: *context_menu_position.read(),
                file_path: file_path.clone(),
                is_pinned: is_pinned,
                on_close: move |_| show_context_menu.set(false),
                on_close_tab: handle_close_tab,
                on_close_others: handle_close_others,
                on_close_all: handle_close_all,
                on_toggle_pin: handle_toggle_pin,
                on_copy_path: handle_copy_path,
                on_reload: handle_reload,
                on_set_parent_as_root: handle_set_parent_as_root,
                on_open_in_new_window: handle_open_in_new_window,
                on_move_to_window: handle_move_to_window,
                on_reveal_in_finder: handle_reveal_in_finder,
                other_windows: other_windows.read().clone(),
                disabled: !transferable,
            }
        }
    }
}
