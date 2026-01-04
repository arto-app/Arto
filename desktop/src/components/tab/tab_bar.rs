use dioxus::core::use_drop;
use dioxus::desktop::{tao::window::WindowId, window};
use dioxus::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;

use super::calculations::{calculate_shift_class, exceeds_drag_threshold};
use super::floating_tab::FloatingTab;
use super::tab_item::TabItem;
use crate::components::icon::{Icon, IconName};
use crate::drag::{self, GlobalActiveDrag};
use crate::events::ACTIVE_DRAG_UPDATE;
use crate::state::AppState;
use crate::window::Offset;

/// Drag start threshold in pixels
const DRAG_THRESHOLD: f64 = 5.0;

/// Pending drag state before threshold is reached
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingDrag {
    /// Index of the tab being dragged
    pub index: usize,
    /// Initial mouse X position (client coordinates)
    pub start_x: f64,
    /// Initial mouse Y position (client coordinates)
    pub start_y: f64,
    /// Mouse grab offset within the tab element (where user clicked)
    pub grab_offset: Offset,
    /// Pointer ID for pointer capture
    pub pointer_id: i32,
}

/// Tab bar bounds in client coordinates (relative to viewport)
#[derive(Debug, Clone, PartialEq)]
pub struct TabBarBounds {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

/// Global tab bar bounds registry
/// Maps WindowId to TabBarBounds for hit testing during drag
static TAB_BAR_BOUNDS: std::sync::LazyLock<RwLock<HashMap<WindowId, TabBarBounds>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// Get tab bar bounds for a specific window
pub fn get_tab_bar_bounds(window_id: WindowId) -> Option<TabBarBounds> {
    TAB_BAR_BOUNDS.read().get(&window_id).cloned()
}

/// Update tab bar bounds for a window
fn set_tab_bar_bounds(window_id: WindowId, tab_bar: TabBarBounds) {
    TAB_BAR_BOUNDS.write().insert(window_id, tab_bar);
}

/// Remove tab bar bounds (called when window closes)
fn unregister_tab_bar_bounds(window_id: WindowId) {
    TAB_BAR_BOUNDS.write().remove(&window_id);
}

/// Global tab count registry per window.
/// Used during drag operations to calculate target index.
static TAB_COUNTS: std::sync::LazyLock<RwLock<HashMap<WindowId, usize>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// Get tab count for a window
pub fn get_tab_count(window_id: WindowId) -> usize {
    TAB_COUNTS.read().get(&window_id).copied().unwrap_or(0)
}

/// Update tab count for a window
fn set_tab_count(window_id: WindowId, count: usize) {
    TAB_COUNTS.write().insert(window_id, count);
}

/// Remove tab count entry (called when window closes)
fn unregister_tab_count(window_id: WindowId) {
    TAB_COUNTS.write().remove(&window_id);
}

/// Cancel active drag and restore tab to source window on Escape key
fn cancel_active_drag_on_escape(
    state: &mut AppState,
    current_window_id: WindowId,
    active_drag_signal: &mut Signal<Option<GlobalActiveDrag>>,
) {
    use crate::drag::DetachState;

    if let Some(active) = drag::get_active_drag() {
        // Close preview window if visible
        if matches!(
            active.detach_state,
            DetachState::Creating | DetachState::Detached { .. }
        ) {
            crate::window::close_preview_window();
        }
    }

    // Restore tab to source window if this is the source
    if let Some(dragged) = drag::get_dragged_tab() {
        if dragged.source_window_id == current_window_id {
            state.insert_tab(dragged.tab, dragged.source_index);
        }
    }

    drag::end_active_drag();
    drag::end_drag();
    active_drag_signal.set(None);
}

/// Start global active drag after threshold is exceeded
///
/// This function handles the transition from pending to active drag state,
/// including tab removal (for multi-tab) and global state initialization.
fn start_global_drag(
    state: &mut AppState,
    pending: &PendingDrag,
    current_window_id: WindowId,
    screen_coords: (f64, f64),
    active_drag_signal: &mut Signal<Option<GlobalActiveDrag>>,
) {
    let index = pending.index;
    let tab = state.tabs.read().get(index).cloned();
    let tab_count = state.tabs.read().len();

    let Some(tab) = tab else {
        return;
    };

    let is_single_tab = tab_count == 1;

    // For single-tab windows: don't remove the tab, window itself becomes preview
    // For multi-tab windows: remove tab, all windows become potential targets
    if !is_single_tab {
        state.take_tab(index);
        // Update tab count immediately (don't wait for use_effect)
        set_tab_count(current_window_id, tab_count - 1);
    }

    // Store tab data globally (for restoration on cancel or insertion on drop)
    drag::start_tab_drag(tab, current_window_id, index);

    // Start global active drag
    // Single-tab: start detached immediately (window becomes preview)
    // Multi-tab: start in source window's tab bar
    let (initial_target, initial_detach) = if is_single_tab {
        // Single tab: immediately enter pending state to create "preview"
        // (which promotes current window instead of creating new one)
        (
            None,
            drag::DetachState::Pending {
                entered_at: std::time::Instant::now(),
            },
        )
    } else {
        // Multi-tab: cursor is in source window's tab bar
        (Some(current_window_id), drag::DetachState::None)
    };

    let new_drag = GlobalActiveDrag {
        source_index: index,
        target_window_id: initial_target,
        target_index: index,
        screen_x: screen_coords.0,
        screen_y: screen_coords.1,
        grab_offset: pending.grab_offset,
        detach_state: initial_detach,
        source_tab_count: tab_count,
    };
    drag::start_active_drag(new_drag.clone());

    // Initialize signal for immediate UI feedback
    active_drag_signal.set(Some(new_drag));

    tracing::debug!(
        index,
        tab_count,
        is_single_tab,
        screen_x = screen_coords.0,
        screen_y = screen_coords.1,
        "Started global active drag"
    );
}

/// Local pending drag state (before threshold is reached)
#[derive(Debug, Clone, Default)]
enum LocalDragState {
    #[default]
    Idle,
    Pending(PendingDrag),
}

#[component]
pub fn TabBar() -> Element {
    let mut state = use_context::<AppState>();
    let tabs = state.tabs.read().clone();
    let active_tab_index = *state.active_tab.read();

    // Local drag state - only tracks pending state before threshold
    // Once active, global ACTIVE_DRAG takes over
    let mut local_drag_state = use_signal(LocalDragState::default);

    // Current window ID
    let current_window_id = window().id();

    // Update global tab count registry when tabs change
    let tabs_signal = state.tabs;
    use_effect(move || {
        let tab_count = tabs_signal.read().len();
        set_tab_count(current_window_id, tab_count);
    });

    // Clean up registries when window closes
    use_drop(move || {
        unregister_tab_count(current_window_id);
        unregister_tab_bar_bounds(current_window_id);
    });

    // Unified drag state signal (updated via broadcast for re-rendering)
    // All windows subscribe; each checks if it's the current target
    let mut active_drag_signal = use_signal(|| None::<GlobalActiveDrag>);

    // Store tab bar element reference for bounds calculation on drag start
    let mut tab_bar_element: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);

    // Track escape key to cancel drag
    let handle_keydown = move |evt: Event<KeyboardData>| {
        if evt.key() == Key::Escape {
            // Cancel local pending state
            if matches!(*local_drag_state.read(), LocalDragState::Pending(_)) {
                local_drag_state.set(LocalDragState::Idle);
            }
            // Cancel global active drag and restore tab
            if drag::is_active_drag() {
                cancel_active_drag_on_escape(
                    &mut state,
                    current_window_id,
                    &mut active_drag_signal,
                );
            }
        }
    };

    // Subscribe to unified drag updates
    // All windows listen, refresh bounds from DOM, and update signal to trigger re-render
    use_effect(move || {
        spawn(async move {
            let mut rx = ACTIVE_DRAG_UPDATE.subscribe();
            while let Ok(_update) = rx.recv().await {
                // Refresh tab bar bounds from DOM (handles window resize)
                // Clone the element reference before await to avoid holding GenerationalRef
                let element = tab_bar_element.read().clone();
                if let Some(ref el) = element {
                    if let Ok(rect) = el.get_client_rect().await {
                        set_tab_bar_bounds(
                            current_window_id,
                            TabBarBounds {
                                left: rect.origin.x,
                                right: rect.origin.x + rect.size.width,
                                top: rect.origin.y,
                                bottom: rect.origin.y + rect.size.height,
                            },
                        );
                    }
                }
                // Update signal with current global state to trigger re-render
                active_drag_signal.set(drag::get_active_drag());
            }
        });
    });

    // Pointer move handler for Pending → Active transition only
    // Once active, DeviceEvent handlers take over (including target_index calculation)
    let handle_pointermove = move |evt: Event<PointerData>| {
        let pointer = evt.data();
        let x = pointer.client_coordinates().x;
        let y = pointer.client_coordinates().y;

        let current_state = local_drag_state.read().clone();

        if let LocalDragState::Pending(pending) = current_state {
            let dx = (x - pending.start_x).abs();
            let dy = (y - pending.start_y).abs();

            if exceeds_drag_threshold(dx, dy, DRAG_THRESHOLD) {
                let screen = pointer.screen_coordinates();
                start_global_drag(
                    &mut state,
                    &pending,
                    current_window_id,
                    (screen.x, screen.y),
                    &mut active_drag_signal,
                );
                local_drag_state.set(LocalDragState::Idle);
            }
        }
        // Note: Active drag (including target_index) is handled by DeviceEvent in App
    };

    // Pointer up handler for local pending state only
    // Active drag release is handled by DeviceEvent in App component
    let handle_pointerup = move |_evt: Event<PointerData>| {
        // Only cancel local pending state
        // Active drag is handled by DeviceEvent
        if matches!(*local_drag_state.read(), LocalDragState::Pending(_)) {
            local_drag_state.set(LocalDragState::Idle);
        }
    };

    // Get current global drag info for rendering (from signal for reactivity)
    let global_active_drag = active_drag_signal.read().clone();

    // Check if this window is the current drag target
    let is_target_window = global_active_drag
        .as_ref()
        .is_some_and(|d| d.target_window_id == Some(current_window_id));

    // Check if any drag is in progress (for CSS class)
    let is_dragging = global_active_drag.is_some() || drag::is_tab_dragging();

    // Get target_index for placeholder insertion (must be outside rsx!)
    let drag_target_index = if is_target_window {
        global_active_drag.as_ref().map(|d| d.target_index)
    } else {
        None
    };

    rsx! {
        div {
            class: "tab-bar",
            class: if is_dragging { "dragging" },
            tabindex: "0",
            onkeydown: handle_keydown,
            onpointermove: handle_pointermove,
            onpointerup: handle_pointerup,
            onmounted: move |evt| {
                // Store element reference for bounds calculation on drag start
                tab_bar_element.set(Some(evt.data()));
            },

            // Render tabs with drag support
            for (index, tab) in tabs.iter().enumerate() {
                TabItem {
                    key: "{index}",
                    index,
                    tab: tab.clone(),
                    shift_class: calculate_shift_class(is_target_window, drag_target_index, index),
                    is_active: index == active_tab_index,
                    on_drag_start: move |pending: PendingDrag| {
                        local_drag_state.set(LocalDragState::Pending(pending));
                    },
                }
            }

            // Placeholder at end (always present during drag to push [+] button)
            if drag_target_index.is_some() {
                div {
                    class: "tab placeholder",
                }
            }

            // New tab button
            NewTabButton {}

            // Preferences button
            PreferencesButton {}
        }

        // Drag overlay for capturing events (shown during any active drag)
        // DeviceEvent handles mouse tracking, but overlay helps with local pointer events
        if global_active_drag.is_some() {
            div {
                id: "drag-overlay",
                class: "drag-overlay",
                onpointermove: handle_pointermove,
                onpointerup: handle_pointerup,
            }
        }

        // Floating tab during active drag (shown in target window)
        // With unified approach: tab is removed at drag start, floating tab follows cursor
        // in whichever window's tab bar the cursor is currently in
        if is_target_window {
            if let Some(ref active_drag) = global_active_drag {
                if matches!(active_drag.detach_state, drag::DetachState::None) {
                    {
                        let tab_bar_y = get_tab_bar_bounds(current_window_id)
                            .map(|b| b.top)
                            .unwrap_or(0.0);
                        let tab_name = drag::get_dragged_tab()
                            .map(|t| t.tab.display_name())
                            .unwrap_or_default();

                        rsx! {
                            FloatingTab {
                                drag: active_drag.clone(),
                                tab_name: tab_name,
                                fixed_y: tab_bar_y,
                            }
                        }
                    }
                }
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
    use super::*;

    mod pending_drag {
        use super::*;

        #[test]
        fn default_values() {
            let pending = PendingDrag::default();
            assert_eq!(pending.index, 0);
            assert_eq!(pending.start_x, 0.0);
            assert_eq!(pending.start_y, 0.0);
            assert_eq!(pending.grab_offset.x, 0.0);
            assert_eq!(pending.grab_offset.y, 0.0);
            assert_eq!(pending.pointer_id, 0);
        }
    }
}
