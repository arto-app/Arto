//! Values that Arto's configuration and its wire protocol both carry.
//!
//! `config.json` names a theme preference and where an opened file goes, and
//! a second launch hands the same values to the running instance. Both read
//! one definition, so the spelling on the wire cannot drift from the one in
//! the file. The types live here rather than in `arto-config` so the protocol
//! crate does not pull in the whole configuration — the Markdown pipeline and
//! the keybindings among it — for two enums.

mod file_open_behavior;
mod theme;

pub use file_open_behavior::*;
pub use theme::*;
