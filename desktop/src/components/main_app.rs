use crate::ipc::OpenEvent;
use crate::state::Tab;
use crate::window::settings;
#[cfg(not(target_os = "windows"))]
use dioxus::desktop::use_muda_event_handler;
use dioxus::desktop::{window, WindowCloseBehaviour};
use dioxus::prelude::*;
use std::path::PathBuf;

// ============================================================================
// MainApp component
// ============================================================================

/// MainApp - Component dedicated to the first window
/// Configures system event handling and WindowHides behavior
///
/// NOTE: This component should only be used for the first window launched from main.rs.
/// Additional windows should use the App component directly.
///
/// System events (Reopen, file open, IPC) are handled by the Tao event loop's
/// custom_event_handler and IPC's GCD wake callback.
/// This component only handles the initial event (first CLI path) for its own tab.
#[component]
pub fn MainApp() -> Element {
    // Configure WindowCloseBehaviour::WindowHides for first window
    use_hook(|| {
        tracing::debug!("Configuring main window with WindowHides behavior");
        window().set_close_behavior(WindowCloseBehaviour::WindowHides);

        // Set chrome inset (window frame offset) - only first call takes effect
        let win = &window().window;
        if let (Ok(inner), Ok(outer)) = (win.inner_position(), win.outer_position()) {
            crate::window::set_chrome_inset((inner.x - outer.x) as f64, (inner.y - outer.y) as f64);
        }
    });

    // Set up global menu event handling
    #[cfg(not(target_os = "windows"))]
    use_muda_event_handler(move |event| {
        crate::menu::handle_menu_event_global(event);
    });

    // Pop the first event from IPC queue (CLI path pushed by main.rs before launch)
    let first_event = crate::ipc::try_pop_first_event();
    let persisted = crate::state::PersistedState::load();
    if first_event.is_some() {
        tracing::debug!(?first_event, "Received initial open event from IPC queue");
    } else {
        tracing::debug!("No initial event, will restore previous session if available");
    }

    // Resolve initial tabs and directory from event
    let is_first_window = true;
    let (tabs, active_tab, directory_override) = match &first_event {
        Some(OpenEvent::Open(request)) => {
            let tabs = if request.files.is_empty() {
                vec![Tab::default()]
            } else {
                request.files.iter().cloned().map(Tab::new).collect()
            };
            (tabs, 0, request.directory.clone())
        }
        _ if !persisted.open_files.is_empty() => {
            let tabs = persisted.restored_open_tabs();
            let active_tab = persisted.restored_active_tab();
            tracing::debug!(
                tab_count = tabs.len(),
                active_tab,
                "Restoring previous file session"
            );
            (tabs, active_tab, None)
        }
        _ => {
            let welcome_content = crate::assets::get_default_markdown_content();
            (vec![Tab::with_inline_content(welcome_content)], 0, None)
        }
    };

    // Get initial configuration values
    let directory_pref = settings::get_directory_preference(is_first_window);
    let theme_pref = settings::get_theme_preference(is_first_window);
    let position_pref = settings::get_window_position_preference(is_first_window);
    let sidebar_pref = settings::get_sidebar_preference(is_first_window);
    let right_sidebar_pref = settings::get_right_sidebar_preference(is_first_window);
    let zoom_pref = settings::get_zoom_preference(is_first_window);

    // Directory resolution: override (from event) → config → tab parent → home → root
    let directory = directory_override
        .or(directory_pref.directory)
        .or_else(|| {
            tabs.iter()
                .find_map(|tab| tab.file().and_then(|p| p.parent().map(|p| p.to_path_buf())))
        })
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));

    // Render App component with initial state
    // Subsequent system events are handled by custom_event_handler (main.rs)
    // and GCD wake callback (ipc.rs).
    rsx! {
        crate::components::app::App {
            tabs: tabs,
            active_tab: active_tab,
            directory: directory,
            theme: theme_pref.theme,
            initial_window_position: position_pref.position,
            sidebar_pinned: sidebar_pref.pinned,
            sidebar_panel: sidebar_pref.panel,
            sidebar_width: sidebar_pref.width,
            sidebar_show_all_files: sidebar_pref.show_all_files,
            sidebar_zoom_level: sidebar_pref.zoom_level,
            right_sidebar_pinned: right_sidebar_pref.pinned,
            right_sidebar_width: right_sidebar_pref.width,
            right_sidebar_panel: right_sidebar_pref.panel,
            right_sidebar_zoom_level: right_sidebar_pref.zoom_level,
            zoom_level: zoom_pref.zoom_level,
        }
    }
}
