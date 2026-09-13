//! The primary instance's side: accept later launches and hand their
//! events to the app.

use crate::client::{APPLY_TIMEOUT, READY_TIMEOUT};
use crate::methods::{self, PeerInfo};
use crate::socket::{self, IpcError};
use crate::{framing, jsonrpc};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;

/// The app's end of a request that is waiting for its answer.
///
/// Every request gets one. The connection thread is holding the socket open
/// on it, so an answer here is what the peer finally reads — which is why
/// the app, not this crate, decides what a request amounts to: only the app
/// knows whether the document opened, where the reader is, or what the
/// window has drawn.
///
/// Carried across threads and, for a launch waiting on a window, across an
/// arbitrary stretch of time. Dropping it without answering releases the
/// peer too, with an error: nothing that could still answer exists once the
/// last one is gone.
pub struct Responder(mpsc::Sender<Result<serde_json::Value, jsonrpc::ResponseError>>);

impl Responder {
    /// Answer the request with what it asked for.
    ///
    /// A value that cannot be serialized is answered as an internal error
    /// rather than dropped, so the peer hears something either way.
    pub fn ok<T: serde::Serialize>(self, value: T) {
        let answer = serde_json::to_value(value).map_err(|error| {
            jsonrpc::ResponseError::new(jsonrpc::ErrorCode::InternalError, error.to_string())
        });
        let _ = self.0.send(answer);
    }

    /// Answer the request with why it could not be carried out.
    pub fn err(self, error: jsonrpc::ResponseError) {
        let _ = self.0.send(Err(error));
    }
}

impl std::fmt::Debug for Responder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Responder")
    }
}

/// A bound listener, ready to accept secondary instances.
pub struct Server {
    listener: Listener,
    socket_path: PathBuf,
}

impl Server {
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

    /// Accept connections forever, calling `on_request` with what each one
    /// asks for, in order.
    ///
    /// Each connection is read on its own thread so a slow or stalled
    /// client cannot hold up the next one; reads time out after
    /// [`IPC_TIMEOUT`](crate::IPC_TIMEOUT). A connection that sends nothing
    /// parseable does not call `on_request` at all. Blocks the calling
    /// thread for the life of the listener.
    ///
    /// `on_request` is handed one request at a time, each answered before
    /// the next is read, and every one of them carries a [`Responder`]: this
    /// crate knows what was asked, and only the app knows the answer.
    ///
    /// `server_info` is what `initialize` reports back — the app's name and
    /// build, which this crate deliberately does not know.
    pub fn serve(
        self,
        server_info: PeerInfo,
        on_request: impl Fn(methods::Call, Responder) + Send + Sync + 'static,
    ) {
        let on_request: Arc<dyn Fn(methods::Call, Responder) + Send + Sync> = Arc::new(on_request);

        for conn in self.listener.incoming() {
            let stream = match conn {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::debug!(%error, "Failed to accept IPC connection");
                    continue;
                }
            };

            let handler = Arc::clone(&on_request);
            let info = server_info.clone();
            let spawned = std::thread::Builder::new()
                .name("ipc-client-handler".into())
                .spawn(move || handle_connection(stream, &info, handler.as_ref()));
            if let Err(error) = spawned {
                // The closure, stream included, is consumed by the failed
                // spawn, so this connection is lost; the client sees its
                // write fail and falls back to becoming primary or retrying.
                tracing::debug!(%error, "Failed to spawn IPC client handler thread");
            }
        }
    }
}

/// Read one connection to its end.
fn handle_connection(
    stream: Stream,
    server_info: &PeerInfo,
    on_request: &dyn Fn(methods::Call, Responder),
) {
    // Set read timeout to avoid blocking forever
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    let mut reader = std::io::BufReader::new(stream);
    handle_jsonrpc(&mut reader, server_info, on_request);
}

/// Serve one connection's worth of JSON-RPC.
///
/// One request at a time, each answered before the next is read: a launch
/// sends one and waits, so there is nothing to gain from interleaving and a
/// great deal of clarity to lose.
fn handle_jsonrpc<S: std::io::Read + Write>(
    reader: &mut std::io::BufReader<S>,
    server_info: &PeerInfo,
    on_request: &dyn Fn(methods::Call, Responder),
) {
    let mut initialized = false;

    loop {
        let body = match framing::read_message(reader) {
            Ok(Some(body)) => body,
            Ok(None) => break,
            Err(error) => {
                tracing::debug!(%error, "Error reading a framed IPC message");
                break;
            }
        };

        let message = match jsonrpc::Message::from_bytes(&body) {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%error, "Failed to parse a JSON-RPC message");
                // A message that could not be parsed has no id to answer to,
                // and JSON-RPC spells that absence as a null one.
                respond(
                    reader,
                    jsonrpc::Response::error(
                        jsonrpc::RequestId::Null,
                        jsonrpc::ResponseError::new(
                            jsonrpc::ErrorCode::ParseError,
                            error.to_string(),
                        ),
                    ),
                );
                continue;
            }
        };

        match message {
            jsonrpc::Message::Request(request) => {
                let response = answer(request, server_info, &mut initialized, on_request);
                respond(reader, response);
            }
            jsonrpc::Message::Notification(notification) => {
                tracing::debug!(method = %notification.method, "Received a JSON-RPC notification");
                if notification.method == methods::EXIT {
                    break;
                }
            }
            // Nothing here ever asks the launch a question, so an answer is
            // a message from a peer that has misunderstood which end it is.
            jsonrpc::Message::Response(response) => {
                tracing::debug!(?response.id, "Ignoring an unexpected JSON-RPC response");
            }
        }
    }
}

fn answer(
    request: jsonrpc::Request,
    server_info: &PeerInfo,
    initialized: &mut bool,
    on_request: &dyn Fn(methods::Call, Responder),
) -> jsonrpc::Response {
    let id = request.id.clone();

    if request.method == methods::INITIALIZE {
        *initialized = true;
        // The one place the peer says which build it is. A launch that a
        // primary cannot understand is the failure this crate's upgrade
        // note is about, and without this line the log says nothing about
        // who was on the other end.
        let client_info = request
            .params
            .and_then(|params| serde_json::from_value::<methods::InitializeParams>(params).ok())
            .and_then(|params| params.client_info);
        tracing::debug!(?client_info, "A client opened a connection");
        let result = methods::InitializeResult {
            server_info: server_info.clone(),
            capabilities: methods::ServerCapabilities::default(),
        };
        return result_or_internal_error(id, result);
    }

    if !*initialized {
        return jsonrpc::Response::error(
            id,
            jsonrpc::ResponseError::new(
                jsonrpc::ErrorCode::ServerNotInitialized,
                format!("{} must come first", methods::INITIALIZE),
            ),
        );
    }

    let call = match methods::Call::from_method(&request.method, request.params) {
        Ok(call) => call,
        Err(error) => return jsonrpc::Response::error(id, error),
    };

    tracing::debug!(method = %request.method, "Handing a JSON-RPC request to the app");

    // How long the app gets to answer. A request that asked to wait is
    // waiting on a window being drawn; every other one is waiting only for
    // the main thread to come round. The peer listens for longer than
    // either, so whatever is answered here is still being read.
    let wait_ready = call.wait_ready;
    let deadline = if wait_ready {
        READY_TIMEOUT
    } else {
        APPLY_TIMEOUT
    };

    let (sender, receiver) = mpsc::channel();
    on_request(call, Responder(sender));

    // The app's answer, not this crate's. Answering here before the main
    // thread had done anything is what made an `arto/open` report success
    // the instant it was queued — true of the queue, and nothing the peer
    // actually asked about.
    match receiver.recv_timeout(deadline) {
        Ok(Ok(value)) => jsonrpc::Response::result(id, value),
        Ok(Err(error)) => jsonrpc::Response::error(id, error),
        // Running out of time, and every responder being dropped, are the
        // same thing to the peer: no answer is coming. For a launch that
        // was waiting on a window, that *is* the answer — the window did
        // not draw — and saying so beats an error, which the launch would
        // report as the instance refusing a request it in fact carried out.
        Err(error) if wait_ready => {
            tracing::debug!(%error, "No window reported ready for a waiting launch");
            result_or_internal_error(id, methods::AppliedResult { ready: false })
        }
        Err(error) => {
            tracing::debug!(%error, "The app did not answer a request");
            jsonrpc::Response::error(
                id,
                jsonrpc::ResponseError::new(
                    jsonrpc::ErrorCode::InternalError,
                    "the running instance did not answer",
                ),
            )
        }
    }
}

/// Answer with the encoded result, or with an internal error when the
/// result itself could not be encoded — a request is always answered, and
/// never with a half-written one.
fn result_or_internal_error(
    id: jsonrpc::RequestId,
    result: impl serde::Serialize,
) -> jsonrpc::Response {
    match serde_json::to_value(result) {
        Ok(value) => jsonrpc::Response::result(id, value),
        Err(error) => jsonrpc::Response::error(
            id,
            jsonrpc::ResponseError::new(jsonrpc::ErrorCode::InternalError, error.to_string()),
        ),
    }
}

fn respond<S: std::io::Read + Write>(
    reader: &mut std::io::BufReader<S>,
    response: jsonrpc::Response,
) {
    let message = jsonrpc::Message::Response(response);
    let Ok(body) = message.to_bytes() else {
        return;
    };
    // Reading is buffered and writing is not, and the two directions of a
    // socket are independent, so writing through the reader's handle is
    // safe and saves splitting the stream.
    if let Err(error) = framing::write_message(reader.get_mut(), &body) {
        tracing::debug!(%error, "Could not write a JSON-RPC response");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{OpenRequest, WindowOptions};
    use serde_json::json;
    use std::sync::Mutex;

    /// A socket's two directions, without a socket: reads drain what the
    /// client "sent", writes collect what the server answered.
    struct Duplex {
        incoming: std::io::Cursor<Vec<u8>>,
        outgoing: Vec<u8>,
    }

    impl std::io::Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.incoming.read(buf)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.outgoing.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn framed(message: jsonrpc::Message) -> Vec<u8> {
        let mut out = Vec::new();
        framing::write_message(&mut out, &message.to_bytes().unwrap()).unwrap();
        out
    }

    fn server_info() -> PeerInfo {
        PeerInfo {
            name: "arto".into(),
            version: Some("test".into()),
        }
    }

    /// Drive a whole connection and answer with what the server wrote and
    /// what it handed the app.
    ///
    /// The stand-in app answers every request at once, the way the real one
    /// does for anything that is not waiting on a window.
    fn serve_once(incoming: Vec<u8>) -> (Vec<jsonrpc::Response>, Vec<crate::OpenEvent>) {
        let applied = Mutex::new(Vec::new());
        let mut reader = std::io::BufReader::new(Duplex {
            incoming: std::io::Cursor::new(incoming),
            outgoing: Vec::new(),
        });

        handle_jsonrpc(&mut reader, &server_info(), &|call, responder| {
            let waited = call.wait_ready;
            applied.lock().unwrap().push(call.event);
            responder.ok(methods::AppliedResult { ready: waited });
        });

        let written = std::mem::take(&mut reader.get_mut().outgoing);
        let mut responses = Vec::new();
        let mut cursor = std::io::BufReader::new(std::io::Cursor::new(written));
        while let Some(body) = framing::read_message(&mut cursor).unwrap() {
            if let jsonrpc::Message::Response(response) =
                jsonrpc::Message::from_bytes(&body).unwrap()
            {
                responses.push(response);
            }
        }
        (responses, applied.into_inner().unwrap())
    }

    fn initialize() -> Vec<u8> {
        framed(jsonrpc::Request::new(1, methods::INITIALIZE, None).into())
    }

    #[test]
    fn initialize_answers_with_what_this_instance_can_do() {
        let (responses, applied) = serve_once(initialize());
        assert_eq!(responses.len(), 1);
        let result = responses[0].clone().into_result().unwrap();
        assert_eq!(result["serverInfo"]["name"], "arto");
        assert_eq!(result["capabilities"]["waitReady"], true);
        assert!(applied.is_empty());
    }

    #[test]
    fn a_request_before_initialize_is_refused() {
        // The gate exists so a capability is never assumed: a client that
        // skipped the handshake never heard what this instance supports.
        let incoming = framed(jsonrpc::Request::new(1, methods::REOPEN, None).into());
        let (responses, applied) = serve_once(incoming);
        assert_eq!(
            responses[0].clone().into_result().unwrap_err().code,
            jsonrpc::ErrorCode::ServerNotInitialized as i64
        );
        assert!(applied.is_empty());
    }

    #[test]
    fn an_open_request_reaches_the_app_and_is_answered() {
        let mut incoming = initialize();
        incoming.extend(framed(
            jsonrpc::Request::new(
                2,
                methods::OPEN,
                Some(json!({"files": ["/README.md"], "directory": null, "behavior": null})),
            )
            .into(),
        ));

        let (responses, applied) = serve_once(incoming);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[1].id, jsonrpc::RequestId::Number(2));
        // Not waiting, so the answer says only that it was applied.
        assert_eq!(
            responses[1].clone().into_result().unwrap(),
            json!({"ready": false})
        );
        assert_eq!(
            applied,
            vec![crate::OpenEvent::Open(OpenRequest {
                files: vec!["/README.md".into()],
                directory: None,
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            })]
        );
    }

    #[test]
    fn a_request_that_waits_is_answered_once_the_window_reports() {
        let mut incoming = initialize();
        incoming.extend(framed(
            jsonrpc::Request::new(2, methods::REOPEN, Some(json!({"waitReady": true}))).into(),
        ));

        let (responses, _) = serve_once(incoming);
        assert_eq!(
            responses[1].clone().into_result().unwrap(),
            json!({"ready": true})
        );
    }

    #[test]
    fn an_unknown_method_is_answered_rather_than_dropped() {
        let mut incoming = initialize();
        incoming.extend(framed(
            jsonrpc::Request::new(2, "arto/levitate", None).into(),
        ));

        let (responses, applied) = serve_once(incoming);
        assert_eq!(
            responses[1].clone().into_result().unwrap_err().code,
            jsonrpc::ErrorCode::MethodNotFound as i64
        );
        assert!(applied.is_empty());
    }

    #[test]
    fn exit_closes_the_connection_and_nothing_else() {
        let mut incoming = initialize();
        incoming.extend(framed(
            jsonrpc::Notification::new(methods::EXIT, None).into(),
        ));
        // Anything after `exit` is never read, which is what makes it a
        // close rather than a pause.
        incoming.extend(framed(
            jsonrpc::Request::new(3, methods::REOPEN, None).into(),
        ));

        let (responses, applied) = serve_once(incoming);
        assert_eq!(responses.len(), 1, "only initialize was answered");
        assert!(applied.is_empty());
    }
}
