//! The secondary instance's side: hand the request to whoever is listening.

use crate::protocol::{IpcMessage, OpenEvent};
use crate::socket;
use interprocess::local_socket::{prelude::*, GenericFilePath, Stream, ToFsName};
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// Outcome of trying to hand an event to an existing instance.
#[derive(Debug)]
pub enum SendResult {
    /// The running instance took the event; this process should exit.
    Sent,
    /// Nothing is listening; this process should become primary.
    NoExistingInstance,
    /// Something answered the connection but the event could not be
    /// delivered, most likely because the primary died mid-write. The
    /// caller decides whether to become primary anyway.
    Failed(std::io::Error),
}

/// Try to hand an event to an already running instance.
///
/// Connecting is bounded by a timeout so a wedged primary cannot hang a
/// new launch; no answer within it counts as no instance.
pub fn send_to_existing_instance(event: &OpenEvent) -> SendResult {
    let socket_path = socket::socket_path();

    let Some(stream) = connect_with_timeout(&socket_path, socket::IPC_TIMEOUT) else {
        return SendResult::NoExistingInstance;
    };

    match send_event(stream, event) {
        Ok(()) => SendResult::Sent,
        Err(error) => SendResult::Failed(error),
    }
}

/// Write one JSON line and make sure it went out.
fn send_event(mut stream: Stream, event: &OpenEvent) -> std::io::Result<()> {
    // A write timeout keeps a stuck primary from hanging this process.
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    let message = IpcMessage::from(event.clone());
    let json = serde_json::to_string(&message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(stream, "{json}")?;

    // Flush and verify - this will fail if primary crashed
    stream.flush()
}

/// Try to connect to a socket with timeout.
///
/// Returns None if connection fails or times out.
///
/// # Implementation Note
///
/// This function spawns a thread to perform the blocking connect() call,
/// then waits on a channel with timeout. If the timeout expires, the spawned
/// thread is abandoned and may continue running until connect() completes or fails.
///
/// While this could theoretically accumulate zombie threads if connection attempts
/// repeatedly timeout, in practice:
/// - The OS will eventually return from connect() (success or failure)
/// - Timeouts are rare in normal operation (only when primary instance is stuck)
/// - The secondary instance exits immediately after this function returns
///
/// Future improvements could use platform-specific SO_CONNECT_TIMEOUT socket options
/// or async runtimes with proper cancellation support.
pub(crate) fn connect_with_timeout(socket_path: &Path, timeout: Duration) -> Option<Stream> {
    let path = socket_path.to_path_buf();

    // Use a channel to communicate the result from the connection thread
    let (tx, rx) = mpsc::channel();

    let spawned = std::thread::Builder::new()
        .name("ipc-connect".to_string())
        .spawn({
            let tx = tx.clone();
            move || {
                let name = match path.to_fs_name::<GenericFilePath>() {
                    Ok(name) => name,
                    Err(_) => {
                        let _ = tx.send(None);
                        return;
                    }
                };

                let result = Stream::connect(name).ok();
                let _ = tx.send(result);
            }
        });

    match spawned {
        Ok(_handle) => {
            // Drop the original sender so rx detects disconnect if the thread panics
            // without sending (preserves original behavior of immediate Disconnected error)
            drop(tx);
        }
        Err(error) => {
            tracing::debug!(%error, "Failed to spawn IPC connection thread");
            let _ = tx.send(None);
        }
    }

    // Wait for result with timeout
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => {
            // Timeout or channel closed - connection thread may still be running
            // but we don't wait for it (it will terminate when connect completes/fails)
            tracing::debug!("Connection attempt timed out");
            None
        }
    }
}
