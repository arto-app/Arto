mod assets;
mod bookmarks;
mod cache;
pub mod cli;
mod components;
mod config;
mod document_link;
mod events;
mod files;
mod fuzzy;
mod history;
mod hooks;
pub mod ipc;
mod keybindings;
mod markdown;
#[cfg(target_os = "macos")]
mod menu;
mod pinned_search;
mod roots;
mod scroll_anchor;
mod state;
mod theme;
pub mod utils;
mod visits;
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
    /// A running instance took the request; exit with this status without
    /// starting anything.
    HandedOver(i32),
    Launched,
}

pub fn run(invocation: cli::CliInvocation) -> RunResult {
    // Both of these come before the handoff, not after it, because the
    // handoff is where a launch can fail: a secondary that reports nothing
    // is a secondary whose only diagnostics went to a subscriber that had
    // not been installed yet. `.env` stays ahead of tracing so it can still
    // set `RUST_LOG`.
    if let Ok(dotenv) = dotenvy::dotenv() {
        println!("Loaded .env file from: {}", dotenv.display());
    }
    init_tracing();

    // Try to hand the request to an existing instance. Only "nothing is
    // listening" and "the connection failed before anything was written"
    // leave this process free to go on and become primary; anything else
    // means the request is already with a live instance, and starting a
    // second app would either open the document twice or fail to bind.
    match ipc::try_send_to_existing_instance(&invocation) {
        ipc::SendResult::Applied { ready } => {
            if invocation.wait_ready && !ready {
                eprintln!("arto: the window did not report a finished render in time");
                return RunResult::HandedOver(1);
            }
            return RunResult::HandedOver(0);
        }
        ipc::SendResult::Refused(error) => {
            eprintln!("arto: the running instance refused the request: {error}");
            return RunResult::HandedOver(1);
        }
        ipc::SendResult::Unanswered => {
            eprintln!(
                "arto: handed the request over but got no answer; \
                 it may or may not have been carried out"
            );
            return RunResult::HandedOver(1);
        }
        ipc::SendResult::NoExistingInstance | ipc::SendResult::Failed(_) => {}
    }

    // Clear stale WebView cache when build changes (app upgrade via Homebrew, etc.)
    cache::clear_stale_webview_cache_if_needed();

    // Start IPC server to accept connections from future instances
    ipc::start_ipc_server();

    // Push CLI request to IPC event queue (MainApp will pop and apply it as initial state)
    //
    // A launch naming no path queues nothing, and the window opens on the
    // welcome page — unless it asked for a geometry or a theme, which the
    // first window has to be told about, and the event is how it is told.
    if let Some(request) = ipc::build_open_request(&invocation) {
        let event = ipc::OpenEvent::Open(request);
        tracing::debug!(?event, "Pushing CLI request to IPC event queue");
        ipc::push_event(event);
    } else if !invocation.window.is_empty() {
        let event = ipc::open_event_for_invocation(&invocation);
        tracing::debug!(?event, "Pushing CLI window options to IPC event queue");
        ipc::push_event(event);
    }

    // let menu = menu::build_menu();

    // Get window parameters for first window from preferences, with anything
    // this launch asked for laid over them.
    let params = window::CreateMainWindowConfigParams {
        focused: !invocation.behind,
        ..window::CreateMainWindowConfigParams::from_preferences(true)
    }
    .with_window_options(&invocation.window);

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
                    ipc::push_event(ipc::OpenEvent::Reopen {
                        behavior: None,
                        behind: false,
                        window: Default::default(),
                    });
                    ipc::process_main_thread_tasks();
                }
                Event::WindowEvent {
                    event: WindowEvent::Focused(true),
                    window_id,
                    ..
                } => {
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
    // Only macOS gets a native menu, because only there does it live outside
    // the window. On Windows and Linux the same items hang off one glyph in
    // the header instead, so nothing takes a strip of the window to say what
    // the app is called.
    #[cfg(target_os = "macos")]
    let config = config.with_menu(crate::menu::build_menu());
    #[cfg(not(target_os = "macos"))]
    let config = config.with_menu(None);

    // Tao activates the app as soon as it finishes launching. `--behind` has
    // to own the event loop to turn that off, so the launch leaves whatever
    // the user was working in frontmost.
    #[cfg(target_os = "macos")]
    let config = if invocation.behind {
        use dioxus::desktop::tao::event_loop::EventLoopBuilder;
        use dioxus::desktop::tao::platform::macos::EventLoopExtMacOS;

        let mut event_loop = EventLoopBuilder::with_user_event().build();
        event_loop.set_activate_ignoring_other_apps(false);
        config.with_event_loop(event_loop)
    } else {
        config
    };

    // Launch MainApp (first window only)
    // MainApp pops the first CLI event from IPC queue for its initial document.
    // Remaining events are processed by custom_event_handler and GCD callbacks.
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(components::main_app::MainApp);

    // Clean up IPC socket on normal exit
    ipc::cleanup_socket();
    RunResult::Launched
}

fn init_tracing() {
    let env_filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOGLEVEL));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .without_time()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    let registry = tracing_subscriber::registry()
        .with(env_filter_layer)
        .with(fmt_layer);

    // On macOS, log to Console.app via oslog
    #[cfg(target_os = "macos")]
    let registry = registry.with(tracing_oslog::OsLogger::new(
        "com.lambdalisue.Arto",
        "default",
    ));

    registry.init();
}
