//! The end of a connection that more than one thread writes to.
//!
//! A mutex rather than a channel and a pump thread: what has to be said is
//! that one frame's header and body may not interleave with another's, and
//! a thread whose only work is to hold that exclusion buys nothing.

use crate::framing;
use crate::jsonrpc;
use std::sync::{Arc, Mutex};

/// A connection's write half. Every clone writes to the same socket.
///
/// The app may hold a clone long after the connection is gone, so the
/// socket is given up by the server when the connection ends
/// rather than when the last clone drops. Otherwise a client that
/// subscribed and left would keep its descriptor open until something
/// happened to be written to it.
#[derive(Clone)]
pub struct Writer(Arc<Mutex<Option<Box<dyn std::io::Write + Send>>>>);

impl Writer {
    pub(crate) fn new(sink: impl std::io::Write + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Some(Box::new(sink)))))
    }

    /// Write one message, whole.
    ///
    /// Failure is `false` rather than an error to carry: every caller's
    /// answer to a peer that has gone away is to stop writing to it.
    pub fn send(&self, message: &jsonrpc::Message) -> bool {
        let Ok(body) = message.to_bytes() else {
            tracing::debug!("Could not encode a message to send");
            return false;
        };
        // A poisoned lock means a thread panicked mid-frame, so what is on
        // the wire is already half a message.
        let Ok(mut stream) = self.0.lock() else {
            tracing::debug!("Connection writer was poisoned; giving up on it");
            return false;
        };
        let Some(stream) = stream.as_mut() else {
            return false;
        };
        match framing::write_message(stream, &body) {
            Ok(()) => true,
            Err(error) => {
                tracing::debug!(%error, "Could not write to an IPC connection");
                false
            }
        }
    }

    /// Tell the peer something it did not ask for.
    pub fn notify(&self, method: &str, params: impl serde::Serialize) -> bool {
        let params = match serde_json::to_value(params) {
            Ok(params) => Some(params),
            Err(error) => {
                tracing::debug!(%error, "Could not encode notification parameters");
                return false;
            }
        };
        self.send(&jsonrpc::Notification::new(method, params).into())
    }

    /// Whether this end is still worth writing to.
    ///
    /// Only says the connection has not been closed; a peer can still go
    /// away between the question and the write, which is what [`send`]'s
    /// `false` is for.
    ///
    /// [`send`]: Self::send
    pub fn is_open(&self) -> bool {
        self.0.lock().is_ok_and(|stream| stream.is_some())
    }

    /// Give up the socket. Every clone is closed with it, and nothing more
    /// is written through any of them.
    pub(crate) fn close(&self) {
        if let Ok(mut stream) = self.0.lock() {
            *stream = None;
        }
    }
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Writer")
    }
}
