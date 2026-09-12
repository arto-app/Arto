//! The secondary instance's side: hand the request to whoever is listening.

use crate::protocol::{IpcMessage, OpenEvent, ReadyReply};
use crate::socket;
use interprocess::local_socket::{prelude::*, GenericFilePath, Stream, ToFsName};
use std::io::{BufRead, Write};
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

/// How long a launch that asked to wait will hold the connection open.
///
/// Far longer than [`socket::IPC_TIMEOUT`], which bounds a handshake: this
/// bounds a document being read from disk, parsed, and drawn with its
/// diagrams and formulas. A cold start with a large document is seconds, and
/// the point of waiting is to not have to guess how many.
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Try to hand an event to an already running instance.
///
/// Connecting is bounded by a timeout so a wedged primary cannot hang a
/// new launch; no answer within it counts as no instance.
///
/// With `wait_ready`, the connection stays open until the primary says the
/// target window has drawn the request, or until [`READY_TIMEOUT`] passes.
/// A primary too old to understand the flag closes the connection instead of
/// replying, which reads as no reply and returns just as a reply would — the
/// request itself was still delivered.
pub fn send_to_existing_instance(event: &OpenEvent, wait_ready: bool) -> SendResult {
    let socket_path = socket::socket_path();

    let Some(stream) = connect_with_timeout(&socket_path, socket::IPC_TIMEOUT) else {
        return SendResult::NoExistingInstance;
    };

    match send_event(stream, event, wait_ready) {
        Ok(()) => SendResult::Sent,
        Err(error) => SendResult::Failed(error),
    }
}

/// Write one JSON line and make sure it went out.
fn send_event(mut stream: Stream, event: &OpenEvent, wait_ready: bool) -> std::io::Result<()> {
    // A write timeout keeps a stuck primary from hanging this process.
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    let message = IpcMessage::from_event(event.clone(), wait_ready);
    let json = serde_json::to_string(&message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(stream, "{json}")?;

    // Flush and verify - this will fail if primary crashed
    stream.flush()?;

    if wait_ready {
        await_ready(stream);
    }
    Ok(())
}

/// Block until the primary answers, it hangs up, or the wait runs out.
///
/// Nothing here fails the send: the request was delivered and acknowledged
/// by the write above, and what is being waited for is a courtesy on top of
/// it. Every way the wait can end is therefore a debug line and a return.
fn await_ready(stream: Stream) {
    if let Err(error) = socket::set_socket_timeout(&stream, READY_TIMEOUT) {
        tracing::debug!(%error, "Could not extend the IPC socket timeout for the ready wait");
    }

    let mut line = String::new();
    match std::io::BufReader::new(stream).read_line(&mut line) {
        Ok(0) => tracing::debug!("Primary closed the connection without a ready reply"),
        Ok(_) => match serde_json::from_str::<ReadyReply>(line.trim()) {
            Ok(ReadyReply::Ready) => tracing::debug!("Target window reported ready"),
            Ok(ReadyReply::Timeout) => {
                tracing::debug!("Primary gave up waiting for the target window")
            }
            Err(error) => tracing::debug!(%line, %error, "Unparseable ready reply"),
        },
        Err(error) => tracing::debug!(%error, "Gave up waiting for the ready reply"),
    }
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
