use super::protocol::IpcMessage;
use super::queue::push_event;
use super::socket;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::Stream;
use std::io::BufRead;

/// Start the IPC server to listen for connections from new instances.
///
/// This function spawns a background thread that accepts connections. Received
/// events are pushed to the global IPC event queue and the main thread is woken
/// via GCD to process them.
///
/// # Thread Lifecycle
///
/// The spawned thread runs indefinitely and is not explicitly joined on shutdown.
/// Socket cleanup relies on:
/// - Signal handlers (`register_cleanup_handler()`) to remove the socket on SIGTERM/SIGINT
/// - Stale socket detection on next startup to handle crashes
/// - OS-level cleanup when the process exits
///
/// This design trade-off avoids the complexity of coordinating graceful shutdown
/// with Dioxus's lifecycle. Any leftover socket file is harmless and cleaned up on next launch.
pub fn start_ipc_server() {
    // Register cleanup handler for graceful shutdown
    socket::register_cleanup_handler();

    std::thread::spawn(move || {
        if let Err(e) = run_ipc_server_sync() {
            tracing::error!(
                ?e,
                "IPC server failed to start or encountered a fatal error; \
                 single-instance enforcement is broken. Terminating to prevent duplicate instances."
            );
            // Fail fast: running without an IPC server breaks the single-instance guarantee,
            // so terminate the process rather than continuing in a degraded state.
            //
            // NOTE: process::exit() does not run destructors (e.g. use_drop / PersistedState::save).
            // This is acceptable because run_ipc_server_sync() only returns Err during listener
            // creation (before entering the accept loop), which happens during early startup before
            // any user-visible windows or unsaved state exist.
            std::process::exit(1);
        }
    });
}

/// Internal sync IPC server implementation.
fn run_ipc_server_sync() -> anyhow::Result<()> {
    let socket_path = socket::get_socket_path();

    // Ensure parent directory exists (for user-isolated paths like /tmp/arto-{uid}/)
    if let Some(parent) = socket_path.parent() {
        if !parent.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = std::fs::DirBuilder::new();
                builder.mode(0o700); // Owner-only access for security
                builder.recursive(true);
                builder.create(parent)?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    tracing::info!(?socket_path, "IPC server starting");

    // Try to create listener - handles race condition properly
    let listener = match socket::try_create_listener(&socket_path) {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!(?e, ?socket_path, "Failed to create IPC listener");
            return Err(e);
        }
    };

    tracing::info!("IPC server ready for connections");

    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                if let Err(e) = std::thread::Builder::new()
                    .name("ipc-client-handler".into())
                    .spawn(move || {
                        handle_client_connection(stream);
                    })
                {
                    tracing::error!(?e, "Failed to spawn IPC client handler thread");
                }
            }
            Err(e) => {
                tracing::warn!(?e, "Failed to accept IPC connection");
            }
        }
    }

    Ok(())
}

/// Handle a single client connection.
///
/// Parses JSON Lines messages, pushes events to the global queue,
/// and wakes the main thread via GCD to process them.
fn handle_client_connection(stream: Stream) {
    // Set read timeout to avoid blocking forever
    socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT);

    let reader = std::io::BufReader::new(stream);
    let mut received_events = false;

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                // Timeout or connection closed
                tracing::debug!(?e, "Error reading from IPC client");
                break;
            }
        };

        if line.is_empty() {
            continue;
        }

        // Parse JSON Line
        let message: IpcMessage = match serde_json::from_str(&line) {
            Ok(msg) => msg,
            Err(e) => {
                tracing::warn!(%line, ?e, "Failed to parse IPC message");
                continue;
            }
        };

        tracing::debug!(?message, "Received IPC message");

        let event = message.into_open_event();
        push_event(event);
        received_events = true;
    }

    // Wake main thread once after processing all messages from this client
    if received_events {
        super::wake_main_thread();
    }
}
