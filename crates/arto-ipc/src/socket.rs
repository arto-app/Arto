//! Where the socket lives and how the listener is created.

use crate::client::connect_with_timeout;
use interprocess::local_socket::{GenericFilePath, Listener, ListenerOptions, Stream, ToFsName};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// File name of the socket (Unix) or suffix of the pipe name (Windows).
pub const SOCKET_NAME: &str = "com.lambdalisue.arto.sock";

/// Timeout for IPC operations (connection, read, write).
pub const IPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum retries for listener creation (handles TOCTOU race conditions)
const MAX_LISTENER_RETRIES: u32 = 3;

/// Why the primary instance could not start listening.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("{path} is not a valid socket name: {source}")]
    InvalidSocketName {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot create the socket directory {path}: {source}")]
    CreateSocketDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot listen on {path}: {source}")]
    Bind {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Another process won the race to become primary between this
    /// process's connection attempt and its bind. Happens only when two
    /// launches start within milliseconds of each other.
    #[error("another instance became primary at {path} during initialization")]
    AnotherInstanceActive { path: PathBuf },
}

/// The platform-specific socket path, isolated per user.
///
/// - Unix: `$XDG_RUNTIME_DIR/<name>`, or `/tmp/arto-<uid>/<name>`
/// - Windows: a named pipe carrying the user name
#[cfg(unix)]
pub fn socket_path() -> PathBuf {
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
pub fn socket_path() -> PathBuf {
    // Windows named pipes are already isolated by session
    // Include username for additional safety
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    PathBuf::from(format!(r"\\.\pipe\arto-{}-{}", username, SOCKET_NAME))
}

/// Check if an IO error indicates "address already in use".
pub(crate) fn is_address_in_use(err: &std::io::Error) -> bool {
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
pub(crate) fn set_socket_timeout(stream: &Stream, timeout: Duration) -> std::io::Result<()> {
    use std::os::fd::{AsFd, AsRawFd};

    // Access the inner Unix domain socket stream, if supported
    // Note: The pattern is currently irrefutable on Unix, but we use if-let
    // for forward compatibility in case the interprocess crate adds new stream types
    #[allow(irrefutable_let_patterns)]
    let Stream::UdSocket(ref inner) = *stream
    else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported IPC stream type for setting socket timeout",
        ));
    };

    // Get raw fd via BorrowedFd
    let fd = inner.as_fd().as_raw_fd();
    let tv = libc::timeval {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_usec: timeout.subsec_micros() as libc::suseconds_t,
    };

    // Try both directions even if one fails: a read timeout still protects
    // the server thread when only the send timeout could not be set.
    let mut result = Ok(());
    for option in [libc::SO_SNDTIMEO, libc::SO_RCVTIMEO] {
        // SAFETY: fd is valid from the stream, tv is properly initialized
        let ret = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        };
        if ret != 0 && result.is_ok() {
            result = Err(std::io::Error::last_os_error());
        }
    }
    result
}

/// Set socket timeout for named pipes (Windows).
/// Note: Windows named pipes have different timeout semantics.
/// The timeout is set during pipe creation, not on the stream.
/// This function is a no-op but maintains API compatibility.
#[cfg(windows)]
pub(crate) fn set_socket_timeout(_stream: &Stream, _timeout: Duration) -> std::io::Result<()> {
    // Windows named pipes set timeout at creation time via PIPE_WAIT mode
    // The interprocess crate handles this internally
    // For additional control, we would need to use SetNamedPipeHandleState
    // but the default behavior is acceptable for our use case
    Ok(())
}

/// Remove the socket file so the next launch does not find a stale one.
///
/// A file that is already gone is not an error.
#[cfg(unix)]
pub fn cleanup_socket() -> std::io::Result<()> {
    match std::fs::remove_file(socket_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
pub fn cleanup_socket() -> std::io::Result<()> {
    // Windows named pipes are automatically cleaned up by the OS
    Ok(())
}

/// Try to create a listener, handling stale socket files safely.
///
/// This avoids race conditions by:
/// 1. First trying to create the listener directly
/// 2. If that fails with "address in use", checking if the socket is actually active
/// 3. Only removing the socket if it's confirmed to be stale (can't connect)
/// 4. Retrying with exponential backoff if another process races us
pub(crate) fn try_create_listener(socket_path: &Path) -> Result<Listener, IpcError> {
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
                        error = %e,
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
fn try_create_listener_once(socket_path: &Path) -> Result<Listener, IpcError> {
    let to_name = || {
        socket_path
            .to_fs_name::<GenericFilePath>()
            .map_err(|source| IpcError::InvalidSocketName {
                path: socket_path.to_path_buf(),
                source,
            })
    };
    let bind_error = |source| IpcError::Bind {
        path: socket_path.to_path_buf(),
        source,
    };

    // First attempt - try to create listener directly
    match ListenerOptions::new().name(to_name()?).create_sync() {
        Ok(listener) => return Ok(listener),
        Err(e) => {
            if !is_address_in_use(&e) {
                return Err(bind_error(e));
            }
            tracing::debug!("Socket exists, checking if it's stale");
        }
    }

    // Socket exists - check if it's active by trying to connect (with short timeout)
    let check_timeout = Duration::from_secs(1);
    if connect_with_timeout(socket_path, check_timeout).is_some() {
        // Another instance became primary between our initial check and listener creation.
        // This is a valid race during concurrent launches; the caller may choose to retry.
        return Err(IpcError::AnotherInstanceActive {
            path: socket_path.to_path_buf(),
        });
    }

    // Socket is stale - safe to remove (Unix only, Windows pipes auto-cleanup)
    #[cfg(unix)]
    {
        tracing::debug!(?socket_path, "Removing stale socket file");
        // Ignore remove error - another process may have already removed it (TOCTOU race)
        if let Err(e) = std::fs::remove_file(socket_path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!(?e, "Failed to remove stale socket file");
            }
        }
    }

    // Second attempt after removing stale socket
    ListenerOptions::new()
        .name(to_name()?)
        .create_sync()
        .map_err(bind_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn socket_path_is_user_isolated() {
        let path = socket_path();

        // Ensure the socket file name is exactly SOCKET_NAME
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("Socket path should have a valid UTF-8 file name");
        assert_eq!(file_name, SOCKET_NAME);

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
    fn is_address_in_use_ignores_unrelated_errors() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        assert!(!is_address_in_use(&err));
    }

    #[test]
    #[cfg(unix)]
    fn is_address_in_use_matches_eaddrinuse() {
        let err = std::io::Error::from_raw_os_error(libc::EADDRINUSE);
        assert!(is_address_in_use(&err));
    }
}
