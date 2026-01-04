use crate::events::{DIRECTORY_OPEN_BROADCAST, FILE_OPEN_BROADCAST};
use crate::state::Tab;
use crate::window as window_manager;
use crate::window::{settings, CreateMainWindowConfigParams};
use dioxus::core::spawn_forever;
use dioxus::desktop::use_muda_event_handler;
use dioxus::desktop::{window, WindowCloseBehaviour};
use dioxus::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::sync::mpsc::Receiver;

// ============================================================================
// OpenEvent definition
// ============================================================================

/// Open event types for distinguishing files, directories, and reopen events
/// Used to communicate between OS event handler (main.rs) and MainApp component
#[derive(Debug, Clone)]
pub enum OpenEvent {
    /// File opened from Finder/CLI
    File(PathBuf),
    /// Directory opened from Finder/CLI (should set sidebar root)
    Directory(PathBuf),
    /// App icon clicked (reopen event)
    Reopen,
}

/// A global receiver to receive open events from the main thread (OS → Dioxus context)
/// This is set once by main.rs and consumed once by this MainApp component.
pub static OPEN_EVENT_RECEIVER: Mutex<Option<Receiver<OpenEvent>>> = Mutex::new(None);

// ============================================================================
// System event handling
// ============================================================================

#[tracing::instrument]
fn handle_open_event(event: OpenEvent) {
    tracing::debug!(?event, "Handling system open event");

    match event {
        OpenEvent::File(file) => {
            if window_manager::has_any_main_windows() {
                let _ = FILE_OPEN_BROADCAST.send(file);
            } else {
                spawn(async move {
                    window_manager::create_new_main_window_with_file(
                        file,
                        CreateMainWindowConfigParams::default(),
                    )
                    .await;
                });
            }
        }
        OpenEvent::Directory(dir) => {
            if window_manager::has_any_main_windows() {
                let _ = DIRECTORY_OPEN_BROADCAST.send(dir);
            } else {
                spawn(async move {
                    let params = CreateMainWindowConfigParams {
                        directory: Some(dir),
                        ..Default::default()
                    };
                    window_manager::create_new_main_window_with_empty(params).await;
                });
            }
        }
        OpenEvent::Reopen => {
            if !window_manager::focus_last_focused_main_window() {
                spawn(async move {
                    window_manager::create_new_main_window_with_empty(
                        CreateMainWindowConfigParams::default(),
                    )
                    .await;
                });
            }
        }
    }
}

// ============================================================================
// MainApp component
// ============================================================================

/// MainApp - Component dedicated to the first window
/// Configures system event handling and WindowHides behavior
///
/// NOTE: This component should only be used for the first window launched from main.rs.
/// Additional windows should use the App component directly.
#[component]
pub fn MainApp() -> Element {
    // Configure WindowCloseBehaviour::WindowHides for first window
    use_hook(|| {
        tracing::debug!("Configuring main window with WindowHides behavior");
        window().set_close_behavior(WindowCloseBehaviour::WindowHides);

        // Register the first window in MAIN_WINDOWS list
        // This is critical for has_any_main_windows() to work correctly
        let weak_handle = std::rc::Rc::downgrade(&window());
        window_manager::register_main_window(weak_handle);

        // Set chrome inset (window frame offset) - only first call takes effect
        let win = &window().window;
        if let (Ok(inner), Ok(outer)) = (win.inner_position(), win.outer_position()) {
            window_manager::set_chrome_inset(
                (inner.x - outer.x) as f64,
                (inner.y - outer.y) as f64,
            );
        }
    });

    // Set up global menu event handling
    use_muda_event_handler(move |event| {
        crate::menu::handle_menu_event_global(event);
    });

    // Get receiver and consume initial event
    let mut rx = OPEN_EVENT_RECEIVER
        .lock()
        .expect("Failed to lock OPEN_EVENT_RECEIVER")
        .take()
        .expect("OPEN_EVENT_RECEIVER not initialized");

    // Handle initial event (file, directory, or none)
    let first_event = if let Ok(event) = rx.try_recv() {
        tracing::debug!(?event, "Received initial open event");
        Some(event)
    } else {
        tracing::debug!("No initial event, will show welcome screen");
        None
    };

    // Resolve initial tab and directory from event
    let is_first_window = true;
    let (tab, directory_override) = match &first_event {
        Some(OpenEvent::File(path)) => (Tab::new(path.clone()), None),
        Some(OpenEvent::Directory(path)) => (Tab::default(), Some(path.clone())),
        _ => {
            let welcome_content = crate::assets::get_default_markdown_content();
            (Tab::with_inline_content(welcome_content), None)
        }
    };

    // Get initial configuration values
    let directory_pref = settings::get_directory_preference(is_first_window);
    let theme_pref = settings::get_theme_preference(is_first_window);
    let sidebar_pref = settings::get_sidebar_preference(is_first_window);

    // Directory resolution: override (from event) → config → tab parent → home → root
    let directory = directory_override
        .or(directory_pref.directory)
        .or_else(|| tab.file().and_then(|p| p.parent().map(|p| p.to_path_buf())))
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));

    // Set up system event handler (for subsequent events)
    use_hook(|| {
        spawn_forever(async move {
            while let Some(event) = rx.recv().await {
                handle_open_event(event);
            }
        });
    });

    // Render App component with initial state
    rsx! {
        crate::components::app::App {
            tab: tab,
            directory: directory,
            theme: theme_pref.theme,
            sidebar_open: sidebar_pref.open,
            sidebar_width: sidebar_pref.width,
            sidebar_show_all_files: sidebar_pref.show_all_files,
        }
    }
}
