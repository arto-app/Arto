//! Single-instance IPC for Arto.
//!
//! Only one Arto process runs per user. A second launch connects to the
//! socket of the running instance, hands over what it was asked to open,
//! and exits; if nothing answers, it becomes the primary instance and
//! starts listening itself.
//!
//! This crate owns the two halves that do not depend on a window system:
//! the wire protocol ([`IpcMessage`] and its normalized form [`OpenEvent`])
//! and the local socket ([`send_to_existing_instance`], [`IpcServer`]). What
//! happens with a received event, such as choosing a window and opening the
//! files in it, is the app's business.
//!
//! # Flow
//!
//! ```text
//! 1st launch (primary):
//!   send_to_existing_instance() → NoExistingInstance
//!   IpcServer::bind() → serve(|events| ...)   accepts later launches
//!
//! 2nd launch (secondary):
//!   send_to_existing_instance() → Sent → exit
//! ```
//!
//! # Protocol
//!
//! One JSON object per line ([JSON Lines](https://jsonlines.org)), written
//! by the secondary instance and read by the primary:
//!
//! ```json
//! {"type":"open","files":["/path/to/file.md"],"directory":null,"behavior":"last_focused","behind":false}
//! {"type":"open","files":[],"directory":"/path/to/dir","behavior":"new_window","behind":true}
//! {"type":"reopen","behavior":"last_focused","behind":false}
//! {"type":"reopen","behavior":"new_window","behind":false,"window":{"position":{"x":120,"y":64}},"wait_ready":true}
//! ```
//!
//! The older `file` and `directory` messages are still accepted so a
//! freshly upgraded primary understands a not-yet-upgraded secondary, and
//! `behind` defaults to `false` when a message omits it. [`WindowOptions`]
//! and `wait_ready` are left out of the line when nothing asked for them,
//! so the common case is the line it has always been.
//!
//! A message that sets `wait_ready` gets an answer back down the same
//! connection ([`ReadyReply`]) once the app fires the [`ReadySignal`] it was
//! handed; everything else is written and forgotten.
//!
//! # Socket location
//!
//! Unix: `$XDG_RUNTIME_DIR/com.lambdalisue.arto.sock`, or
//! `/tmp/arto-<uid>/com.lambdalisue.arto.sock` when there is no runtime
//! directory. Windows: a named pipe carrying the user name. See
//! [`socket_path`].

mod client;
mod protocol;
mod server;
mod socket;

pub use client::*;
pub use protocol::*;
pub use server::*;
pub use socket::*;
