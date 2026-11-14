//! What the two ends actually say to each other.
//!
//! The lifecycle is LSP's — `initialize`, then anything else — with one
//! deliberate departure spelled out at [`EXIT`]. The methods themselves are
//! Arto's, and they carry the same [`OpenRequest`] and [`WindowOptions`] the
//! line-delimited protocol carried: what changed is the envelope, not what a
//! launch is asking for.

use crate::jsonrpc::{ErrorCode, ResponseError};
use crate::protocol::{OpenEvent, OpenRequest, WindowKey, WindowOptions};
use arto_types::{FileOpenBehavior, Theme};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Establish what the two ends can say to each other. Must come first.
pub const INITIALIZE: &str = "initialize";
/// The client is ready. A notification, as in LSP.
pub const INITIALIZED: &str = "initialized";
/// Open files, a directory, or both.
pub const OPEN: &str = "arto/open";
/// Bring a window forward, or place and repaint one, without opening
/// anything.
pub const REOPEN: &str = "arto/reopen";
/// What is open, and where the reader is in it.
pub const STATE: &str = "arto/state";
/// Put the reader somewhere in the document a window is showing.
pub const GOTO: &str = "arto/goto";
/// What the reader has selected, if anything.
pub const SELECTION: &str = "arto/selection";
/// A picture of a block, or of what the reader sees, as the page draws it.
pub const CAPTURE: &str = "arto/capture";
/// A window's state changed. Carries the whole [`WindowState`] rather than
/// a delta: one window's state is small, and a client that wants to know
/// where the reader is should not have to reassemble it.
pub const DID_CHANGE_STATE: &str = "arto/didChangeState";
/// Stop asking for things; [`EXIT`] follows.
///
/// LSP's own handshake for ending a session, kept because an editor's
/// client machinery sends it and would otherwise be answered
/// `MethodNotFound` on the way out. It ends nothing by itself: a request
/// after it is refused, and the connection closes when `exit` arrives.
pub const SHUTDOWN: &str = "shutdown";
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
    #[serde(default)]
    pub capabilities: ClientCapabilities,
}

/// What a client can be told, as opposed to what it can ask. Nothing is
/// pushed to a client that did not ask for it: a notification arriving at a
/// peer that expects none is at best ignored and at worst a parse error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Send [`DID_CHANGE_STATE`] when a window's state changes.
    #[serde(default)]
    pub state_changes: bool,
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
///
/// Every field defaults to `false`, so an instance from before a capability
/// existed reads as lacking it rather than failing to parse. Without that,
/// adding a field here would make every older running instance unreachable:
/// the launch would conclude there is none, then fail to bind the socket it
/// is holding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(default)]
    pub open: bool,
    #[serde(default)]
    pub reopen: bool,
    /// Whether the instance can report what its windows are showing.
    #[serde(default)]
    pub state: bool,
    /// Whether the reader can be moved within a document from outside.
    #[serde(default)]
    pub goto: bool,
    /// Whether what the reader has selected can be asked for.
    #[serde(default)]
    pub selection: bool,
    /// Whether a picture of what a window draws can be asked for.
    #[serde(default)]
    pub capture: bool,
    /// Whether a client that asked for [`DID_CHANGE_STATE`] will get it.
    /// The client says what it can be told before hearing this, so the
    /// answer is what tells it whether anything will actually arrive.
    #[serde(default)]
    pub state_changes: bool,
    /// Whether a request may ask to be answered only once the window has
    /// drawn, rather than as soon as the request is applied.
    #[serde(default)]
    pub wait_ready: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            open: true,
            reopen: true,
            state: true,
            goto: true,
            selection: true,
            capture: true,
            state_changes: true,
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

/// The parameters of [`STATE`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateParams {
    /// One window, or every open one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowKey>,
}

/// What [`STATE`] answers with.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateResult {
    /// Every window asked about, in the order they opened.
    pub windows: Vec<WindowState>,
    /// Which of them the reader was last in, if any is still open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<WindowKey>,
}

/// One window, as somebody outside would want it described.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub window: WindowKey,
    /// What is being read, or absent for a window on the welcome page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<PathBuf>,
    pub theme: Theme,
    /// Where the reader is, named by content rather than by pixels.
    pub scroll: ScrollPosition,
    pub headings: Vec<Heading>,
}

/// Where the reader is, named by content rather than by pixels.
///
/// A pixel offset only means anything while every block is the height it
/// was when the offset was taken, and diagrams and formulas are drawn after
/// the document appears. Naming the block is what makes "where am I" and
/// "put me back there" the same place.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollPosition {
    /// 1-based source line of the block at the top; 0 is the top of the
    /// document.
    pub line: u32,
    /// How far into that block the top edge sits, 0 to just under 1.
    pub fraction: f32,
}

/// One entry of a document's table of contents.
///
/// No source line: the extractor does not carry one, and the rendered
/// heading's `data-source-line` lives in the page, not in this process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// The anchor the heading renders with.
    pub id: String,
}

/// The parameters of [`DID_CHANGE_STATE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeStateParams {
    pub window: WindowKey,
    /// How the window reads now, or absent once it has closed. A close
    /// needs no shape of its own: a window with no state is a window gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<WindowState>,
}

/// The parameters of [`SELECTION`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionParams {
    /// Which window to ask. The last focused one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowKey>,
}

/// What [`SELECTION`] answers with.
///
/// The one thing on this protocol that cannot be read off the main thread:
/// a selection lives in the WebView, so answering means a round trip into
/// the page. That is why it is its own method rather than a field of
/// [`StateResult`], which is answered in the turn that asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionResult {
    pub window: WindowKey,
    /// What is selected, or absent when nothing is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The parameters of [`CAPTURE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureParams {
    /// Which window to draw from. The last focused one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowKey>,
    pub target: CaptureTarget,
}

/// What to draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureTarget {
    /// The top-level block a source line belongs to — the line
    /// [`ScrollPosition`] names for the block at the top, among others.
    Block(u32),
    /// The block of a heading, by the `id` [`STATE`] reported for it.
    Heading(String),
    /// What the reader sees of the document now.
    Viewport,
}

/// What [`CAPTURE`] answers with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResult {
    pub window: WindowKey,
    /// The picture, as base64 of a PNG.
    pub png: String,
    /// Its size in pixels.
    pub width: u32,
    pub height: u32,
    /// The source lines the block was rendered from; absent for a viewport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
}

/// Source lines, 1-based, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

/// The parameters of [`GOTO`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoParams {
    /// Which window to move. The last focused one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowKey>,
    pub target: GotoTarget,
}

/// Where to put the reader.
///
/// Externally tagged — `{"heading": "install"}` rather than a shape with
/// four optional fields of which exactly one may be set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GotoTarget {
    /// A heading, by the `id` [`STATE`] reported for it.
    Heading(String),
    /// A place in the source, as [`STATE`] reports the current one — hand
    /// back what it gave and the same text is under the top edge again.
    Scroll(ScrollPosition),
    /// A hit of a search, which is run as if it had been typed into the
    /// panel: the matches stay highlighted where the reader can see them.
    Search {
        query: String,
        /// Which hit, counting from zero. Out of range moves nothing.
        #[serde(default)]
        index: u32,
    },
    /// The top of the document.
    Top,
    /// The bottom of the document.
    Bottom,
}

/// What [`GOTO`] answers with.
///
/// Sent once the window has been asked to move, not once the reader has
/// arrived — a destination is held until the document stops growing under
/// it. A client that needs the settled position asks [`STATE`] for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoResult {
    /// Which window was moved.
    pub window: WindowKey,
}

/// One request, as the app's own handler sees it.
///
/// Split by what it does rather than by method name, because that is the
/// distinction the app acts on: only [`Apply`](Self::Apply) opens anything
/// or can be asked to wait for a drawing.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// Change what a window is showing.
    Apply {
        event: OpenEvent,
        /// Answer only once the target window has drawn it.
        wait_ready: bool,
    },
    /// Report what is showing.
    State(StateParams),
    /// Move the reader within what a window is already showing.
    Goto(GotoParams),
    /// Report what the reader has selected.
    Selection(SelectionParams),
    /// Draw a picture of what a window shows.
    Capture(CaptureParams),
}

impl Call {
    /// Whether the answer waits for a window to be drawn.
    pub fn wait_ready(&self) -> bool {
        matches!(
            self,
            Self::Apply {
                wait_ready: true,
                ..
            }
        )
    }

    /// Whether answering may take as long as drawing a document does, which
    /// is what both ends size their deadline by.
    pub fn waits_on_a_drawing(&self) -> bool {
        self.wait_ready() || matches!(self, Self::Capture(_))
    }

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
                Ok(Self::Apply {
                    event: OpenEvent::Open(params.request),
                    wait_ready: params.wait_ready,
                })
            }
            REOPEN => {
                let params: ReopenParams = parse_params(params)?;
                Ok(Self::Apply {
                    event: OpenEvent::Reopen {
                        behavior: params.behavior,
                        behind: params.behind,
                        window: params.window,
                    },
                    wait_ready: params.wait_ready,
                })
            }
            STATE => Ok(Self::State(parse_params(params)?)),
            GOTO => Ok(Self::Goto(parse_params(params)?)),
            SELECTION => Ok(Self::Selection(parse_params(params)?)),
            CAPTURE => Ok(Self::Capture(parse_params(params)?)),
            other => Err(ResponseError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method: {other}"),
            )),
        }
    }

    /// Whether an instance that answered `initialize` with `capabilities`
    /// can be asked this.
    pub fn is_offered_by(&self, capabilities: &ServerCapabilities) -> bool {
        match self {
            Self::Apply { event, wait_ready } => {
                let method = match event {
                    OpenEvent::Open(_) => capabilities.open,
                    OpenEvent::Reopen { .. } => capabilities.reopen,
                };
                method && (!wait_ready || capabilities.wait_ready)
            }
            Self::State(_) => capabilities.state,
            Self::Goto(_) => capabilities.goto,
            Self::Selection(_) => capabilities.selection,
            Self::Capture(_) => capabilities.capture,
        }
    }

    /// The method and parameters that carry this call.
    ///
    /// Fallible because a [`PathBuf`](std::path::PathBuf) that is not valid
    /// UTF-8 cannot be written as JSON. Answering that with `null` params
    /// would send a request the primary is bound to refuse, and the refusal
    /// would say `InvalidParams` rather than what was actually wrong.
    pub fn to_method(&self) -> Result<(&'static str, serde_json::Value), serde_json::Error> {
        match self {
            Self::Apply {
                event: OpenEvent::Open(request),
                wait_ready,
            } => Ok((
                OPEN,
                serde_json::to_value(OpenParams {
                    request: request.clone(),
                    wait_ready: *wait_ready,
                })?,
            )),
            Self::Apply {
                event:
                    OpenEvent::Reopen {
                        behavior,
                        behind,
                        window,
                    },
                wait_ready,
            } => Ok((
                REOPEN,
                serde_json::to_value(ReopenParams {
                    behavior: *behavior,
                    behind: *behind,
                    window: *window,
                    wait_ready: *wait_ready,
                })?,
            )),
            Self::State(params) => Ok((STATE, serde_json::to_value(params)?)),
            Self::Goto(params) => Ok((GOTO, serde_json::to_value(params)?)),
            Self::Selection(params) => Ok((SELECTION, serde_json::to_value(params)?)),
            Self::Capture(params) => Ok((CAPTURE, serde_json::to_value(params)?)),
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
    use arto_types::Theme;
    use serde_json::json;
    use std::path::PathBuf;

    fn offering_only(capability: &str) -> ServerCapabilities {
        serde_json::from_value(json!({ capability: true })).unwrap()
    }

    #[test]
    fn a_call_is_offered_only_by_an_instance_that_has_its_capability() {
        let calls = [
            (
                "open",
                Call::Apply {
                    event: OpenEvent::Open(OpenRequest {
                        files: vec![],
                        directory: None,
                        behavior: None,
                        behind: false,
                        window: WindowOptions::default(),
                    }),
                    wait_ready: false,
                },
            ),
            (
                "reopen",
                Call::Apply {
                    event: OpenEvent::Reopen {
                        behavior: None,
                        behind: false,
                        window: WindowOptions::default(),
                    },
                    wait_ready: false,
                },
            ),
            ("state", Call::State(StateParams::default())),
            (
                "goto",
                Call::Goto(GotoParams {
                    window: None,
                    target: GotoTarget::Top,
                }),
            ),
            ("selection", Call::Selection(SelectionParams::default())),
            (
                "capture",
                Call::Capture(CaptureParams {
                    window: None,
                    target: CaptureTarget::Viewport,
                }),
            ),
        ];
        for (capability, call) in calls {
            assert!(
                call.is_offered_by(&offering_only(capability)),
                "{capability}"
            );
            assert!(
                !call.is_offered_by(&offering_only("none")),
                "{capability} offered by an instance that never mentioned it"
            );
        }
    }

    #[test]
    fn a_call_that_waits_needs_an_instance_that_can_wait() {
        // Asking an instance to wait that cannot would be answered at once,
        // and the client would take `ready: false` for "did not draw".
        let call = Call::Apply {
            event: OpenEvent::Reopen {
                behavior: None,
                behind: false,
                window: WindowOptions::default(),
            },
            wait_ready: true,
        };
        assert!(!call.is_offered_by(&offering_only("reopen")));
        let both: ServerCapabilities =
            serde_json::from_value(json!({"reopen": true, "waitReady": true})).unwrap();
        assert!(call.is_offered_by(&both));
    }

    #[test]
    fn an_open_call_round_trips_through_its_method_and_params() {
        let call = Call::Apply {
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
        let call = Call::Apply {
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
        let call = Call::Apply {
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
            Call::Apply {
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
        let call = Call::Apply {
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
    fn an_instance_that_never_heard_of_a_capability_is_read_as_lacking_it() {
        // What an older running instance answers `initialize` with. Reading
        // this as a parse failure would make it unreachable: the launch
        // would conclude there is no instance, and then fail to bind the
        // socket that instance is holding.
        let older: ServerCapabilities =
            serde_json::from_value(json!({"open": true, "reopen": true, "waitReady": true}))
                .unwrap();
        assert!(older.open);
        assert!(older.wait_ready);
        assert!(!older.state, "a capability it never mentioned is absent");
    }

    #[test]
    fn asking_about_everything_needs_no_parameters() {
        assert_eq!(
            Call::from_method(STATE, None).unwrap(),
            Call::State(StateParams { window: None })
        );
    }

    #[test]
    fn a_state_call_round_trips_through_its_method_and_params() {
        let call = Call::State(StateParams {
            window: Some(WindowKey(3)),
        });
        let (method, params) = call.to_method().unwrap();
        assert_eq!(method, STATE);
        assert_eq!(params, json!({"window": 3}));
        assert_eq!(Call::from_method(method, Some(params)).unwrap(), call);
    }

    #[test]
    fn a_query_never_waits_for_a_window() {
        // Nothing is drawn by asking, so there is nothing to wait for.
        assert!(!Call::State(StateParams::default()).wait_ready());
    }

    #[test]
    fn the_answer_names_windows_the_way_the_request_does() {
        let result = StateResult {
            windows: vec![WindowState {
                window: WindowKey(1),
                document: Some(PathBuf::from("/README.md")),
                theme: Theme::Dark,
                scroll: ScrollPosition {
                    line: 42,
                    fraction: 0.25,
                },
                headings: vec![Heading {
                    level: 2,
                    text: "Install".into(),
                    id: "install".into(),
                }],
            }],
            focused: Some(WindowKey(1)),
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({
                "windows": [{
                    "window": 1,
                    "document": "/README.md",
                    "theme": "dark",
                    "scroll": {"line": 42, "fraction": 0.25},
                    "headings": [{"level": 2, "text": "Install", "id": "install"}],
                }],
                "focused": 1,
            })
        );
    }

    #[test]
    fn a_window_on_the_welcome_page_reports_no_document() {
        let window = WindowState {
            window: WindowKey(2),
            document: None,
            theme: Theme::Auto,
            scroll: ScrollPosition::default(),
            headings: Vec::new(),
        };
        let json = serde_json::to_value(&window).unwrap();
        assert!(
            !json.as_object().unwrap().contains_key("document"),
            "{json}"
        );
    }

    #[test]
    fn a_capture_call_round_trips_through_its_method_and_params() {
        let call = Call::Capture(CaptureParams {
            window: Some(WindowKey(3)),
            target: CaptureTarget::Block(42),
        });
        let (method, params) = call.to_method().unwrap();
        assert_eq!(method, CAPTURE);
        assert_eq!(params, json!({"window": 3, "target": {"block": 42}}));
        assert_eq!(Call::from_method(method, Some(params)).unwrap(), call);
    }

    #[test]
    fn capture_targets_travel_tagged_by_what_they_name() {
        assert_eq!(
            serde_json::to_value(CaptureTarget::Heading("install".into())).unwrap(),
            json!({"heading": "install"})
        );
        assert_eq!(
            serde_json::to_value(CaptureTarget::Viewport).unwrap(),
            json!("viewport")
        );
    }

    #[test]
    fn a_capture_is_given_as_long_as_a_drawing_to_answer() {
        // It draws the block's diagrams before it can draw the block, which
        // is the same wait as a launch that asked for the window to draw.
        let capture = Call::Capture(CaptureParams {
            window: None,
            target: CaptureTarget::Viewport,
        });
        assert!(capture.waits_on_a_drawing());
        assert!(!Call::State(StateParams::default()).waits_on_a_drawing());
        assert!(!capture.wait_ready(), "a capture opens nothing to wait for");
    }

    #[test]
    fn a_selection_call_round_trips_through_its_method_and_params() {
        let call = Call::Selection(SelectionParams {
            window: Some(WindowKey(2)),
        });
        let (method, params) = call.to_method().unwrap();
        assert_eq!(method, SELECTION);
        assert_eq!(params, json!({"window": 2}));
        assert_eq!(Call::from_method(method, Some(params)).unwrap(), call);
    }

    #[test]
    fn a_search_target_travels_tagged_by_what_kind_of_place_it_is() {
        let call = Call::Goto(GotoParams {
            window: None,
            target: GotoTarget::Search {
                query: "install".into(),
                index: 2,
            },
        });
        let (_, params) = call.to_method().unwrap();
        assert_eq!(
            params,
            json!({"target": {"search": {"query": "install", "index": 2}}})
        );
    }

    #[test]
    fn a_search_that_names_no_hit_takes_the_first() {
        let call =
            Call::from_method(GOTO, Some(json!({"target": {"search": {"query": "a"}}}))).unwrap();
        assert_eq!(
            call,
            Call::Goto(GotoParams {
                window: None,
                target: GotoTarget::Search {
                    query: "a".into(),
                    index: 0,
                },
            })
        );
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
                "capabilities": {
                    "open": true,
                    "reopen": true,
                    "state": true,
                    "goto": true,
                    "selection": true,
                    "capture": true,
                    "stateChanges": true,
                    "waitReady": true,
                },
            })
        );
    }
}
