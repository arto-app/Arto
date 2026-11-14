//! The primary instance's side: accept later launches and hand their
//! events to the app.

use crate::client::{APPLY_TIMEOUT, READY_TIMEOUT};
use crate::methods::{self, PeerInfo};
use crate::socket::{self, IpcError};
use crate::writer::Writer;
use crate::{framing, jsonrpc};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream};
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
    ///
    /// `on_subscribe` is handed the write half of a connection whose
    /// `initialize` asked to be told when something changes. One that did
    /// not ask is never passed on.
    pub fn serve(
        self,
        server_info: PeerInfo,
        on_request: impl Fn(methods::Call, Responder) + Send + Sync + 'static,
        on_subscribe: impl Fn(Writer) + Send + Sync + 'static,
    ) {
        let on_request: Arc<dyn Fn(methods::Call, Responder) + Send + Sync> = Arc::new(on_request);
        let on_subscribe: Arc<dyn Fn(Writer) + Send + Sync> = Arc::new(on_subscribe);

        for conn in self.listener.incoming() {
            let stream = match conn {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::debug!(%error, "Failed to accept IPC connection");
                    continue;
                }
            };

            let handler = Arc::clone(&on_request);
            let subscribe = Arc::clone(&on_subscribe);
            let info = server_info.clone();
            let spawned = std::thread::Builder::new()
                .name("ipc-client-handler".into())
                .spawn(move || {
                    handle_connection(stream, &info, handler.as_ref(), subscribe.as_ref())
                });
            if let Err(error) = spawned {
                // The closure, stream included, is consumed by the failed
                // spawn, so this connection is lost; the client sees its
                // write fail and falls back to becoming primary or retrying.
                tracing::debug!(%error, "Failed to spawn IPC client handler thread");
            }
        }
    }
}

/// One connection, read by its loop and written by whoever holds its
/// [`Writer`].
#[derive(Clone)]
struct SharedStream(Arc<Stream>);

impl std::io::Read for SharedStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl std::io::Write for SharedStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        (&*self.0).flush()
    }
}

/// Read one connection to its end.
fn handle_connection(
    stream: Stream,
    server_info: &PeerInfo,
    on_request: &dyn Fn(methods::Call, Responder),
    on_subscribe: &dyn Fn(Writer),
) {
    // Bounded until the peer has shown it speaks the protocol, so a
    // connection that never says anything does not hold a thread forever.
    if let Err(error) = socket::set_socket_timeout(&stream, socket::IPC_TIMEOUT) {
        tracing::debug!(%error, "Could not set the IPC socket timeout");
    }

    // Shared so a notification can be written while this loop is blocked on
    // a read; `&Stream` reads and writes, and one handle owned by the loop
    // could not be lent to another thread.
    let stream = SharedStream(Arc::new(stream));
    let writer = Writer::new(stream.clone());

    // An initialized client may legitimately have nothing to ask for a long
    // while. Writes stay bounded: a client that stops reading must not
    // stall whoever is writing to it.
    let allow_idle = {
        let stream = stream.clone();
        move || {
            if let Err(error) = socket::clear_read_timeout(&stream.0) {
                tracing::debug!(%error, "Could not lift the IPC read timeout");
            }
        }
    };

    let mut reader = std::io::BufReader::new(stream);
    handle_jsonrpc(
        &mut reader,
        &writer,
        server_info,
        on_request,
        on_subscribe,
        &allow_idle,
    );

    // The app may be holding a clone of this — a client that subscribed and
    // then left. Closing here gives the socket up now instead of at
    // whatever moment something is next written to it.
    writer.close();
}

/// Serve one connection's worth of JSON-RPC.
///
/// One request at a time, each answered before the next is read: a launch
/// sends one and waits, so there is nothing to gain from interleaving and a
/// great deal of clarity to lose.
fn handle_jsonrpc<S: std::io::Read>(
    reader: &mut std::io::BufReader<S>,
    writer: &Writer,
    server_info: &PeerInfo,
    on_request: &dyn Fn(methods::Call, Responder),
    on_subscribe: &dyn Fn(Writer),
    on_initialized: &dyn Fn(),
) {
    let mut lifecycle = Lifecycle::Fresh;

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
                writer.send(
                    &jsonrpc::Response::error(
                        jsonrpc::RequestId::Null,
                        jsonrpc::ResponseError::new(
                            jsonrpc::ErrorCode::ParseError,
                            error.to_string(),
                        ),
                    )
                    .into(),
                );
                continue;
            }
        };

        match message {
            jsonrpc::Message::Request(request) if request.method == methods::INITIALIZE => {
                let was_fresh = lifecycle == Lifecycle::Fresh;
                let (response, subscribe) = initialize(request, server_info, &mut lifecycle);
                if !writer.send(&response.into()) {
                    break;
                }
                if was_fresh && lifecycle == Lifecycle::Initialized {
                    on_initialized();
                }
                // After the response has gone out, never before. A
                // notification written first would be the first frame the
                // client reads, and a plain request/response client reads
                // that as a failed handshake.
                if subscribe {
                    on_subscribe(writer.clone());
                }
            }
            jsonrpc::Message::Request(request) => {
                let response = answer(request, &mut lifecycle, on_request);
                if !writer.send(&response.into()) {
                    break;
                }
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

/// How far through LSP's lifecycle this connection has come.
///
/// Both ends of it refuse requests, and for different reasons: before
/// `initialize` nothing has been negotiated, and after `shutdown` the
/// client has said it is leaving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Fresh,
    Initialized,
    ShuttingDown,
}

/// Answer `initialize`, and say whether this client asked to be told of
/// changes. Subscribing is the caller's to do, once the response is out.
fn initialize(
    request: jsonrpc::Request,
    server_info: &PeerInfo,
    lifecycle: &mut Lifecycle,
) -> (jsonrpc::Response, bool) {
    let id = request.id.clone();

    // Twice is a client that has lost track of its own connection, and
    // taking it would leave this end holding two subscriptions to the same
    // socket — every change written down it twice.
    if *lifecycle != Lifecycle::Fresh {
        let error = jsonrpc::ResponseError::new(
            jsonrpc::ErrorCode::InvalidRequest,
            format!("{} has already been answered", methods::INITIALIZE),
        );
        return (jsonrpc::Response::error(id, error), false);
    }

    let params = match request.params {
        // Parameters that cannot be read are refused rather than defaulted.
        // A client whose `stateChanges` did not parse would be told the
        // handshake succeeded and then wait forever for notifications
        // nobody agreed to send.
        Some(params) => match serde_json::from_value::<methods::InitializeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                let error = jsonrpc::ResponseError::new(
                    jsonrpc::ErrorCode::InvalidParams,
                    error.to_string(),
                );
                return (jsonrpc::Response::error(id, error), false);
            }
        },
        None => methods::InitializeParams::default(),
    };

    *lifecycle = Lifecycle::Initialized;
    // The one place the peer says which build it is. A launch that a
    // primary cannot understand is the failure this crate's upgrade note is
    // about, and without this line the log says nothing about who was on
    // the other end.
    tracing::debug!(client_info = ?params.client_info, "A client opened a connection");

    let result = methods::InitializeResult {
        server_info: server_info.clone(),
        capabilities: methods::ServerCapabilities::default(),
    };
    (
        result_or_internal_error(id, result),
        params.capabilities.state_changes,
    )
}

fn answer(
    request: jsonrpc::Request,
    lifecycle: &mut Lifecycle,
    on_request: &dyn Fn(methods::Call, Responder),
) -> jsonrpc::Response {
    let id = request.id.clone();

    if *lifecycle == Lifecycle::ShuttingDown {
        return jsonrpc::Response::error(
            id,
            jsonrpc::ResponseError::new(
                jsonrpc::ErrorCode::InvalidRequest,
                format!("{} has been requested", methods::SHUTDOWN),
            ),
        );
    }

    if *lifecycle == Lifecycle::Fresh {
        return jsonrpc::Response::error(
            id,
            jsonrpc::ResponseError::new(
                jsonrpc::ErrorCode::ServerNotInitialized,
                format!("{} must come first", methods::INITIALIZE),
            ),
        );
    }

    if request.method == methods::SHUTDOWN {
        *lifecycle = Lifecycle::ShuttingDown;
        return jsonrpc::Response::result(id, serde_json::Value::Null);
    }

    let call = match methods::Call::from_method(&request.method, request.params) {
        Ok(call) => call,
        Err(error) => return jsonrpc::Response::error(id, error),
    };

    tracing::debug!(method = %request.method, "Handing a JSON-RPC request to the app");

    // How long the app gets to answer. A request that asked to wait, or a
    // capture, is waiting on a window being drawn; every other one is waiting
    // only for the main thread to come round. The peer listens for longer than
    // either, so whatever is answered here is still being read.
    let wait_ready = call.wait_ready();
    let deadline = if call.waits_on_a_drawing() {
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

    /// The sending half, without a socket: what the server writes is
    /// collected here for the test to read back.
    #[derive(Clone, Default)]
    struct Collected(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Collected {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
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

    fn serve_once(incoming: Vec<u8>) -> (Vec<jsonrpc::Response>, Vec<crate::OpenEvent>) {
        let (responses, applied, _) = serve_once_subscribing(incoming);
        (responses, applied)
    }

    /// Drive a whole connection and answer with what the server wrote, what
    /// it handed the app, and every connection it offered to subscribe.
    ///
    /// The stand-in app answers every request at once, the way the real one
    /// does for anything that is not waiting on a window.
    fn serve_once_subscribing(
        incoming: Vec<u8>,
    ) -> (Vec<jsonrpc::Response>, Vec<crate::OpenEvent>, Vec<Writer>) {
        let applied = Mutex::new(Vec::new());
        let subscribed = Mutex::new(Vec::new());
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(incoming));
        let collected = Collected::default();
        let writer = Writer::new(collected.clone());

        handle_jsonrpc(
            &mut reader,
            &writer,
            &server_info(),
            &|call, responder| match call {
                methods::Call::Apply { event, wait_ready } => {
                    applied.lock().unwrap().push(event);
                    responder.ok(methods::AppliedResult { ready: wait_ready });
                }
                // No windows here to describe, so the shape is what is
                // under test rather than the contents.
                methods::Call::State(_) => responder.ok(methods::StateResult::default()),
                methods::Call::Goto(params) => responder.ok(methods::GotoResult {
                    window: params.window.unwrap_or(crate::WindowKey(1)),
                }),
                methods::Call::Selection(params) => responder.ok(methods::SelectionResult {
                    window: params.window.unwrap_or(crate::WindowKey(1)),
                    text: None,
                }),
                methods::Call::Capture(_) => responder.err(jsonrpc::ResponseError::new(
                    jsonrpc::ErrorCode::InternalError,
                    "nothing to draw here",
                )),
            },
            &|writer| subscribed.lock().unwrap().push(writer),
            &|| {},
        );

        let written = std::mem::take(&mut *collected.0.lock().unwrap());
        let mut responses = Vec::new();
        let mut cursor = std::io::BufReader::new(std::io::Cursor::new(written));
        while let Some(body) = framing::read_message(&mut cursor).unwrap() {
            if let jsonrpc::Message::Response(response) =
                jsonrpc::Message::from_bytes(&body).unwrap()
            {
                responses.push(response);
            }
        }
        (
            responses,
            applied.into_inner().unwrap(),
            subscribed.into_inner().unwrap(),
        )
    }

    fn initialize() -> Vec<u8> {
        framed(jsonrpc::Request::new(1, methods::INITIALIZE, None).into())
    }

    fn initialize_asking_for(capabilities: methods::ClientCapabilities) -> Vec<u8> {
        framed(
            jsonrpc::Request::new(
                1,
                methods::INITIALIZE,
                serde_json::to_value(methods::InitializeParams {
                    client_info: None,
                    capabilities,
                })
                .ok(),
            )
            .into(),
        )
    }

    #[test]
    fn a_client_that_asked_for_nothing_is_never_written_to_unasked() {
        let (_, _, subscribed) = serve_once_subscribing(initialize());
        assert!(subscribed.is_empty());
    }

    #[test]
    fn a_client_that_asked_to_be_told_of_changes_is_handed_its_write_half() {
        let (_, _, subscribed) =
            serve_once_subscribing(initialize_asking_for(methods::ClientCapabilities {
                state_changes: true,
            }));
        assert_eq!(subscribed.len(), 1);
    }

    #[test]
    fn a_subscriber_hears_the_handshake_before_anything_else() {
        // The write half reaches the app only once the response is on the
        // wire; a notification ahead of it would be read as a failed
        // handshake by a plain request/response client.
        let collected = Collected::default();
        let writer = Writer::new(collected.clone());
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(initialize_asking_for(
            methods::ClientCapabilities {
                state_changes: true,
            },
        )));
        let written_when_subscribed = Mutex::new(Vec::new());

        handle_jsonrpc(
            &mut reader,
            &writer,
            &server_info(),
            &|_, _| unreachable!("the handshake asks the app nothing"),
            &|_| {
                let so_far = collected.0.lock().unwrap().clone();
                *written_when_subscribed.lock().unwrap() = so_far;
            },
            &|| {},
        );

        let at_subscribe = written_when_subscribed.into_inner().unwrap();
        assert!(
            !at_subscribe.is_empty(),
            "the response had not been written yet"
        );
    }

    /// How many times a connection told its socket it may now stay idle.
    fn idle_allowances(incoming: Vec<u8>) -> usize {
        let allowed = std::sync::atomic::AtomicUsize::new(0);
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(incoming));
        handle_jsonrpc(
            &mut reader,
            &Writer::new(Collected::default()),
            &server_info(),
            &|_, responder| responder.ok(serde_json::Value::Null),
            &|_| {},
            &|| {
                allowed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            },
        );
        allowed.into_inner()
    }

    #[test]
    fn a_connection_may_stay_idle_once_it_has_initialized() {
        // A client that holds its connection open between requests — one
        // following the state, or an MCP server — would otherwise be cut
        // off the first time it had nothing to ask for a few seconds.
        assert_eq!(idle_allowances(initialize()), 1);
    }

    #[test]
    fn a_connection_that_never_initialized_stays_bounded() {
        let request = framed(jsonrpc::Request::new(1, methods::STATE, None).into());
        assert_eq!(idle_allowances(request), 0);
    }

    #[test]
    fn a_refused_initialize_does_not_lift_the_bound() {
        let incoming = framed(
            jsonrpc::Request::new(
                1,
                methods::INITIALIZE,
                Some(json!({"capabilities": {"stateChanges": "yes"}})),
            )
            .into(),
        );
        assert_eq!(idle_allowances(incoming), 0);
    }

    #[test]
    fn initialize_twice_is_refused_rather_than_subscribed_twice() {
        let mut incoming = initialize_asking_for(methods::ClientCapabilities {
            state_changes: true,
        });
        incoming.extend(initialize_asking_for(methods::ClientCapabilities {
            state_changes: true,
        }));

        let (responses, _, subscribed) = serve_once_subscribing(incoming);
        assert_eq!(
            responses[1].clone().into_result().unwrap_err().code,
            jsonrpc::ErrorCode::InvalidRequest as i64
        );
        assert_eq!(subscribed.len(), 1, "one connection, one subscription");
    }

    #[test]
    fn initialize_parameters_that_cannot_be_read_are_refused() {
        // Defaulting them would tell a client its handshake succeeded and
        // leave it waiting for notifications nobody agreed to send.
        let incoming = framed(
            jsonrpc::Request::new(
                1,
                methods::INITIALIZE,
                Some(json!({"capabilities": {"stateChanges": "true"}})),
            )
            .into(),
        );

        let (responses, _, subscribed) = serve_once_subscribing(incoming);
        assert_eq!(
            responses[0].clone().into_result().unwrap_err().code,
            jsonrpc::ErrorCode::InvalidParams as i64
        );
        assert!(subscribed.is_empty());
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
    fn shutdown_is_answered_and_then_nothing_else_is() {
        // An editor's client machinery ends a session this way. Answering
        // the `shutdown` and then refusing what follows is what tells it
        // the session is over, rather than leaving it to read a socket that
        // simply stops.
        let mut incoming = initialize();
        incoming.extend(framed(
            jsonrpc::Request::new(2, methods::SHUTDOWN, None).into(),
        ));
        incoming.extend(framed(
            jsonrpc::Request::new(3, methods::STATE, None).into(),
        ));

        let (responses, applied) = serve_once(incoming);
        assert_eq!(responses.len(), 3);
        assert_eq!(responses[1].clone().into_result().unwrap(), json!(null));
        assert_eq!(
            responses[2].clone().into_result().unwrap_err().code,
            jsonrpc::ErrorCode::InvalidRequest as i64
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
