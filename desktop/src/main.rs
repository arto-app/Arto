mod assets;
mod components;
mod config;
mod drag;
mod events;
mod history;
mod markdown;
mod menu;
mod state;
mod theme;
mod utils;
mod watcher;
mod window;

use dioxus::desktop::tao::event::{Event, WindowEvent};
use tokio::sync::mpsc::channel;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::prelude::*;

const DEFAULT_LOGLEVEL: &str = if cfg!(debug_assertions) {
    "debug"
} else {
    "info"
};

fn main() {
    // Load environment variables from .env file
    if let Ok(dotenv) = dotenvy::dotenv() {
        println!("Loaded .env file from: {}", dotenv.display());
    }
    init_tracing();

    // Create event channel and store receiver for MainApp
    let (tx, rx) = channel::<components::main_app::OpenEvent>(10);
    components::main_app::OPEN_EVENT_RECEIVER
        .lock()
        .expect("Failed to lock OPEN_EVENT_RECEIVER")
        .replace(rx);

    let menu = menu::build_menu();

    // Get window parameters for first window from preferences
    let params = window::CreateMainWindowConfigParams::from_preferences(true);

    let config = window::create_main_window_config(&params)
        .with_custom_event_handler(move |event, _target| match event {
            Event::Opened { urls, .. } => {
                for url in urls {
                    if let Ok(path) = url.to_file_path() {
                        let open_event = if path.is_dir() {
                            components::main_app::OpenEvent::Directory(path)
                        } else if path.is_file() {
                            components::main_app::OpenEvent::File(path)
                        } else {
                            // Skip invalid paths
                            continue;
                        };
                        tx.try_send(open_event).expect("Failed to send open event");
                    }
                }
            }
            Event::Reopen { .. } => {
                // Send reopen event through channel to handle it safely in component context
                tx.try_send(components::main_app::OpenEvent::Reopen).ok();
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(true),
                window_id,
                ..
            } => {
                // Skip updating LAST_FOCUSED_WINDOW for preview windows
                // to prevent focus from jumping to wrong window during drag
                if window::get_preview_window_id() != Some(*window_id) {
                    window::update_last_focused_window(*window_id);
                }
            }
            _ => {}
        })
        .with_menu(menu);

    // Launch MainApp (first window only)
    // Initial event will be consumed inside MainApp after Dioxus starts
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(components::main_app::MainApp);
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
