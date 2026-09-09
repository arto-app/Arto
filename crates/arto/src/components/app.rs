mod drag_drop_overlay;
mod drop_handlers;
mod keybinding_engine;
mod listeners;
mod shortcut_overlay;

use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::event::{Event as TaoEvent, WindowEvent};
#[cfg(target_os = "macos")]
use dioxus::desktop::use_muda_event_handler;
use dioxus::desktop::{use_wry_event_handler, window};
use dioxus::document;
use dioxus::prelude::*;
use dioxus_core::use_drop;
use std::path::PathBuf;

use super::content::{
    close_context_menu, use_search_handler, Content, ContentContextMenu, CONTENT_CONTEXT_MENU,
};
use super::header::Header;
use super::sidebar::file_explorer::SidebarContextMenuHost;
use super::sidebar::Sidebar;
use crate::assets::main_script_url;
#[cfg(target_os = "macos")]
use crate::menu;
use crate::state::{AppState, Document, PersistedState};
use crate::theme::Theme;

use drag_drop_overlay::DragDropOverlay;
use drop_handlers::handle_dropped_files;
use keybinding_engine::setup_keybinding_engine;
use listeners::setup_window_listeners;
use shortcut_overlay::{
    build_shortcut_help_items, close_shortcut_overlay, split_shortcut_help_columns,
    ShortcutHelpOverlay, ShortcutOverlayVisibility,
};

#[component]
pub fn App(
    // The document to open in this window, if there is one.
    document: Document,
    // Temporary roots this window starts with, beside the places (resolved
    // by the window's creator or by MainApp). Empty means the tree shows the
    // places alone, rather than scanning an arbitrary directory.
    temps: Vec<PathBuf>,
    theme: Theme, // The enum: Auto/Light/Dark
    content_full_width: bool,
    sidebar_pinned: bool,
    sidebar_width: f64,
    sidebar_show_all_files: bool,
    sidebar_zoom_level: f64,
    zoom_level: f64,
) -> Element {
    // Initialize application state with the provided document
    // Taken mutably only by the native menu handler, which is built on macOS
    // alone — the other platforms draw their menu in the header, so nothing
    // there needs `mut`.
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut state = use_context_provider(|| {
        let mut app_state = AppState::new(theme);
        // A duplicated window arrives with the place its original had reached
        // recorded in the document's history; a fresh one carries the top.
        if let Some(entry) = document.history.current() {
            app_state
                .pending_scroll_anchor
                .set(Some(entry.scroll_anchor));
        }
        // A document a window is born with — from the command line, from the
        // Finder, from "Open in New Window" — was read just as much as one
        // opened later, and the history is the list of what has been read.
        if let Some(file) = document.file() {
            crate::visits::record_visit(file);
        }
        app_state.document.set(document);
        app_state.content_full_width.set(content_full_width);

        // Apply initial sidebar settings from params (including the roots)
        {
            let mut sidebar = app_state.sidebar.write();
            sidebar.roots =
                crate::roots::Roots::new(crate::bookmarks::BOOKMARKS.read().places(), temps);
            sidebar.pinned = sidebar_pinned;
            sidebar.width = sidebar_width;
            sidebar.show_all_files = sidebar_show_all_files;
        }

        // Apply initial zoom levels from params (already normalized in window::settings)
        {
            app_state.sidebar.write().zoom_level = sidebar_zoom_level;
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
                        const {{ init }} = await import("{main_script}");
                        init();
                    }} catch (error) {{
                        console.error("Failed to load main module:", error);
                    }}
                }})();
                "#,
                main_script = main_script_url()
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
    #[cfg(target_os = "macos")]
    use_muda_event_handler(move |event| {
        // Only handle state-dependent events
        menu::handle_menu_event_with_state(event, &mut state);
    });

    // Handle window events
    use_wry_event_handler(move |event, target| {
        let _ = target;

        match event {
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
            _ => {}
        }
    });

    setup_window_listeners(state);

    // Keep the window title on the document being read
    use_effect(move || {
        let title =
            crate::utils::window_title::generate_window_title(&state.document.read().content);
        window().set_title(&title);
    });

    // Save state and close child windows when this window closes
    use_drop(move || {
        let window_id = window().id();

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
    // Generation counter for auto-hide timer cancellation
    let mut left_hide_gen = use_signal(|| 0u32);
    // Track whether mouse is physically inside the overlay wrapper.
    // Used by on_resize_change to decide whether to start a hide timer:
    // if mouse is inside, onmouseleave will handle hiding naturally.
    let mut left_mouse_inside = use_signal(|| false);

    /// Grace before a peeking panel retracts.
    ///
    /// Long enough for two things, not one. The specification's 240ms covers
    /// the gap between the rail and the panel, which the pointer crosses in a
    /// moment; leaving the panel is not that — the pointer goes to the
    /// document, or off the window, or back again a second later, and a panel
    /// that closed the instant it was left would have to be asked for again
    /// every time. It shuts when the reader has plainly moved on.
    const OVERLAY_HIDE_DELAY_MS: u64 = 700;

    /// Put the peeking panel away once the grace has passed, unless something
    /// asks for it again first.
    ///
    /// Every read here peeks. This is called from an effect, and a signal read
    /// inside one subscribes the effect to it — reading the generation and then
    /// writing it would wake the effect with its own write, forever, spawning a
    /// timer each time round.
    fn retract_after_grace(mut hide_gen: Signal<u32>, mut hover_active: Signal<bool>) {
        let generation = *hide_gen.peek() + 1;
        hide_gen.set(generation);
        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(OVERLAY_HIDE_DELAY_MS)).await;
            if *hide_gen.peek() == generation {
                hover_active.set(false);
            }
        });
    }

    // A menu the panel opened holds the peek open while it is up, because the
    // pointer has to leave the panel to use it. When it closes somewhere else
    // entirely, no further mouse event is coming to send the panel back, so
    // its closing is what does.
    let menu_open = use_memo(move || state.sidebar_context_menu.read().is_some());
    use_effect(move || {
        if !menu_open() && !*left_mouse_inside.peek() {
            retract_after_grace(left_hide_gen, left_hover_active);
        }
    });

    // The width has the last word on what is drawn beside the document. The
    // panel's own choice is untouched by it, so widening the window brings a
    // panel back exactly as it was left; a panel folded with Cmd+B stays
    // folded, because that was intent rather than a consequence of width.
    let chrome = use_memo(move || state.visible_chrome());
    let rail_visible = use_memo(move || chrome().rail);
    let left_pinned = state.sidebar.read().pinned && chrome().panel;

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
            // The pointer leaving the window is the reader leaving, and a
            // peeking panel has to go with them. Nothing else can say so: the
            // panel's own `onmouseleave` never fires for a pointer that left
            // by way of the rail, and a window that is not focused gets no
            // mouse events at all — so the panel would still be out when the
            // reader came back to a window they had put away.
            onmouseleave: move |evt: Event<MouseData>| {
                if evt.data().held_buttons().contains(dioxus::html::input_data::MouseButton::Primary) {
                    return;
                }
                if state.sidebar_context_menu.read().is_some() {
                    return;
                }
                left_mouse_inside.set(false);
                retract_after_grace(left_hide_gen, left_hover_active);
            },
            ondrop: move |evt| {
                evt.prevent_default();
                is_dragging.set(false);

                spawn(async move {
                    handle_dropped_files(evt, state).await;
                });
            },

            // The rail is here whenever the window can spare 40px for it. It
            // is what switches the panel's faces and, because it is visible
            // and exists for the purpose, it is also what the pointer can
            // safely aim at to bring the panel back.
            if rail_visible() {
                crate::components::sidebar::rail::Rail {
                    on_peek: move |face| {
                        // Resting on a glyph brings that face over the
                        // document. A panel that is being held stays as it
                        // was: what is held was chosen, and a pointer passing
                        // over the rail is not a choice — it would rewrite the
                        // reader's own with nothing but a hover.
                        if !left_pinned {
                            state.sidebar.write().face = face;
                            left_hover_active.set(true);
                            left_hide_gen.set(left_hide_gen() + 1);
                        }
                    },
                }
            }

            // Left sidebar: pinned → flex layout, unpinned → overlay with animation
            if left_pinned {
                // Pinned: the panel stands beside the document. The rail is
                // what pins and unpins it, so there is no control inside.
                Sidebar {}
            }

            div {
                class: "main-area",
                Header {},
                Content {},
            }

            // Overlay wrappers (rendered when unpinned, animated via .visible class)
            if !left_pinned {
                div {
                    class: "sidebar-overlay-wrapper left",
                    // Stand beside the rail rather than over it: the marks
                    // that peeked the panel out are the ones that switch its
                    // faces and send it back, so they have to stay visible.
                    class: if rail_visible() { "beside-rail" },
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
                        // A menu opened from a row is drawn at the window's
                        // root rather than inside the panel, so the pointer
                        // moving onto it leaves the panel. It is still the
                        // panel being used: retracting it here would take the
                        // menu's subject away mid-click.
                        if state.sidebar_context_menu.read().is_some() {
                            return;
                        }
                        retract_after_grace(left_hide_gen, left_hover_active);
                    },
                    Sidebar {
                        on_resize_change: move |resizing: bool| {
                            if resizing {
                                // Cancel any pending hide timer
                                left_hide_gen.set(left_hide_gen() + 1);
                            } else if !left_mouse_inside() {
                                // Resize ended with mouse outside: start hide timer
                                retract_after_grace(left_hide_gen, left_hover_active);
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

            // Left-sidebar file-tree context menu (rendered at the app-container
            // root, outside the watcher-keyed file tree, so refresh-driven
            // remounts of the tree can no longer unmount an open menu).
            SidebarContextMenuHost {}

            // The palette sits above everything, including the panel: it is
            // opened over whatever is being read and closes back onto it.
            // Mounted only while open, so it always opens on a clear query.
            if *state.palette_open.read() {
                crate::components::palette::Palette {}
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
