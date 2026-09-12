//! The primary instance's side: accept later launches and hand their
//! events to the app.

use crate::client::READY_TIMEOUT;
use crate::methods::{self, PeerInfo};
use crate::protocol::OpenEvent;
use crate::socket::{self, IpcError};
use crate::{framing, jsonrpc};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream};
use std::io::Write;
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

    /// Accept connections forever, calling `on_event` with what each one
    /// asks for, in order.
    ///
    /// Each connection is read on its own thread so a slow or stalled
    /// client cannot hold up the next one; reads time out after
    /// [`IPC_TIMEOUT`](crate::IPC_TIMEOUT). A connection that sends nothing
    /// parseable does not call `on_event` at all. Blocks the calling
    /// thread for the life of the listener.
    ///
    /// `on_event` is handed one request at a time, each answered before the
    /// next is read. Its second argument is `Some` only for a launch that
    /// asked to wait for its window; firing it releases that launch. See
    /// [`ReadySignal`].
    ///
    /// `server_info` is what `initialize` reports back — the app's name and
    /// build, which this crate deliberately does not know.
    pub fn serve(
        self,
        server_info: PeerInfo,
        on_event: impl Fn(OpenEvent, Option<ReadySignal>) + Send + Sync + 'static,
    ) {
        let on_event: Arc<dyn Fn(OpenEvent, Option<ReadySignal>) + Send + Sync> =
            Arc::new(on_event);

        for conn in self.listener.incoming() {
            let stream = match conn {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::debug!(%error, "Failed to accept IPC connection");
                    continue;
                }
            };

            let handler = Arc::clone(&on_event);
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
    on_event: &dyn Fn(OpenEvent, Option<ReadySignal>),
) {
    // Set read timeout to avoid blocking forever
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    let mut reader = std::io::BufReader::new(stream);
    handle_jsonrpc(&mut reader, server_info, on_event);
}

/// Serve one connection's worth of JSON-RPC.
///
/// One request at a time, each answered before the next is read: a launch
/// sends one and waits, so there is nothing to gain from interleaving and a
/// great deal of clarity to lose.
fn handle_jsonrpc<S: std::io::Read + Write>(
    reader: &mut std::io::BufReader<S>,
    server_info: &PeerInfo,
    on_event: &dyn Fn(OpenEvent, Option<ReadySignal>),
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
                let response = answer(request, server_info, &mut initialized, on_event);
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
    on_event: &dyn Fn(OpenEvent, Option<ReadySignal>),
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

    tracing::debug!(method = %request.method, "Applying a JSON-RPC request");

    if !call.wait_ready {
        on_event(call.event, None);
        return applied(id, false);
    }

    let (sender, receiver) = mpsc::channel();
    on_event(call.event, Some(ReadySignal(sender)));
    let ready = match receiver.recv_timeout(READY_TIMEOUT) {
        Ok(()) => true,
        // Disconnected means every signal was dropped, so no answer is ever
        // coming; saying so at once beats holding the launch for the full
        // timeout to reach the same place.
        Err(error) => {
            tracing::debug!(%error, "No window reported ready for a waiting launch");
            false
        }
    };
    applied(id, ready)
}

fn applied(id: jsonrpc::RequestId, ready: bool) -> jsonrpc::Response {
    result_or_internal_error(id, methods::AppliedResult { ready })
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
    fn serve_once(incoming: Vec<u8>) -> (Vec<jsonrpc::Response>, Vec<OpenEvent>) {
        let applied = Mutex::new(Vec::new());
        let mut reader = std::io::BufReader::new(Duplex {
            incoming: std::io::Cursor::new(incoming),
            outgoing: Vec::new(),
        });

        handle_jsonrpc(&mut reader, &server_info(), &|event, ready| {
            applied.lock().unwrap().push(event);
            // Nothing here has a window, so a launch that asked to wait is
            // told so at once rather than left for the timeout.
            if let Some(ready) = ready {
                ready.fire();
            }
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
            vec![OpenEvent::Open(OpenRequest {
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
