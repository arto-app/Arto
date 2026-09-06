use super::protocol::{build_open_request, IpcMessage, SendResult};
use super::socket;
use interprocess::local_socket::{prelude::*, GenericFilePath, Stream, ToFsName};
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// Try to send a CLI invocation to an existing Arto instance.
///
/// If an existing instance is found, sends invocation data and returns `SendResult::Sent`.
/// If no existing instance is found, returns `SendResult::NoExistingInstance`.
///
/// # Arguments
/// * `invocation` - CLI invocation data to send to the existing instance
pub fn try_send_to_existing_instance(invocation: &crate::cli::CliInvocation) -> SendResult {
    let socket_path = socket::get_socket_path();

    // Try to connect to existing instance with timeout
    let stream = match try_connect_with_timeout(&socket_path, socket::IPC_TIMEOUT) {
        Some(stream) => stream,
        None => {
            // Connection failed or timed out - no existing instance, we become primary
            return SendResult::NoExistingInstance;
        }
    };

    // Send messages and check for errors (handles primary crash during send)
    match send_messages_to_primary(stream, invocation) {
        Ok(()) => SendResult::Sent,
        Err(e) => {
            tracing::warn!(?e, "Failed to send messages to primary instance");
            // Primary may have crashed - caller should become primary
            SendResult::NoExistingInstance
        }
    }
}

/// Send messages to the primary instance, returning error if communication fails.
fn send_messages_to_primary(
    mut stream: Stream,
    invocation: &crate::cli::CliInvocation,
) -> std::io::Result<()> {
    // Set write timeout to avoid hanging if primary is stuck
    socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT);

    let message = if let Some(request) = build_open_request(invocation) {
        IpcMessage::from_open_request(request)
    } else {
        IpcMessage::Reopen {
            behavior: invocation.open_mode.to_file_open_behavior(),
        }
    };

    // Send one message as JSON Line, checking write
    let json = serde_json::to_string(&message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(stream, "{json}")?;

    // Flush and verify - this will fail if primary crashed
    stream.flush()?;

    Ok(())
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
pub(super) fn try_connect_with_timeout(socket_path: &Path, timeout: Duration) -> Option<Stream> {
    let path = socket_path.to_path_buf();

    // Use a channel to communicate the result from the connection thread
    let (tx, rx) = mpsc::channel();

    let tx_clone = tx.clone();

    // Spawn a named thread to attempt connection (blocking)
    match std::thread::Builder::new()
        .name("ipc-connect".to_string())
        .spawn(move || {
            let name = match path.to_fs_name::<GenericFilePath>() {
                Ok(name) => name,
                Err(_) => {
                    let _ = tx_clone.send(None);
                    return;
                }
            };

            let result = Stream::connect(name).ok();
            let _ = tx_clone.send(result);
        }) {
        Ok(_handle) => {
            // Drop the original sender so rx detects disconnect if the thread panics
            // without sending (preserves original behavior of immediate Disconnected error)
            drop(tx);
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to spawn IPC connection thread");
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
