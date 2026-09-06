//! The primary instance's side: accept later launches and hand their
//! events to the app.

use crate::protocol::{IpcMessage, OpenEvent};
use crate::socket::{self, IpcError};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A bound listener, ready to accept secondary instances.
pub struct IpcServer {
    listener: Listener,
    socket_path: PathBuf,
}

impl IpcServer {
    /// Bind the socket, replacing a stale socket file from a crashed
    /// instance but refusing to steal one that is still answered.
    pub fn bind() -> Result<Self, IpcError> {
        let socket_path = socket::socket_path();

        // Ensure parent directory exists (for user-isolated paths like /tmp/arto-{uid}/)
        if let Some(parent) = socket_path.parent() {
            if !parent.exists() {
                create_private_directory(parent).map_err(|source| {
                    IpcError::CreateSocketDirectory {
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
        }

        let listener = socket::try_create_listener(&socket_path)?;
        tracing::debug!(?socket_path, "IPC listener bound");

        Ok(Self {
            listener,
            socket_path,
        })
    }

    /// Where this server listens.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Accept connections forever, calling `on_events` once per connection
    /// with everything that connection sent, in order.
    ///
    /// Each connection is read on its own thread so a slow or stalled
    /// client cannot hold up the next one; reads time out after
    /// [`IPC_TIMEOUT`](crate::IPC_TIMEOUT). A connection that sends nothing
    /// parseable does not call `on_events` at all. Blocks the calling
    /// thread for the life of the listener.
    pub fn serve(self, on_events: impl Fn(Vec<OpenEvent>) + Send + Sync + 'static) {
        let on_events: Arc<dyn Fn(Vec<OpenEvent>) + Send + Sync> = Arc::new(on_events);

        for conn in self.listener.incoming() {
            let stream = match conn {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::debug!(%error, "Failed to accept IPC connection");
                    continue;
                }
            };

            let handler = Arc::clone(&on_events);
            let spawned = std::thread::Builder::new()
                .name("ipc-client-handler".into())
                .spawn(move || handle_connection(stream, handler.as_ref()));
            if let Err(error) = spawned {
                // The closure, stream included, is consumed by the failed
                // spawn, so this connection is lost; the client sees its
                // write fail and falls back to becoming primary or retrying.
                tracing::debug!(%error, "Failed to spawn IPC client handler thread");
            }
        }
    }
}

/// Read JSON Lines until the client closes or the read times out, then
/// hand everything received to the app at once.
fn handle_connection(stream: Stream, on_events: &dyn Fn(Vec<OpenEvent>)) {
    // Set read timeout to avoid blocking forever
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    let reader = std::io::BufReader::new(stream);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                // Timeout or connection closed
                tracing::debug!(%error, "Error reading from IPC client");
                break;
            }
        };

        if line.is_empty() {
            continue;
        }

        let message: IpcMessage = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%line, %error, "Failed to parse IPC message");
                continue;
            }
        };

        tracing::debug!(?message, "Received IPC message");
        events.push(message.into_open_event());
    }

    if !events.is_empty() {
        on_events(events);
    }
}

/// Create the socket's parent directory readable by its owner only.
#[cfg(unix)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(path)
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}
