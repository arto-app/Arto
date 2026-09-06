//! Arto's keybinding model.
//!
//! Everything about shortcuts that does not need a window: parsing shortcut
//! notation (`"Ctrl+Shift+g"`, `"g g"`) into chords, the binding sets stored
//! in the user's `mappings.json` and shipped as presets, the actions those
//! bindings name, the context rules, the sequence-matching engine, and the
//! formatting of shortcut hints.
//!
//! The desktop app supplies what this crate deliberately leaves out: turning
//! Dioxus keyboard events into chords, converting chords into native menu
//! accelerators, and reading the user's configuration.

mod action;
mod bindings;
mod codes;
mod context;
mod engine;
mod hint;
pub mod presets;
mod resolve;
mod shortcut;

pub use action::*;
pub use bindings::*;
pub use codes::*;
pub use context::*;
pub use engine::*;
pub use hint::*;
pub use resolve::*;
pub use shortcut::*;

// The key types chords are built from. Callers that already have them from
// Dioxus or muda get the very same types.
pub use keyboard_types::{Code, Key, Modifiers};
