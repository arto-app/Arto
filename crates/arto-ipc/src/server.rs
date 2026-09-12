//! The primary instance's side: accept later launches and hand their
//! events to the app.

use crate::client::READY_TIMEOUT;
use crate::protocol::{IpcMessage, OpenEvent, ReadyReply};
use crate::socket::{self, IpcError};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;

/// The app's end of a launch that is waiting to be told the window drew it.
///
/// Handed to the events callback only when the launch asked to wait, and
/// carried by the app to whatever finally knows the answer. Dropping it
/// without a call to [`ReadySignal::fire`] releases the waiting launch too:
/// nothing that can still answer exists once the last one is gone.
pub struct ReadySignal(mpsc::Sender<()>);

impl ReadySignal {
    /// Tell the waiting launch the window has drawn what it asked for.
    pub fn fire(self) {
        let _ = self.0.send(());
    }
}

impl std::fmt::Debug for ReadySignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReadySignal")
    }
}

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
    ///
    /// The second argument is `Some` only for a launch that asked to wait
    /// for its window; firing it writes the reply that lets that launch
    /// exit. See [`ReadySignal`].
    pub fn serve(
        self,
        on_events: impl Fn(Vec<OpenEvent>, Option<ReadySignal>) + Send + Sync + 'static,
    ) {
        let on_events: Arc<dyn Fn(Vec<OpenEvent>, Option<ReadySignal>) + Send + Sync> =
            Arc::new(on_events);

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
///
/// A message that asks to wait ends the read early instead: such a client is
/// not going to close the connection — that is what it is waiting on — so
/// reading to EOF would deadlock the two of them against each other. It
/// sends exactly one message, so everything it had to say has been read by
/// the time the flag is seen.
fn handle_connection(stream: Stream, on_events: &dyn Fn(Vec<OpenEvent>, Option<ReadySignal>)) {
    // Set read timeout to avoid blocking forever
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    let mut reader = std::io::BufReader::new(stream);
    let mut events = Vec::new();
    let mut wait_ready = false;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) => {
                // Timeout or connection closed
                tracing::debug!(%error, "Error reading from IPC client");
                break;
            }
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }

        let message: IpcMessage = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%line, %error, "Failed to parse IPC message");
                continue;
            }
        };

        tracing::debug!(?message, "Received IPC message");
        wait_ready = message.wait_ready();
        events.push(message.into_open_event());
        if wait_ready {
            break;
        }
    }

    if events.is_empty() {
        return;
    }

    if !wait_ready {
        on_events(events, None);
        return;
    }

    let (sender, receiver) = mpsc::channel();
    on_events(events, Some(ReadySignal(sender)));

    let reply = match receiver.recv_timeout(READY_TIMEOUT) {
        Ok(()) => ReadyReply::Ready,
        // Disconnected means every signal was dropped, so no answer is ever
        // coming; saying so at once beats holding the launch for the full
        // timeout to reach the same place.
        Err(error) => {
            tracing::debug!(%error, "No window reported ready for a waiting launch");
            ReadyReply::Timeout
        }
    };
    reply_to_waiting_client(reader.into_inner(), &reply);
}

fn reply_to_waiting_client(mut stream: Stream, reply: &ReadyReply) {
    let Ok(json) = serde_json::to_string(reply) else {
        return;
    };
    if let Err(error) = writeln!(stream, "{json}").and_then(|()| stream.flush()) {
        tracing::debug!(%error, "Could not send the ready reply");
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
