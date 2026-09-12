//! Single-instance IPC for Arto.
//!
//! Only one Arto process runs per user. A second launch connects to the
//! socket of the running instance, hands over what it was asked to open,
//! and exits; if nothing answers, it becomes the primary instance and
//! starts listening itself.
//!
//! This crate owns the two halves that do not depend on a window system:
//! the wire protocol ([`Message`] and the [`OpenEvent`] it carries)
//! and the local socket ([`send_to_existing_instance`], [`Server`]). What
//! happens with a received event, such as choosing a window and opening the
//! files in it, is the app's business.
//!
//! # Flow
//!
//! ```text
//! 1st launch (primary):
//!   send_to_existing_instance() → NoExistingInstance
//!   Server::bind() → serve(|event, ready| ...)   accepts later launches
//!
//! 2nd launch (secondary):
//!   send_to_existing_instance() → Sent → exit
//! ```
//!
//! # Protocol
//!
//! JSON-RPC 2.0 in LSP's framing ([`read_message`] and [`write_message`]):
//! a `Content-Length` header, a blank line, then that many bytes of
//! message. `initialize` comes first and settles what the two ends can say
//! to each other; after it, [`OPEN`] and [`REOPEN`] carry what a launch was
//! asked to do and are answered with [`AppliedResult`].
//!
//! ```text
//! Content-Length: 58\r\n
//! \r\n
//! {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
//! ```
//!
//! Borrowing LSP's transport rather than inventing one is the point: the
//! editors Arto already lives beside speak it, so a client for this is
//! mostly a client they already have. The methods are Arto's own, and
//! [`EXIT`] deliberately means less here than it does there.
//!
//! # Upgrading over a running instance
//!
//! This replaced a bespoke line-delimited protocol outright rather than
//! falling back to it, so a copy of Arto started before the change does not
//! understand a launch from after it: the launch reports no instance, and
//! the socket it could not use stops it becoming primary either. Quitting
//! and reopening Arto is the whole of the fix, and it is needed once.
//!
//! Carrying the old protocol as a fallback would have meant keeping it —
//! and a rule for when it could go — indefinitely, to smooth a window that
//! closes the first time the user restarts the app.
//!
//! # Socket location
//!
//! Unix: `$XDG_RUNTIME_DIR/com.lambdalisue.arto.sock`, or
//! `/tmp/arto-<uid>/com.lambdalisue.arto.sock` when there is no runtime
//! directory. Windows: a named pipe carrying the user name. See
//! [`socket_path`].

mod client;
mod framing;
mod jsonrpc;
mod methods;
mod protocol;
mod server;
mod socket;

pub use client::*;
pub use framing::*;
pub use jsonrpc::*;
pub use methods::*;
pub use protocol::*;
pub use server::*;
pub use socket::*;
