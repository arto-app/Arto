//! What the two ends actually say to each other.
//!
//! The lifecycle is LSP's — `initialize`, then anything else — with one
//! deliberate departure spelled out at [`EXIT`]. The methods themselves are
//! Arto's, and they carry the same [`OpenRequest`] and [`WindowOptions`] the
//! line-delimited protocol carried: what changed is the envelope, not what a
//! launch is asking for.

use crate::jsonrpc::{ErrorCode, ResponseError};
use crate::protocol::{OpenEvent, OpenRequest, WindowOptions};
use arto_config::FileOpenBehavior;
use serde::{Deserialize, Serialize};

/// Establish what the two ends can say to each other. Must come first.
pub const INITIALIZE: &str = "initialize";
/// The client is ready. A notification, as in LSP.
pub const INITIALIZED: &str = "initialized";
/// Open files, a directory, or both.
pub const OPEN: &str = "arto/open";
/// Bring a window forward, or place and repaint one, without opening
/// anything.
pub const REOPEN: &str = "arto/reopen";
/// End this connection.
///
/// **This is where Arto departs from LSP.** There, `exit` ends the server
/// process; here it ends only the connection that sent it. A launch handing
/// over a path is a client for the length of one request, and letting any
/// such client quit the application the user is reading in would turn a
/// stray message into lost work. Nothing on this socket can close Arto.
pub const EXIT: &str = "exit";

/// What a client says about itself when it opens a connection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<PeerInfo>,
}

/// What the running instance says about itself in return.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub server_info: PeerInfo,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// What this instance can be asked for.
///
/// Negotiated rather than assumed, which is the point of having an
/// `initialize` at all: the older protocol answered "can you do this?" by
/// accumulating message shapes that had to be understood forever. A client
/// meeting an instance that lacks a capability can say so, or fall back,
/// instead of writing into a socket and hoping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    pub open: bool,
    pub reopen: bool,
    /// Whether a request may ask to be answered only once the window has
    /// drawn, rather than as soon as the request is applied.
    pub wait_ready: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            open: true,
            reopen: true,
            wait_ready: true,
        }
    }
}

/// The parameters of [`OPEN`].
///
/// [`OpenRequest`]'s own fields are flattened in, so the document half of
/// the message is the same object the line-delimited protocol used and is
/// read by the same type. `waitReady` sits beside them because it says when
/// the *response* comes rather than anything about what to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenParams {
    #[serde(flatten)]
    pub request: OpenRequest,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wait_ready: bool,
}

/// The parameters of [`REOPEN`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReopenParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior: Option<FileOpenBehavior>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub behind: bool,
    #[serde(default, skip_serializing_if = "WindowOptions::is_empty")]
    pub window: WindowOptions,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wait_ready: bool,
}

/// What [`OPEN`] and [`REOPEN`] answer with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedResult {
    /// The window drew what was asked for.
    ///
    /// Only ever true for a request that asked to wait; one that did not is
    /// answered as soon as it is applied, and at that moment nothing has
    /// been drawn yet.
    pub ready: bool,
}

/// One side of the wire, as the app's own handler sees it: the event to
/// apply, and whether the answer waits for the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub event: OpenEvent,
    pub wait_ready: bool,
}

impl Call {
    /// Read a call out of a method name and its parameters.
    ///
    /// Absent parameters are an empty object rather than an error: `reopen`
    /// with nothing to say is a real request — it is what the dock icon
    /// amounts to — and spelling `{}` should not be required to make it.
    pub fn from_method(
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<Self, ResponseError> {
        let params = params.unwrap_or(serde_json::Value::Object(Default::default()));
        match method {
            OPEN => {
                let params: OpenParams = parse_params(params)?;
                Ok(Self {
                    event: OpenEvent::Open(params.request),
                    wait_ready: params.wait_ready,
                })
            }
            REOPEN => {
                let params: ReopenParams = parse_params(params)?;
                Ok(Self {
                    event: OpenEvent::Reopen {
                        behavior: params.behavior,
                        behind: params.behind,
                        window: params.window,
                    },
                    wait_ready: params.wait_ready,
                })
            }
            other => Err(ResponseError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method: {other}"),
            )),
        }
    }

    /// The method and parameters that carry this call.
    ///
    /// Fallible because a [`PathBuf`](std::path::PathBuf) that is not valid
    /// UTF-8 cannot be written as JSON. Answering that with `null` params
    /// would send a request the primary is bound to refuse, and the refusal
    /// would say `InvalidParams` rather than what was actually wrong.
    pub fn to_method(&self) -> Result<(&'static str, serde_json::Value), serde_json::Error> {
        match &self.event {
            OpenEvent::Open(request) => Ok((
                OPEN,
                serde_json::to_value(OpenParams {
                    request: request.clone(),
                    wait_ready: self.wait_ready,
                })?,
            )),
            OpenEvent::Reopen {
                behavior,
                behind,
                window,
            } => Ok((
                REOPEN,
                serde_json::to_value(ReopenParams {
                    behavior: *behavior,
                    behind: *behind,
                    window: *window,
                    wait_ready: self.wait_ready,
                })?,
            )),
        }
    }
}

fn parse_params<T: serde::de::DeserializeOwned>(
    params: serde_json::Value,
) -> Result<T, ResponseError> {
    serde_json::from_value(params)
        .map_err(|error| ResponseError::new(ErrorCode::InvalidParams, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{WindowExtent, WindowPoint};
    use arto_config::Theme;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn an_open_call_round_trips_through_its_method_and_params() {
        let call = Call {
            event: OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/README.md")],
                directory: Some(PathBuf::from("/docs")),
                behavior: Some(FileOpenBehavior::NewWindow),
                behind: false,
                window: WindowOptions {
                    position: Some(WindowPoint { x: 120, y: 64 }),
                    size: Some(WindowExtent {
                        width: 1400,
                        height: 920,
                    }),
                    theme: Some(Theme::Light),
                },
            }),
            wait_ready: true,
        };

        let (method, params) = call.to_method().unwrap();
        assert_eq!(method, OPEN);
        assert_eq!(Call::from_method(method, Some(params)).unwrap(), call);
    }

    #[test]
    fn a_reopen_call_round_trips_through_its_method_and_params() {
        let call = Call {
            event: OpenEvent::Reopen {
                behavior: Some(FileOpenBehavior::LastFocused),
                behind: true,
                window: WindowOptions {
                    theme: Some(Theme::Dark),
                    ..Default::default()
                },
            },
            wait_ready: false,
        };

        let (method, params) = call.to_method().unwrap();
        assert_eq!(method, REOPEN);
        assert_eq!(Call::from_method(method, Some(params)).unwrap(), call);
    }

    #[test]
    fn the_document_half_is_the_object_it_always_was() {
        // `OpenRequest` is flattened rather than nested, so the parameters
        // of `arto/open` read the same as the body of the line-delimited
        // `open` message — one shape, one type, one set of defaults.
        let call = Call {
            event: OpenEvent::Open(OpenRequest {
                files: vec![PathBuf::from("/a.md")],
                directory: None,
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            }),
            wait_ready: false,
        };
        let (_, params) = call.to_method().unwrap();
        assert_eq!(
            params,
            json!({"files": ["/a.md"], "directory": null, "behavior": null, "behind": false})
        );
    }

    #[test]
    fn a_reopen_with_nothing_to_say_needs_no_parameters() {
        // What the dock icon amounts to. Requiring `{}` would make the
        // simplest call the one with the most ceremony.
        let call = Call::from_method(REOPEN, None).unwrap();
        assert_eq!(
            call,
            Call {
                event: OpenEvent::Reopen {
                    behavior: None,
                    behind: false,
                    window: WindowOptions::default(),
                },
                wait_ready: false,
            }
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_path_that_is_not_utf8_fails_to_encode_rather_than_encoding_as_nothing() {
        // The file system allows it, JSON does not. Sending `params: null`
        // would have the primary answer `InvalidParams`, which says nothing
        // about what was actually wrong.
        use std::os::unix::ffi::OsStrExt;

        let path = PathBuf::from(std::ffi::OsStr::from_bytes(b"/\xff.md"));
        let call = Call {
            event: OpenEvent::Open(OpenRequest {
                files: vec![path],
                directory: None,
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            }),
            wait_ready: false,
        };
        assert!(call.to_method().is_err());
    }

    #[test]
    fn an_unknown_method_is_reported_as_such() {
        let error = Call::from_method("arto/levitate", None).unwrap_err();
        assert_eq!(error.code, ErrorCode::MethodNotFound as i64);
    }

    #[test]
    fn parameters_of_the_wrong_shape_are_reported_as_such() {
        let error = Call::from_method(OPEN, Some(json!({"files": "not a list"}))).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams as i64);
    }

    #[test]
    fn capabilities_say_yes_to_everything_this_version_has() {
        let result = InitializeResult {
            server_info: PeerInfo {
                name: "arto".into(),
                version: Some("0.1.0".into()),
            },
            capabilities: ServerCapabilities::default(),
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({
                "serverInfo": {"name": "arto", "version": "0.1.0"},
                "capabilities": {"open": true, "reopen": true, "waitReady": true},
            })
        );
    }
}
