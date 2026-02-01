mod assets;
mod bookmarks;
mod cache;
mod components;
mod config;
mod drag;
mod events;
mod history;
mod ipc;
mod markdown;
mod menu;
mod pinned_search;
mod state;
mod theme;
mod utils;
mod watcher;
mod window;

use clap::Parser;
use dioxus::desktop::tao::event::{Event, WindowEvent};
use std::path::PathBuf;
use tokio::sync::mpsc::channel;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::prelude::*;

/// Arto - A markdown viewer
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Files or directories to open
    #[arg()]
    paths: Vec<PathBuf>,
}

const DEFAULT_LOGLEVEL: &str = if cfg!(debug_assertions) {
    "debug"
} else {
    "info"
};

fn main() {
    // Parse CLI arguments first (before any other initialization)
    let cli = Cli::parse();

    // Try to send paths to existing instance via IPC
    // If successful, exit immediately without initializing anything else
    if let ipc::SendResult::Sent = ipc::try_send_to_existing_instance(&cli.paths) {
        std::process::exit(0);
    }

    // Load environment variables from .env file
    if let Ok(dotenv) = dotenvy::dotenv() {
        println!("Loaded .env file from: {}", dotenv.display());
    }
    init_tracing();

    // Clear stale WebView cache when build changes (app upgrade via Homebrew, etc.)
    cache::clear_stale_webview_cache_if_needed();

    // Create event channel and store receiver for MainApp
    let (tx, rx) = channel::<components::main_app::OpenEvent>(10);
    components::main_app::OPEN_EVENT_RECEIVER
        .lock()
        .expect("Failed to lock OPEN_EVENT_RECEIVER")
        .replace(rx);

    // Start IPC server to accept connections from future instances
    ipc::start_ipc_server(tx.clone());

    // Send CLI paths as OpenEvents (before Dioxus launches)
    for path in cli.paths {
        let event = match ipc::validate_path(&path) {
            Some(event) => event,
            None => continue, // Invalid path, already logged by validate_path
        };
        tracing::debug!(?event, "Sending CLI path as open event");
        if let Err(e) = tx.try_send(event) {
            tracing::warn!(?e, "Failed to send CLI path event");
        }
    }

    let menu = menu::build_menu();

    // Get window parameters for first window from preferences
    let params = window::CreateMainWindowConfigParams::from_preferences(true);

    let config = window::create_main_window_config(&params)
        .with_custom_event_handler(move |event, _target| {
            use components::main_app::OpenEvent;
            match event {
                Event::Opened { urls, .. } => {
                    let paths = urls.iter().filter_map(|url| match url.to_file_path() {
                        Ok(path) => Some(path),
                        Err(_) => {
                            tracing::info!(?url, "Non file/directory path URL is specified. Skip.");
                            None
                        }
                    });
                    for path in paths {
                        if path.is_dir() {
                            tx.try_send(OpenEvent::Directory(path))
                                .expect("Failed to send directory open event");
                        } else {
                            tx.try_send(OpenEvent::File(path))
                                .expect("Failed to send file open event");
                        };
                    }
                }
                Event::Reopen { .. } => {
                    // Send reopen event through channel to handle it in component context
                    tx.try_send(OpenEvent::Reopen)
                        .expect("Failed to send reopen event");
                }
                Event::WindowEvent {
                    event: WindowEvent::Focused(true),
                    window_id,
                    ..
                } => {
                    // Skip updating LAST_FOCUSED_WINDOW while a preview window exists
                    // to prevent focus from jumping to wrong window during drag.
                    // This blocks all focus updates during drag, not just when the
                    // preview window itself gains focus.
                    if !window::has_preview_window() {
                        window::update_last_focused_window(*window_id);
                    }
                }
                _ => {}
            }
        })
        .with_menu(menu);

    // Launch MainApp (first window only)
    // Initial event will be consumed inside MainApp after Dioxus starts
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(components::main_app::MainApp);

    // Clean up IPC socket on normal exit
    ipc::cleanup_socket();
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
    let registry = registry.with(
        tracing_oslog::OsLogger::new("com.lambdalisue.Arto", "default").with_filter(silence_filter),
    );

    registry.init();
}
