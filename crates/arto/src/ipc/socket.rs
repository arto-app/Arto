use super::client::try_connect_with_timeout;
#[cfg(unix)]
use super::queue::request_shutdown;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, Stream, ToFsName};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Socket name for IPC communication.
pub(super) const SOCKET_NAME: &str = "com.lambdalisue.arto.sock";

/// Timeout for IPC operations (connection, read, write).
pub(super) const IPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum retries for listener creation (handles TOCTOU race conditions)
const MAX_LISTENER_RETRIES: u32 = 3;

/// Returns the platform-specific socket path with user isolation.
///
/// - Unix: Uses XDG_RUNTIME_DIR or falls back to /tmp with user ID
/// - Windows: Uses a named pipe path with user name
#[cfg(unix)]
pub(super) fn get_socket_path() -> PathBuf {
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
pub(super) fn get_socket_path() -> PathBuf {
    // Windows named pipes are already isolated by session
    // Include username for additional safety
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    PathBuf::from(format!(r"\\.\pipe\arto-{}-{}", username, SOCKET_NAME))
}

/// Check if an IO error indicates "address already in use".
pub(super) fn is_address_in_use(err: &std::io::Error) -> bool {
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
pub(super) fn set_socket_timeout(stream: &Stream, timeout: Duration) {
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
pub(super) fn set_socket_timeout(_stream: &Stream, _timeout: Duration) {
    // Windows named pipes set timeout at creation time via PIPE_WAIT mode
    // The interprocess crate handles this internally
    // For additional control, we would need to use SetNamedPipeHandleState
    // but the default behavior is acceptable for our use case
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
///
/// Uses signal-hook to allow multiple independent signal handlers to coexist,
/// avoiding conflicts with other parts of the application that may need to
/// handle signals.
#[cfg(unix)]
pub(super) fn register_cleanup_handler() {
    use signal_hook::{consts::signal::*, iterator::Signals};
    use std::sync::Once;
    use std::thread;

    static REGISTER_ONCE: Once = Once::new();

    REGISTER_ONCE.call_once(|| {
        match Signals::new([SIGINT, SIGTERM]) {
            Ok(mut signals) => {
                // Spawn a dedicated thread to listen for termination signals and
                // request graceful shutdown when they are received. This approach
                // allows multiple independent signal handlers to coexist.
                thread::spawn(move || {
                    for signal in &mut signals {
                        match signal {
                            SIGINT | SIGTERM => {
                                request_shutdown(signal);
                            }
                            _ => {}
                        }
                    }
                });
                tracing::debug!("IPC cleanup signal handler registered");
            }
            Err(e) => {
                tracing::warn!(?e, "Failed to register IPC cleanup signal handler");
            }
        }
    });
}

#[cfg(not(unix))]
pub(super) fn register_cleanup_handler() {
    // No-op on Windows
}

/// Try to create a listener, handling stale socket files safely.
///
/// This avoids race conditions by:
/// 1. First trying to create the listener directly
/// 2. If that fails with "address in use", checking if the socket is actually active
/// 3. Only removing the socket if it's confirmed to be stale (can't connect)
/// 4. Retrying with exponential backoff if another process races us
pub(super) fn try_create_listener(
    socket_path: &Path,
) -> anyhow::Result<interprocess::local_socket::Listener> {
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
        // Another instance became primary between our initial check and listener creation.
        // This is a valid race during concurrent launches; the caller may choose to retry.
        return Err(anyhow::anyhow!(
            "Another instance became primary during initialization; please retry"
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
