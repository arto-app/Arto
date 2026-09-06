mod assets;
mod bookmarks;
mod cache;
pub mod cli;
mod components;
mod config;
mod drag;
mod events;
mod history;
mod hooks;
pub mod ipc;
mod keybindings;
mod markdown;
#[cfg(not(target_os = "windows"))]
mod menu;
mod pinned_search;
mod shortcut;
mod state;
mod theme;
pub mod utils;
mod watcher;
mod window;

use dioxus::desktop::tao::event::{Event, WindowEvent};
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::prelude::*;

const DEFAULT_LOGLEVEL: &str = if cfg!(debug_assertions) {
    "debug"
} else {
    "info"
};

pub enum RunResult {
    SentToExistingInstance,
    Launched,
}

pub fn run(invocation: cli::CliInvocation) -> RunResult {
    // Try to send paths to existing instance via IPC
    // If successful, exit immediately without initializing anything else
    if let ipc::SendResult::Sent = ipc::try_send_to_existing_instance(&invocation) {
        return RunResult::SentToExistingInstance;
    }

    // Load environment variables from .env file
    if let Ok(dotenv) = dotenvy::dotenv() {
        println!("Loaded .env file from: {}", dotenv.display());
    }
    init_tracing();

    // Clear stale WebView cache when build changes (app upgrade via Homebrew, etc.)
    cache::clear_stale_webview_cache_if_needed();

    // Start IPC server to accept connections from future instances
    ipc::start_ipc_server();

    // Push CLI request to IPC event queue (MainApp will pop and apply it as initial state)
    if let Some(request) = ipc::build_open_request(&invocation) {
        let event = ipc::OpenEvent::Open(request);
        tracing::debug!(?event, "Pushing CLI request to IPC event queue");
        ipc::push_event(event);
    }

    // let menu = menu::build_menu();

    // Get window parameters for first window from preferences
    let params = window::CreateMainWindowConfigParams::from_preferences(true);

    let config = window::create_main_window_config(&params).with_custom_event_handler(
        move |event, _target| {
            match event {
                Event::Opened { urls, .. } => {
                    // Handle file/directory open events from Finder
                    tracing::debug!(url_count = urls.len(), "Event::Opened received");
                    for url in urls {
                        match url.to_file_path() {
                            Ok(path) => {
                                if let Some(event) = ipc::validate_path(path) {
                                    ipc::push_event(event);
                                }
                            }
                            Err(_) => {
                                tracing::info!(
                                    ?url,
                                    "Non file/directory path URL is specified. Skip."
                                );
                            }
                        }
                    }
                    // Process immediately (we're on the main thread)
                    ipc::process_main_thread_tasks();
                }
                Event::Reopen { .. } => {
                    // Handle dock click / app activation
                    tracing::debug!("Event::Reopen received (dock click or app activation)");
                    ipc::push_event(ipc::OpenEvent::Reopen { behavior: None });
                    ipc::process_main_thread_tasks();
                }
                Event::WindowEvent {
                    event: WindowEvent::Focused(true),
                    window_id,
                    ..
                } if !window::has_preview_window() => {
                    // Skip updating LAST_FOCUSED_WINDOW while a preview window exists
                    // to prevent focus from jumping to wrong window during drag.
                    // This blocks all focus updates during drag, not just when the
                    // preview window itself gains focus.
                    window::update_last_focused_window(*window_id);
                }
                Event::MainEventsCleared => {
                    // Defense in depth: drain the IPC queue once per event-loop cycle.
                    //
                    // On macOS, GCD wake (dispatch_async_f) reliably delivers IPC events,
                    // so this branch is effectively redundant. It exists as a fallback for
                    // future cross-platform support where wake_main_thread() may not have
                    // a fully reliable platform-specific implementation (e.g., Linux/Windows).
                    ipc::process_main_thread_tasks();
                }
                _ => {}
            }
        },
    );
    #[cfg(not(target_os = "windows"))]
    let config = config.with_menu(crate::menu::build_menu());
    #[cfg(target_os = "windows")]
    let config = config.with_menu(None);

    // Launch MainApp (first window only)
    // MainApp pops the first CLI event from IPC queue for its initial tab.
    // Remaining events are processed by custom_event_handler and GCD callbacks.
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(components::main_app::MainApp);

    // Clean up IPC socket on normal exit
    ipc::cleanup_socket();
    RunResult::Launched
}

fn init_tracing() {
    let silence_filter = tracing_subscriber::filter::filter_fn(|metadata| {
        // Filter out specific error from dioxus_core::properties:136
        // Known issue: https://github.com/DioxusLabs/dioxus/issues/3872
        metadata.target() != "dioxus_core::properties::__component_called_as_function"
    });

    let env_filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOGLEVEL));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .without_time()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .with_filter(silence_filter.clone());

    let registry = tracing_subscriber::registry()
        .with(env_filter_layer)
        .with(fmt_layer);

    // On macOS, log to Console.app via oslog
    #[cfg(target_os = "macos")]
    let registry = registry.with(
        tracing_oslog::OsLogger::new("com.lambdalisue.Arto", "default").with_filter(silence_filter),
    );

    registry.init();
}
