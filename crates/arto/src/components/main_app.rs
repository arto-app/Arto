use crate::ipc::OpenEvent;
use crate::state::{Document, PersistedState};
use crate::window::settings;
#[cfg(target_os = "macos")]
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
/// This component only handles the initial event (the first CLI path) for its
/// own document; any further paths get windows of their own.
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
    #[cfg(target_os = "macos")]
    use_muda_event_handler(move |event| {
        crate::menu::handle_menu_event_global(event);
    });

    // Keep native menu accelerators in sync with the keybinding config.
    // Runs on the main thread (required for muda menu mutation).
    #[cfg(target_os = "macos")]
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
        tracing::debug!("No initial event, the window opens on the welcome page");
    }

    // Resolve the document and directory from the event. A window reads one
    // document, so the first path named is this window's; the rest each get a
    // window of their own below.
    let is_first_window = true;
    let (document, rest, directory_override) = match &first_event {
        Some(OpenEvent::Open(request)) => {
            let mut files = request.files.iter();
            let document = files.next().map(Document::new).unwrap_or_default();
            (
                document,
                files.cloned().collect::<Vec<_>>(),
                request.directory.clone(),
            )
        }
        // Launched with nothing to read: the window opens on the welcome
        // page, the same as a window made with Cmd+N or a document put down
        // with Cmd+T. A window with nothing in it says what there is to read
        // rather than explaining that nothing is open.
        _ => (Document::default(), Vec::new(), None),
    };

    // Everything after the first path, once — a launch naming several files is
    // a request for several windows, and dropping them would lose what the
    // command line asked for.
    use_hook(move || {
        for path in rest {
            crate::window::create_main_window_sync(
                &window(),
                Document::new(path),
                crate::window::CreateMainWindowConfigParams::default(),
            );
        }
    });

    // Get initial configuration values
    let directory_pref = settings::get_directory_preference(is_first_window);
    let theme_pref = settings::get_theme_preference(is_first_window);
    let sidebar_pref = settings::get_sidebar_preference(is_first_window);
    let content_full_width = settings::get_content_full_width_preference();
    let zoom_pref = settings::get_zoom_preference(is_first_window);

    // Roots: a directory named on the command line wins; otherwise the session
    // being restored, if the configuration asks for one; otherwise the parent
    // of the file being opened; and failing all of those, the folder the last
    // window was in.
    //
    // That last fallback is what gives a window opening on the welcome page a
    // current directory to show. A configuration that names no default is a
    // question nobody answered, not an answer of "nowhere" — and the folder
    // the reader was last in is the only non-arbitrary place to start. It is
    // still never the home directory: unasked-for, that would only be a scan
    // wide enough to make macOS ask about every folder in it.
    let temps: Vec<_> = match directory_override {
        Some(directory) => vec![directory],
        None => {
            let restored = settings::get_startup_roots();
            if restored.is_empty() {
                crate::window::main::resolve_directory(directory_pref.directory, &document)
                    .or_else(|| PersistedState::load().directory)
                    .into_iter()
                    .collect()
            } else {
                restored
            }
        }
    };

    // Render App component with initial state
    // Subsequent system events are handled by custom_event_handler (main.rs)
    // and GCD wake callback (ipc.rs).
    rsx! {
        crate::components::app::App {
            document: document,
            temps: temps,
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
