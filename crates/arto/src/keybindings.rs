//! Keybindings for the desktop app.
//!
//! The model (shortcut parsing, binding sets, presets, the matching engine,
//! hint formatting) lives in `arto-keybindings`. This module binds it to the
//! things only the app has: Dioxus keyboard events, native menu accelerators,
//! the user's configuration in `CONFIG`, and the dispatcher that turns matched
//! actions into behavior.

mod accelerator;
pub mod dispatcher;

pub use accelerator::*;
pub use arto_keybindings::*;

use crate::config::{BindingSet, CONFIG};
use dioxus::events::KeyboardEvent;
use dioxus::prelude::ModifiersInteraction;

/// Build a chord from a Dioxus keyboard event.
pub fn chord_from_event(event: &KeyboardEvent) -> KeyChord {
    KeyChord::new(event.data().key(), event.data().modifiers())
}

/// Build the matching engine for `bindings`, reporting the entries it had to
/// skip. Invalid keys or unknown actions come from a hand-edited
/// `mappings.json`; they are warned about here, once per engine build,
/// rather than silently dropped.
pub fn engine_for(bindings: &BindingSet) -> KeybindingEngine {
    let (engine, errors) = KeybindingEngine::build(bindings);
    for error in errors {
        tracing::warn!(%error, "Skipping keybinding");
    }
    engine
}

/// Return a formatted shortcut hint for the given action from the user's
/// current bindings.
///
/// Lookup order: context binding → global keybinding → menu shortcut.
pub fn shortcut_hint_for_action(action: &str, context: Option<KeyContext>) -> Option<String> {
    hint_for_action(&CONFIG.read().keybindings, action, context)
}

/// Return a formatted shortcut hint from global (and menu) keybindings.
///
/// Only referenced by the Windows in-app menu (`win_hamburger`); platforms with
/// a native menu (macOS/Linux) derive accelerators from `menu_shortcuts`.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn shortcut_hint_for_global_action(action: &str) -> Option<String> {
    shortcut_hint_for_action(action, None)
}

/// Return a formatted shortcut hint in the given context.
pub fn shortcut_hint_for_context_action(context: KeyContext, action: &str) -> Option<String> {
    shortcut_hint_for_action(action, Some(context))
}
