use crate::ipc::OpenEvent;
use crate::state::Document;
use crate::window::settings;
#[cfg(not(target_os = "windows"))]
use dioxus::desktop::use_muda_event_handler;
use dioxus::desktop::window;
#[cfg(target_os = "macos")]
use dioxus::desktop::WindowCloseBehaviour;
use dioxus::prelude::*;

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
/// This component only handles the initial event (the first CLI path) as its
/// own document.
#[component]
pub fn MainApp() -> Element {
    // Configure WindowCloseBehaviour::WindowHides for first window
    //
    // macOS only: hiding the last window models the dock lifecycle, where an app
    // outlives its windows and is reopened from the dock. Windows and Linux have
    // no such concept, so it just leaves a headless process behind - and the
    // single-instance IPC then swallows every subsequent launch into it, making
    // the app look dead. The Dioxus default (WindowCloses, exiting on last window
    // close) is the correct behaviour there.
    use_hook(|| {
        #[cfg(target_os = "macos")]
        {
            tracing::debug!("Configuring main window with WindowHides behavior");
            window().set_close_behavior(WindowCloseBehaviour::WindowHides);
        }

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

    // Keep native menu accelerators in sync with the keybinding config.
    // Runs on the main thread (required for muda menu mutation).
    #[cfg(not(target_os = "windows"))]
    use_future(move || async move {
        let mut rx = crate::config::CONFIG_CHANGED_BROADCAST.subscribe();
        while rx.recv().await.is_ok() {
            crate::menu::refresh_menu_accelerators();
        }
    });

    // Pop the first event from IPC queue (CLI path pushed by main.rs before launch)
    let first_event = crate::ipc::try_pop_first_event();
    if first_event.is_some() {
        tracing::debug!(?first_event, "Received initial open event from IPC queue");
    } else {
        tracing::debug!("No initial event, will show welcome screen");
    }

    // Resolve the document and directory from the event. A window reads one
    // document; any further files named on the command line are opened by
    // `ipc`, each in a window of its own.
    let is_first_window = true;
    let (document, directory_override) = match &first_event {
        Some(OpenEvent::Open(request)) => {
            let document = request.files.first().map(Document::new).unwrap_or_default();
            (document, request.directory.clone())
        }
        _ => {
            let welcome_content = crate::assets::get_default_markdown_content();
            (Document::with_inline_content(welcome_content), None)
        }
    };

    // Get initial configuration values
    let directory_pref = settings::get_directory_preference(is_first_window);
    let theme_pref = settings::get_theme_preference(is_first_window);
    let sidebar_pref = settings::get_sidebar_preference(is_first_window);
    let content_full_width = settings::get_content_full_width_preference();
    let zoom_pref = settings::get_zoom_preference(is_first_window);

    // Directory resolution: override (from event) → config default → parent of an
    // explicitly opened file. Stays None on a blank config with no opened file so
    // the sidebar shows its empty/welcome state instead of scanning home.
    let params_directory = directory_override.or(directory_pref.directory);
    let directory = crate::window::main::resolve_directory(params_directory, &document);

    // Render App component with initial state
    // Subsequent system events are handled by custom_event_handler (main.rs)
    // and GCD wake callback (ipc.rs).
    rsx! {
        crate::components::app::App {
            document: document,
            directory: directory,
            theme: theme_pref.theme,
            content_full_width,
            sidebar_pinned: sidebar_pref.pinned,
            sidebar_width: sidebar_pref.width,
            sidebar_show_all_files: sidebar_pref.show_all_files,
            sidebar_zoom_level: sidebar_pref.zoom_level,
            zoom_level: zoom_pref.zoom_level,
        }
    }
}
