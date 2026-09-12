//! JSON-RPC 2.0, as much of it as a single-instance handoff needs.
//!
//! Not a dependency on a general JSON-RPC crate, because what travels this
//! socket is a handful of methods between two halves of one binary: the
//! parts that earn their place are the envelope, the id, and the error
//! object, and those are small enough to spell out and test.
//!
//! The envelope is what the older protocol lacked. A line-delimited `open`
//! could only be written and forgotten; a request carries an id, so an
//! answer can be matched to it — which is what lets a launch be told the
//! window is up rather than guessing with a sleep, and what anything that
//! wants to *ask* Arto something will need.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The only version this speaks.
pub const VERSION: &str = "2.0";

/// What a request is answered by.
///
/// Numbers and strings are both allowed by JSON-RPC and a peer may use
/// either, so both are carried rather than normalized: an id is echoed back
/// exactly as it arrived or the sender cannot match it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
    /// No id, which JSON-RPC requires of an error answering a message that
    /// could not be parsed far enough to have one.
    ///
    /// A variant rather than a stand-in number: answering an unparseable
    /// message with id `0` is a claim about a request the peer may really
    /// have sent, and a peer that numbers from zero would match that error
    /// to a request that is still outstanding.
    Null,
}

impl From<i64> for RequestId {
    fn from(id: i64) -> Self {
        Self::Number(id)
    }
}

/// A call that expects an answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    pub fn new(id: impl Into<RequestId>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

/// A call that expects nothing back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Notification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

/// The answer to a [`Request`], carrying a result or an error but never
/// both — which is why they are one field of an enum rather than two
/// `Option`s that could disagree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Result(Value),
    Error(ResponseError),
}

impl Response {
    pub fn result(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            id,
            outcome: Outcome::Result(result),
        }
    }

    pub fn error(id: RequestId, error: ResponseError) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            id,
            outcome: Outcome::Error(error),
        }
    }

    /// The result, or the error the peer answered with.
    pub fn into_result(self) -> Result<Value, ResponseError> {
        match self.outcome {
            Outcome::Result(value) => Ok(value),
            Outcome::Error(error) => Err(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ResponseError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code: code as i64,
            message: message.into(),
            data: None,
        }
    }
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

/// The JSON-RPC codes this uses, plus LSP's own `ServerNotInitialized`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    /// LSP's own: a method arrived before `initialize` established what the
    /// two ends can say to each other.
    ServerNotInitialized = -32002,
}

/// Anything that can arrive on the wire.
///
/// Ordered so that the most specific shape is tried first: a response has an
/// id and no method, a request has both, a notification has a method and no
/// id. Serde's untagged matching takes the first that fits, and a request
/// would otherwise be read as a notification with its id ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Response(Response),
    Request(Request),
    Notification(Notification),
}

impl Message {
    /// Read one message from a framed body.
    pub fn from_bytes(body: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(body)
    }

    /// The bytes of one framed body.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

impl From<Request> for Message {
    fn from(request: Request) -> Self {
        Self::Request(request)
    }
}

impl From<Notification> for Message {
    fn from(notification: Notification) -> Self {
        Self::Notification(notification)
    }
}

impl From<Response> for Message {
    fn from(response: Response) -> Self {
        Self::Response(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn round_trip(message: Message) -> Message {
        Message::from_bytes(&message.to_bytes().unwrap()).unwrap()
    }

    #[test]
    fn a_request_carries_its_version_id_and_method() {
        let request = Request::new(1, "arto/open", Some(json!({"files": []})));
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "arto/open",
                "params": {"files": []},
            })
        );
    }

    #[test]
    fn params_are_left_out_when_there_are_none() {
        let notification = Notification::new("exit", None);
        assert_eq!(
            serde_json::to_value(&notification).unwrap(),
            json!({"jsonrpc": "2.0", "method": "exit"})
        );
    }

    #[test]
    fn a_result_and_an_error_are_never_both_present() {
        assert_eq!(
            serde_json::to_value(Response::result(
                RequestId::Number(1),
                json!({"ready": true})
            ))
            .unwrap(),
            json!({"jsonrpc": "2.0", "id": 1, "result": {"ready": true}})
        );
        assert_eq!(
            serde_json::to_value(Response::error(
                RequestId::Number(1),
                ResponseError::new(ErrorCode::MethodNotFound, "no such method"),
            ))
            .unwrap(),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": {"code": -32601, "message": "no such method"},
            })
        );
    }

    #[test]
    fn each_shape_is_read_back_as_itself() {
        // The three shapes differ only by which fields are present, so the
        // order they are tried in is load-bearing; this is the test that
        // catches a reordering.
        assert!(matches!(
            round_trip(Request::new(1, "initialize", None).into()),
            Message::Request(_)
        ));
        assert!(matches!(
            round_trip(Notification::new("initialized", None).into()),
            Message::Notification(_)
        ));
        assert!(matches!(
            round_trip(Response::result(RequestId::Number(1), json!(null)).into()),
            Message::Response(_)
        ));
    }

    #[test]
    fn an_id_comes_back_exactly_as_it_arrived() {
        // A peer may number its requests or name them; either way the answer
        // has to carry the id it was asked with or nothing can be matched.
        for id in [RequestId::Number(7), RequestId::String("seven".into())] {
            let echoed = round_trip(Response::result(id.clone(), json!(null)).into());
            let Message::Response(response) = echoed else {
                panic!("a response reads back as a response");
            };
            assert_eq!(response.id, id);
        }
    }

    #[test]
    fn an_unparseable_message_is_answered_with_no_id_at_all() {
        let response = Response::error(
            RequestId::Null,
            ResponseError::new(ErrorCode::ParseError, "not JSON"),
        );
        assert_eq!(
            serde_json::to_value(&response).unwrap()["id"],
            serde_json::Value::Null
        );
        // And it comes back as an absence rather than as some number the
        // peer might still be waiting on.
        let Message::Response(back) = round_trip(response.into()) else {
            panic!("a response reads back as a response");
        };
        assert_eq!(back.id, RequestId::Null);
    }

    #[test]
    fn an_error_response_reads_back_as_an_error() {
        let response = Response::error(
            RequestId::Number(1),
            ResponseError::new(ErrorCode::InvalidParams, "files must be a list"),
        );
        let error = response.into_result().unwrap_err();
        assert_eq!(error.code, -32602);
        assert_eq!(error.to_string(), "files must be a list (code -32602)");
    }
}
