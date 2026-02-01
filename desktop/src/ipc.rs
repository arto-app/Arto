//! Single-instance IPC module
//!
//! This module provides IPC functionality to ensure only one instance of Arto runs at a time.
//! When a second instance is launched with paths, it sends those paths to the existing instance
//! via a local socket and exits.
//!
//! # Architecture
//!
//! ```text
//! 1st Instance (Primary):
//!   main() → try_send_to_existing() fails → start_ipc_server()
//!                                                 ↓
//!                                           accept connections
//!                                                 ↓
//!                                           recv JSON Lines → OPEN_EVENT_RECEIVER
//!
//! 2nd Instance (Secondary):
//!   main() → try_send_to_existing() succeeds → exit(0)
//! ```
//!
//! # Protocol
//!
//! Messages are sent as JSON Lines (one JSON object per line):
//!
//! ```json
//! {"type":"file","path":"/path/to/file.md"}
//! {"type":"directory","path":"/path/to/dir"}
//! {"type":"reopen"}
//! ```

use crate::components::main_app::OpenEvent;
use interprocess::local_socket::{prelude::*, GenericFilePath, ListenerOptions, Stream, ToFsName};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

/// Socket name for IPC communication.
const SOCKET_NAME: &str = "com.lambdalisue.arto.sock";

/// Timeout for IPC operations (connection, read, write).
const IPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Returns the platform-specific socket path with user isolation.
///
/// - Unix: Uses XDG_RUNTIME_DIR or falls back to /tmp with user ID
/// - Windows: Uses a named pipe path with user name
#[cfg(unix)]
fn get_socket_path() -> PathBuf {
    // Prefer XDG_RUNTIME_DIR (Linux) - already user-isolated
    if let Some(runtime_dir) = dirs::runtime_dir() {
        return runtime_dir.join(SOCKET_NAME);
    }

    // Fallback to /tmp with user ID for isolation
    // SAFETY: getuid() is always safe to call
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/arto-{uid}")).join(SOCKET_NAME)
}

#[cfg(windows)]
fn get_socket_path() -> PathBuf {
    // Windows named pipes are already isolated by session
    // Include username for additional safety
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    PathBuf::from(format!(r"\\.\pipe\arto-{}-{}", username, SOCKET_NAME))
}

/// Check if an IO error indicates "address already in use".
fn is_address_in_use(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        err.raw_os_error() == Some(libc::EADDRINUSE)
    }
    #[cfg(windows)]
    {
        // Windows error code for "pipe busy"
        // ERROR_PIPE_BUSY = 231
        // Note: ERROR_ACCESS_DENIED (5) is NOT included as it may indicate
        // legitimate permission issues unrelated to the pipe being in use
        err.raw_os_error() == Some(231)
    }
}

/// Set socket timeout for both send and receive operations (Unix).
#[cfg(unix)]
fn set_socket_timeout(stream: &Stream, timeout: Duration) {
    use std::os::fd::{AsFd, AsRawFd};

    // Access the inner Unix domain socket stream, if supported
    // Note: The pattern is currently irrefutable on Unix, but we use if-let
    // for forward compatibility in case the interprocess crate adds new stream types
    #[allow(irrefutable_let_patterns)]
    if let Stream::UdSocket(ref inner) = *stream {
        // Get raw fd via BorrowedFd
        let fd = inner.as_fd().as_raw_fd();
        let tv = libc::timeval {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_usec: timeout.subsec_micros() as libc::suseconds_t,
        };

        // SAFETY: fd is valid from the stream, tv is properly initialized
        unsafe {
            // Set send timeout
            let ret = libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_SNDTIMEO,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
            if ret != 0 {
                tracing::warn!(
                    "Failed to set IPC socket send timeout: {}",
                    std::io::Error::last_os_error()
                );
            }
            // Set receive timeout
            let ret = libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
            if ret != 0 {
                tracing::warn!(
                    "Failed to set IPC socket receive timeout: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
    } else {
        tracing::warn!("Unsupported IPC stream type for setting socket timeout");
    }
}

/// Set socket timeout for named pipes (Windows).
/// Note: Windows named pipes have different timeout semantics.
/// The timeout is set during pipe creation, not on the stream.
/// This function is a no-op but maintains API compatibility.
#[cfg(windows)]
fn set_socket_timeout(_stream: &Stream, _timeout: Duration) {
    // Windows named pipes set timeout at creation time via PIPE_WAIT mode
    // The interprocess crate handles this internally
    // For additional control, we would need to use SetNamedPipeHandleState
    // but the default behavior is acceptable for our use case
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
fn try_connect_with_timeout(socket_path: &Path, timeout: Duration) -> Option<Stream> {
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

/// IPC message types sent between instances as JSON Lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum IpcMessage {
    /// Open a file
    File { path: PathBuf },
    /// Open a directory (set as sidebar root)
    Directory { path: PathBuf },
    /// Reopen/activate the application (no arguments provided)
    Reopen,
}

impl IpcMessage {
    /// Convert to OpenEvent for internal use.
    fn into_open_event(self) -> OpenEvent {
        match self {
            IpcMessage::File { path } => OpenEvent::File(path),
            IpcMessage::Directory { path } => OpenEvent::Directory(path),
            IpcMessage::Reopen => OpenEvent::Reopen,
        }
    }
}

/// Result of trying to send paths to an existing instance.
pub enum SendResult {
    /// Successfully sent paths to existing instance - caller should exit
    Sent,
    /// No existing instance found - caller should become primary
    NoExistingInstance,
}

/// Try to send paths to an existing Arto instance.
///
/// If an existing instance is found, sends all paths and returns `SendResult::Sent`.
/// If no existing instance is found, returns `SendResult::NoExistingInstance`.
///
/// # Arguments
/// * `paths` - Paths to send to the existing instance
pub fn try_send_to_existing_instance(paths: &[PathBuf]) -> SendResult {
    let socket_path = get_socket_path();

    // Try to connect to existing instance with timeout
    let stream = match try_connect_with_timeout(&socket_path, IPC_TIMEOUT) {
        Some(stream) => stream,
        None => {
            // Connection failed or timed out - no existing instance, we become primary
            return SendResult::NoExistingInstance;
        }
    };

    // Send messages and check for errors (handles primary crash during send)
    match send_messages_to_primary(stream, paths) {
        Ok(()) => SendResult::Sent,
        Err(e) => {
            tracing::warn!(?e, "Failed to send messages to primary instance");
            // Primary may have crashed - caller should become primary
            SendResult::NoExistingInstance
        }
    }
}

/// Send messages to the primary instance, returning error if communication fails.
fn send_messages_to_primary(mut stream: Stream, paths: &[PathBuf]) -> std::io::Result<()> {
    // Set write timeout to avoid hanging if primary is stuck
    set_socket_timeout(&stream, IPC_TIMEOUT);

    // Build messages to send
    let mut messages: Vec<IpcMessage> = if paths.is_empty() {
        vec![IpcMessage::Reopen]
    } else {
        paths
            .iter()
            .filter_map(|path| {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if canonical.is_dir() {
                    Some(IpcMessage::Directory { path: canonical })
                } else if canonical.is_file() {
                    Some(IpcMessage::File { path: canonical })
                } else {
                    tracing::warn!(?path, "Skipping invalid path (not a file or directory)");
                    None
                }
            })
            .collect()
    };

    // If all paths were invalid (filtered out), send Reopen to activate the app
    if messages.is_empty() && !paths.is_empty() {
        tracing::debug!("All provided paths were invalid, sending Reopen instead");
        messages.push(IpcMessage::Reopen);
    }

    // Send messages as JSON Lines, checking each write
    for message in messages {
        let json = serde_json::to_string(&message)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(stream, "{json}")?;
    }

    // Flush and verify - this will fail if primary crashed
    stream.flush()?;

    Ok(())
}

/// Start the IPC server to listen for connections from new instances.
///
/// This function spawns a background thread that accepts connections and forwards
/// received paths to the given sender channel.
///
/// # Arguments
/// * `tx` - Channel sender to forward received paths as OpenEvents
pub fn start_ipc_server(tx: Sender<OpenEvent>) {
    // Register cleanup handler for graceful shutdown
    register_cleanup_handler();

    std::thread::spawn(move || {
        if let Err(e) = run_ipc_server_sync(tx) {
            tracing::error!(
                ?e,
                "IPC server failed to start or encountered a fatal error; \
                 single-instance enforcement is broken. Terminating to prevent duplicate instances."
            );
            // Fail fast: running without an IPC server breaks the single-instance guarantee,
            // so terminate the process rather than continuing in a degraded state.
            //
            // NOTE: process::exit() does not run destructors (e.g. use_drop / PersistedState::save).
            // This is acceptable because IPC server failure occurs during early startup, before any
            // user-visible windows or unsaved state exist.
            std::process::exit(1);
        }
    });
}

/// Remove the IPC socket file on clean exit.
///
/// This prevents stale socket detection on next startup.
#[cfg(unix)]
pub fn cleanup_socket() {
    let socket_path = get_socket_path();
    if socket_path.exists() {
        if let Err(e) = std::fs::remove_file(&socket_path) {
            tracing::warn!(?e, ?socket_path, "Failed to remove IPC socket on cleanup");
        } else {
            tracing::debug!(?socket_path, "IPC socket cleaned up");
        }
    }
}

#[cfg(not(unix))]
pub fn cleanup_socket() {
    // Windows named pipes are automatically cleaned up by the OS
}

/// Register signal handlers for clean socket cleanup.
///
/// This complements the stale socket detection on startup by handling
/// graceful shutdown cases like SIGINT and SIGTERM.
#[cfg(unix)]
fn register_cleanup_handler() {
    use std::sync::Once;
    static REGISTER_ONCE: Once = Once::new();

    REGISTER_ONCE.call_once(|| {
        match ctrlc::set_handler(|| {
            cleanup_socket();
        }) {
            Ok(()) => {
                tracing::debug!("IPC cleanup signal handler registered");
            }
            Err(e) => {
                tracing::warn!(?e, "Failed to register IPC cleanup signal handler");
            }
        }
    });
}

#[cfg(not(unix))]
fn register_cleanup_handler() {
    // No-op on Windows
}

/// Internal sync IPC server implementation.
fn run_ipc_server_sync(tx: Sender<OpenEvent>) -> anyhow::Result<()> {
    let socket_path = get_socket_path();

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
    let listener = match try_create_listener(&socket_path) {
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
                let tx = tx.clone();
                if let Err(e) = std::thread::Builder::new()
                    .name("ipc-client-handler".into())
                    .spawn(move || {
                        handle_client_connection(stream, tx);
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

/// Maximum retries for listener creation (handles TOCTOU race conditions)
const MAX_LISTENER_RETRIES: u32 = 3;

/// Try to create a listener, handling stale socket files safely.
///
/// This avoids race conditions by:
/// 1. First trying to create the listener directly
/// 2. If that fails with "address in use", checking if the socket is actually active
/// 3. Only removing the socket if it's confirmed to be stale (can't connect)
/// 4. Retrying with exponential backoff if another process races us
fn try_create_listener(socket_path: &Path) -> anyhow::Result<interprocess::local_socket::Listener> {
    for attempt in 0..MAX_LISTENER_RETRIES {
        match try_create_listener_once(socket_path) {
            Ok(listener) => return Ok(listener),
            Err(e) => {
                if attempt + 1 < MAX_LISTENER_RETRIES {
                    // Exponential backoff: 10ms, 20ms, 40ms...
                    let delay = Duration::from_millis(10 * (1 << attempt));
                    tracing::debug!(
                        attempt = attempt + 1,
                        ?delay,
                        "Listener creation failed, retrying"
                    );
                    std::thread::sleep(delay);
                } else {
                    return Err(e);
                }
            }
        }
    }
    unreachable!()
}

/// Single attempt to create a listener.
fn try_create_listener_once(
    socket_path: &Path,
) -> anyhow::Result<interprocess::local_socket::Listener> {
    let name = socket_path
        .to_fs_name::<GenericFilePath>()
        .map_err(|e| anyhow::anyhow!("Failed to create socket name: {e}"))?;

    // First attempt - try to create listener directly
    match ListenerOptions::new().name(name).create_sync() {
        Ok(listener) => return Ok(listener),
        Err(e) => {
            if !is_address_in_use(&e) {
                return Err(anyhow::anyhow!("Failed to create IPC listener: {e}"));
            }
            tracing::debug!("Socket exists, checking if it's stale");
        }
    }

    // Socket exists - check if it's active by trying to connect (with short timeout)
    let check_timeout = Duration::from_secs(1);
    if try_connect_with_timeout(socket_path, check_timeout).is_some() {
        // Another instance is actually running - this shouldn't happen
        // (try_send_to_existing_instance should have succeeded)
        return Err(anyhow::anyhow!(
            "Another instance is already running (socket is active)"
        ));
    }

    // Socket is stale - safe to remove (Unix only, Windows pipes auto-cleanup)
    #[cfg(unix)]
    {
        tracing::info!(?socket_path, "Removing stale socket file");
        // Ignore remove error - another process may have already removed it (TOCTOU race)
        if let Err(e) = std::fs::remove_file(socket_path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(?e, "Failed to remove stale socket file");
            }
        }
    }

    // Second attempt after removing stale socket
    let name = socket_path
        .to_fs_name::<GenericFilePath>()
        .map_err(|e| anyhow::anyhow!("Failed to create socket name: {e}"))?;

    ListenerOptions::new()
        .name(name)
        .create_sync()
        .map_err(|e| anyhow::anyhow!("Failed to create IPC listener after cleanup: {e}"))
}

/// Handle a single client connection.
fn handle_client_connection(stream: Stream, tx: Sender<OpenEvent>) {
    // Set read timeout to avoid blocking forever
    set_socket_timeout(&stream, IPC_TIMEOUT);

    let reader = BufReader::new(stream);

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
        if let Err(e) = tx.blocking_send(event) {
            tracing::warn!(?e, "Failed to send IPC event to channel");
        }
    }

    // After receiving messages, request the UI runtime to bring the window to front
    if let Err(e) = tx.blocking_send(OpenEvent::Focus) {
        tracing::warn!(?e, "Failed to send focus event to UI channel");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_ipc_message_file_serialization() {
        let msg = IpcMessage::File {
            path: PathBuf::from("/path/to/file.md"),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"file","path":"/path/to/file.md"}"#);
    }

    #[test]
    fn test_ipc_message_directory_serialization() {
        let msg = IpcMessage::Directory {
            path: PathBuf::from("/path/to/dir"),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"directory","path":"/path/to/dir"}"#);
    }

    #[test]
    fn test_ipc_message_reopen_serialization() {
        let msg = IpcMessage::Reopen;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"reopen"}"#);
    }

    #[test]
    fn test_ipc_message_file_deserialization() {
        let json = r#"{"type":"file","path":"/path/to/file.md"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, IpcMessage::File { path } if path == Path::new("/path/to/file.md")));
    }

    #[test]
    fn test_ipc_message_directory_deserialization() {
        let json = r#"{"type":"directory","path":"/path/to/dir"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, IpcMessage::Directory { path } if path == Path::new("/path/to/dir")));
    }

    #[test]
    fn test_ipc_message_reopen_deserialization() {
        let json = r#"{"type":"reopen"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, IpcMessage::Reopen));
    }

    #[test]
    fn test_ipc_message_into_open_event_file() {
        let msg = IpcMessage::File {
            path: PathBuf::from("/test.md"),
        };
        let event = msg.into_open_event();
        assert!(matches!(event, OpenEvent::File(p) if p == Path::new("/test.md")));
    }

    #[test]
    fn test_ipc_message_into_open_event_directory() {
        let msg = IpcMessage::Directory {
            path: PathBuf::from("/test/dir"),
        };
        let event = msg.into_open_event();
        assert!(matches!(event, OpenEvent::Directory(p) if p == Path::new("/test/dir")));
    }

    #[test]
    fn test_ipc_message_into_open_event_reopen() {
        let msg = IpcMessage::Reopen;
        let event = msg.into_open_event();
        assert!(matches!(event, OpenEvent::Reopen));
    }

    #[test]
    fn test_json_lines_protocol() {
        // Test that multiple messages can be parsed from newline-separated JSON
        let input = indoc! {r#"
            {"type":"file","path":"/file1.md"}
            {"type":"directory","path":"/dir"}
            {"type":"reopen"}
        "#};

        let messages: Vec<IpcMessage> = input
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(messages.len(), 3);
        assert!(
            matches!(&messages[0], IpcMessage::File { path } if path == Path::new("/file1.md"))
        );
        assert!(
            matches!(&messages[1], IpcMessage::Directory { path } if path == Path::new("/dir"))
        );
        assert!(matches!(&messages[2], IpcMessage::Reopen));
    }

    #[test]
    #[cfg(unix)]
    fn test_socket_path_contains_user_id() {
        let path = get_socket_path();

        // Ensure the socket file name is exactly SOCKET_NAME
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("Socket path should have a valid UTF-8 file name");
        assert_eq!(
            file_name, SOCKET_NAME,
            "Socket file name should be '{}', got '{}'",
            SOCKET_NAME, file_name
        );

        // Either XDG_RUNTIME_DIR or /tmp/arto-{uid}/
        let parent = path
            .parent()
            .expect("Socket path should have a parent directory");

        let xdg_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let parent_matches_xdg = xdg_runtime_dir.as_deref().is_some_and(|xdg| parent == xdg);

        let parent_str = parent.to_string_lossy();
        let parent_matches_tmp = parent_str.starts_with("/tmp/arto-");

        assert!(
            parent_matches_xdg || parent_matches_tmp,
            "Socket directory should be XDG_RUNTIME_DIR ({:?}) or start with '/tmp/arto-'; got {}",
            xdg_runtime_dir,
            parent_str
        );
    }

    #[test]
    fn test_is_address_in_use_for_non_matching_error() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        assert!(!is_address_in_use(&err));
    }
}
