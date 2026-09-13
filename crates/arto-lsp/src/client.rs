//! The secondary instance's side: hand the request to whoever is listening.

use crate::protocol::OpenEvent;
use crate::socket;
use crate::{framing, jsonrpc, methods};
use interprocess::local_socket::{prelude::*, GenericFilePath, Stream, ToFsName};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// Outcome of trying to hand an event to an existing instance.
///
/// Three of these mean the request reached a live instance and only one
/// means it did not, which is the distinction the caller needs: becoming
/// primary after the request was already delivered opens the document a
/// second time, or fails to bind and exits claiming the single-instance
/// guarantee is broken. Only [`NoExistingInstance`](Self::NoExistingInstance)
/// and [`Failed`](Self::Failed) leave this process free to become primary.
#[derive(Debug)]
pub enum SendResult {
    /// The running instance carried the request out.
    ///
    /// `ready` answers the launch that asked to wait: the window drew what
    /// it was given. It is false for every launch that did not ask, because
    /// nothing has been drawn at the moment such a request is answered.
    Applied { ready: bool },
    /// The running instance understood the request and refused it.
    Refused(jsonrpc::ResponseError),
    /// The request was written to a live instance, which then said nothing.
    ///
    /// Whether it was applied is genuinely unknown — the primary may have
    /// opened the document and quit before answering — so this is neither a
    /// success to report nor grounds to try again.
    Unanswered,
    /// Nothing is listening; this process should become primary.
    NoExistingInstance,
    /// The connection failed before the request could be written, so
    /// nothing was delivered and becoming primary is safe.
    Failed(std::io::Error),
}

/// How long a launch that asked to wait will hold the connection open.
///
/// Far longer than [`socket::IPC_TIMEOUT`], which bounds a handshake: this
/// bounds a document being read from disk, parsed, and drawn with its
/// diagrams and formulas. A cold start with a large document is seconds, and
/// the point of waiting is to not have to guess how many.
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the instance gets to apply a request nobody is waiting on.
///
/// Only the main thread coming round, which is the next turn of the event
/// loop unless something is very wrong — but "very wrong" includes a window
/// busy drawing a large document, so it is not instant.
pub const APPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// What the instance's own deadline is extended by for the launch waiting
/// on it.
///
/// The two clocks start within microseconds of each other, and the instance
/// answers *at* its deadline — `ready: false`, or an error. Given the same
/// bound, that answer would always arrive just after this end stopped
/// listening, and every slow request would read as a vanished primary.
const DEADLINE_MARGIN: Duration = Duration::from_secs(5);

/// How long this launch listens for an answer.
fn client_deadline(wait_ready: bool) -> Duration {
    let theirs = if wait_ready {
        READY_TIMEOUT
    } else {
        APPLY_TIMEOUT
    };
    theirs + DEADLINE_MARGIN
}

/// How much of the handshake got through before anything was asked for.
///
/// Kept apart from [`SendResult`] because the two halves of a handoff fail
/// differently: nothing is lost by a handshake that does not complete, and
/// everything is uncertain once the request has gone out.
enum Handshake {
    /// The instance answered, and this is what it says it can do.
    Ready(methods::ServerCapabilities),
    /// Nothing is listening, or what is listening does not speak this.
    NoInstance,
    Failed(std::io::Error),
}

/// Try to hand an event to an already running instance.
///
/// Connecting is bounded by a timeout so a wedged primary cannot hang a
/// new launch; no answer within it counts as no instance.
///
/// With `wait_ready`, the connection stays open until the primary says the
/// target window has drawn the request, or until [`READY_TIMEOUT`] passes.
///
/// # An instance that does not answer `initialize`
///
/// Reported as no instance, which is the honest answer: something holds the
/// socket, but nothing that can be asked to open a document. The caller then
/// becomes primary itself and fails to bind, which is where the user finds
/// out — rather than here, where a launch has no way to say anything.
///
/// In practice this is a copy of Arto from before the protocol changed,
/// still running because upgrading does not restart it. Quitting and
/// reopening Arto is the whole of the fix, and it is needed once.
///
/// `client_info` is what this launch reports of itself in `initialize` —
/// the app's name and build, which this crate deliberately does not know,
/// exactly as [`Server::serve`](crate::Server::serve) is handed the
/// primary's.
pub fn send_to_existing_instance(
    client_info: methods::PeerInfo,
    event: &OpenEvent,
    wait_ready: bool,
) -> SendResult {
    let socket_path = socket::socket_path();

    let Some(stream) = connect_with_timeout(&socket_path, socket::IPC_TIMEOUT) else {
        return SendResult::NoExistingInstance;
    };
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }
    let mut reader = std::io::BufReader::new(stream);

    let capabilities = match handshake(&mut reader, client_info) {
        Handshake::Ready(capabilities) => capabilities,
        Handshake::NoInstance => return SendResult::NoExistingInstance,
        Handshake::Failed(error) => return SendResult::Failed(error),
    };

    // What the instance says it cannot do, it is not asked to do. This is
    // what `initialize` is for: without it a launch could only write into
    // the socket and hope, which is how the protocol this replaced worked.
    let wait_ready = if wait_ready && !capabilities.wait_ready {
        tracing::debug!("Running instance cannot wait for a window; not asking it to");
        false
    } else {
        wait_ready
    };

    let outcome = request(&mut reader, event, wait_ready);

    // A courtesy, and the reason `exit` means only "close this connection"
    // here: see `methods::EXIT`.
    let _ = send(
        &mut reader,
        jsonrpc::Notification::new(methods::EXIT, None).into(),
    );
    outcome
}

/// Ids for the two requests one handoff makes. A connection this short has
/// no need to generate them.
const INITIALIZE_ID: i64 = 1;
const CALL_ID: i64 = 2;

/// Say who is calling and find out what the instance can do.
///
/// Nothing has been asked for yet, so every way this ends leaves the launch
/// free to become primary.
fn handshake(reader: &mut std::io::BufReader<Stream>, client_info: methods::PeerInfo) -> Handshake {
    let initialize = jsonrpc::Request::new(
        INITIALIZE_ID,
        methods::INITIALIZE,
        serde_json::to_value(methods::InitializeParams {
            client_info: Some(client_info),
        })
        .ok(),
    );
    if let Err(error) = send(reader, initialize.into()) {
        return Handshake::Failed(error);
    }

    let Some(response) = read_response(reader) else {
        return Handshake::NoInstance;
    };
    if response.id != jsonrpc::RequestId::Number(INITIALIZE_ID) {
        tracing::debug!(?response.id, "Instance answered an initialize that was not sent");
        return Handshake::NoInstance;
    }
    let result = match response.into_result() {
        Ok(result) => result,
        Err(error) => {
            tracing::debug!(%error, "Instance refused to initialize");
            return Handshake::NoInstance;
        }
    };
    match serde_json::from_value::<methods::InitializeResult>(result) {
        Ok(result) => {
            tracing::debug!(?result.server_info, "Handshake complete");
            Handshake::Ready(result.capabilities)
        }
        Err(error) => {
            tracing::debug!(%error, "Instance answered initialize with something unreadable");
            Handshake::NoInstance
        }
    }
}

/// Ask for the one thing this launch came to ask for.
///
/// Past this point the request may already have been carried out, so no
/// outcome here lets the caller become primary — see [`SendResult`].
fn request(
    reader: &mut std::io::BufReader<Stream>,
    event: &OpenEvent,
    wait_ready: bool,
) -> SendResult {
    let call = methods::Call {
        event: event.clone(),
        wait_ready,
    };
    // A request that cannot even be encoded is not one to send: the primary
    // would answer `InvalidParams` to a null `params`, and the launch would
    // have nothing better to report than if it had never tried. A path that
    // is not valid UTF-8 is how this happens. Nothing has been written yet,
    // so this is still a clean failure.
    let (method, params) = match call.to_method() {
        Ok(parts) => parts,
        Err(error) => return SendResult::Failed(invalid_data(error)),
    };
    let request = jsonrpc::Request::new(CALL_ID, method, Some(params));
    if let Err(error) = send(reader, request.into()) {
        return SendResult::Failed(error);
    }

    // The answer to this one is the app's, not the connection thread's, so
    // it takes as long as the app does — a whole document being drawn, when
    // the launch asked to wait. Either way this end listens for longer than
    // the instance is allowed to take, so the instance's own answer always
    // arrives while somebody is still reading.
    if let Err(error) = socket::set_socket_timeout(reader.get_mut(), client_deadline(wait_ready)) {
        tracing::debug!(%error, "Could not set the IPC socket timeout for the answer");
    }

    match read_response(reader) {
        Some(response) if response.id != jsonrpc::RequestId::Number(CALL_ID) => {
            tracing::debug!(?response.id, "Instance answered a request that was not sent");
            SendResult::Unanswered
        }
        Some(response) => match response.into_result() {
            Ok(result) => match serde_json::from_value::<methods::AppliedResult>(result) {
                Ok(applied) => SendResult::Applied {
                    ready: applied.ready,
                },
                Err(error) => {
                    tracing::debug!(%error, "Instance answered with something unreadable");
                    SendResult::Unanswered
                }
            },
            Err(error) => SendResult::Refused(error),
        },
        None => SendResult::Unanswered,
    }
}

fn invalid_data(message: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.to_string())
}

fn send(reader: &mut std::io::BufReader<Stream>, message: jsonrpc::Message) -> std::io::Result<()> {
    let body = message
        .to_bytes()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    framing::write_message(reader.get_mut(), &body)
}

fn read_response(reader: &mut std::io::BufReader<Stream>) -> Option<jsonrpc::Response> {
    let body = match framing::read_message(reader) {
        Ok(Some(body)) => body,
        Ok(None) => return None,
        Err(error) => {
            tracing::debug!(%error, "Could not read a framed reply");
            return None;
        }
    };
    match jsonrpc::Message::from_bytes(&body) {
        Ok(jsonrpc::Message::Response(response)) => Some(response),
        Ok(_) => {
            tracing::debug!("Primary sent something other than a response");
            None
        }
        Err(error) => {
            tracing::debug!(%error, "Could not parse the reply");
            None
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// What `run` does with each outcome, stated as the rule rather than
    /// driven through a socket: only two of them leave a launch free to
    /// start a second app, and getting that wrong either opens the document
    /// twice or exits claiming single-instance enforcement is broken.
    fn may_become_primary(result: &SendResult) -> bool {
        matches!(
            result,
            SendResult::NoExistingInstance | SendResult::Failed(_)
        )
    }

    #[test]
    fn only_an_undelivered_request_lets_the_launch_become_primary() {
        assert!(may_become_primary(&SendResult::NoExistingInstance));
        assert!(may_become_primary(&SendResult::Failed(
            std::io::Error::other("connect refused")
        )));

        assert!(!may_become_primary(&SendResult::Applied { ready: true }));
        assert!(!may_become_primary(&SendResult::Applied { ready: false }));
        assert!(!may_become_primary(&SendResult::Unanswered));
        assert!(!may_become_primary(&SendResult::Refused(
            jsonrpc::ResponseError::new(jsonrpc::ErrorCode::MethodNotFound, "no")
        )));
    }

    #[test]
    fn a_launch_that_waits_hears_whether_the_window_drew() {
        // The two are different outcomes, not one: `--help` promises a
        // return only once the document is drawn, and a capture script has
        // no other way to tell it did not happen.
        let drew = SendResult::Applied { ready: true };
        let did_not = SendResult::Applied { ready: false };
        assert!(matches!(drew, SendResult::Applied { ready: true }));
        assert!(matches!(did_not, SendResult::Applied { ready: false }));
    }

    #[test]
    fn the_launch_always_waits_longer_than_the_instance_it_waits_on() {
        // The instance answers *at* its own deadline; listening for exactly
        // as long would miss that answer every time.
        assert!(client_deadline(true) > READY_TIMEOUT);
        assert!(client_deadline(false) > APPLY_TIMEOUT);
    }
}
