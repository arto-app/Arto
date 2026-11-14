//! A Model Context Protocol server over Arto's socket protocol.
//!
//! Each tool is one request to the running Arto, answered with what Arto
//! answered: the server keeps no state of its own and can do nothing the
//! socket cannot. What it adds is a limit — `open` takes only paths under
//! the directories the server was started with ([`Roots`]) — and starting
//! Arto when a file is opened and Arto is not running.

pub mod cli;
mod instance;
mod roots;
mod tools;

pub use instance::*;
pub use roots::*;
pub use tools::*;
