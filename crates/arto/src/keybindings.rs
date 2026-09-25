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

use crate::config::{BindingSet, KeyAction, Lens, CONFIG};
use dioxus::events::KeyboardEvent;
use dioxus::prelude::ModifiersInteraction;
use std::str::FromStr;

/// Build a chord from a Dioxus keyboard event.
pub fn chord_from_event(event: &KeyboardEvent) -> KeyChord {
    KeyChord::new(event.data().key(), event.data().modifiers())
}

/// The bindings a window's keys are matched against: the configured ones,
/// and each usable lens's own shortcut.
pub fn effective_bindings() -> BindingSet {
    let keybindings = CONFIG.read().keybindings.clone();
    with_lens_shortcuts(keybindings, &crate::lenses::offered_lenses())
}

/// `bindings` with the shortcut of each of `lenses` bound to that lens by
/// its place among them, over the document: a lens looks at the document,
/// and a single letter bound everywhere would fire while typing a search.
///
/// A key something else already has stays with it. The engine lets the
/// last binding of a key win, so a lens appended after the keybindings
/// would otherwise take the key from them without a word.
fn with_lens_shortcuts(bindings: BindingSet, lenses: &[Lens]) -> BindingSet {
    let mut bound = bindings.clone();
    for (place, lens) in lenses.iter().enumerate() {
        let (Some(key), Ok(index)) = (&lens.shortcut, u16::try_from(place)) else {
            continue;
        };
        if let Some(holder) = lens_shortcut_holder(&bindings, lenses, place) {
            tracing::warn!(lens = %lens.id, %key, %holder, "a lens shortcut is already bound");
            continue;
        }
        bound.content.push(KeyAction {
            key: key.clone(),
            action: Action::Lens(index).to_string(),
        });
    }
    bound
}

/// What already has the shortcut of the lens at `place` among `lenses`: an
/// action of `bindings` that is heard over the document, or a lens before
/// it. `None` when the key is free, or when the lens has no shortcut that
/// can be read.
///
/// A binding that is the start of the other counts as having it: the
/// engine runs a sequence the moment it is complete, so of `g` and `g g`
/// the longer could never be finished.
pub fn lens_shortcut_holder(
    bindings: &BindingSet,
    lenses: &[Lens],
    place: usize,
) -> Option<String> {
    let chords = lenses
        .get(place)?
        .shortcut
        .as_deref()
        .and_then(|key| ShortcutSequence::from_str(key).ok())?
        .chords;
    let same = |key: &str| {
        ShortcutSequence::from_str(key).is_ok_and(|other| {
            other.chords.starts_with(&chords) || chords.starts_with(&other.chords)
        })
    };
    let bound = [
        &bindings.menu_shortcuts,
        &bindings.global,
        &bindings.content,
    ]
    .into_iter()
    .flatten()
    .find(|binding| same(&binding.key))
    .map(|binding| binding.action.clone());
    bound.or_else(|| {
        lenses[..place]
            .iter()
            .find(|lens| lens.shortcut.as_deref().is_some_and(same))
            .map(|lens| format!("the lens {}", lens.label))
    })
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
/// This is the hint for an action reached by name rather than by key — from
/// the palette, or from the in-app menu — so it asks for no context.
pub fn shortcut_hint_for_global_action(action: &str) -> Option<String> {
    shortcut_hint_for_action(action, None)
}

/// Return a formatted shortcut hint in the given context.
pub fn shortcut_hint_for_context_action(context: KeyContext, action: &str) -> Option<String> {
    shortcut_hint_for_action(action, Some(context))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lens_shortcut_is_bound_over_the_document_to_the_lens_s_place() {
        let lenses = [
            Lens {
                shortcut: Some("Cmd+Shift+t".to_string()),
                ..Lens::new("translate")
            },
            Lens::new("summary"),
            Lens {
                shortcut: Some("g e".to_string()),
                ..Lens::new("explain")
            },
        ];

        let bindings = with_lens_shortcuts(BindingSet::default(), &lenses);

        let bound: Vec<(&str, &str)> = bindings
            .content
            .iter()
            .map(|binding| (binding.key.as_str(), binding.action.as_str()))
            .collect();
        assert_eq!(bound, [("Cmd+Shift+t", "lens.0"), ("g e", "lens.2")]);
        assert!(bindings.global.is_empty());

        let (engine_bindings, errors) = bindings.resolve();
        assert!(errors.is_empty(), "{errors:?}");
        assert!(engine_bindings
            .iter()
            .any(|binding| binding.action == Action::Lens(2)));
    }

    #[test]
    fn a_lens_shortcut_already_bound_is_left_to_what_has_it() {
        let bindings = BindingSet {
            global: vec![KeyAction {
                key: "Cmd+w".to_string(),
                action: "window.close".to_string(),
            }],
            ..Default::default()
        };
        let lenses = [
            Lens {
                shortcut: Some(" Cmd+w ".to_string()),
                ..Lens::new("steals")
            },
            Lens {
                shortcut: Some("g t".to_string()),
                ..Lens::new("first")
            },
            Lens {
                shortcut: Some("g  t".to_string()),
                label: "Second".to_string(),
                ..Lens::new("second")
            },
        ];

        assert_eq!(
            lens_shortcut_holder(&bindings, &lenses, 0).as_deref(),
            Some("window.close")
        );
        assert_eq!(lens_shortcut_holder(&bindings, &lenses, 1), None);
        assert!(lens_shortcut_holder(&bindings, &lenses, 2).is_some());

        // One binding the start of the other takes the other's keys too: the
        // shorter fires before the longer can be finished.
        let vim = BindingSet {
            content: vec![KeyAction {
                key: "g g".to_string(),
                action: "scroll.top".to_string(),
            }],
            ..Default::default()
        };
        let prefixed = |key: &str| {
            [Lens {
                shortcut: Some(key.to_string()),
                ..Lens::new("l")
            }]
        };
        assert!(lens_shortcut_holder(&vim, &prefixed("g"), 0).is_some());
        assert!(lens_shortcut_holder(&vim, &prefixed("g g t"), 0).is_some());
        assert_eq!(lens_shortcut_holder(&vim, &prefixed("g t"), 0), None);

        let bound = with_lens_shortcuts(bindings, &lenses);
        let lens_keys: Vec<&str> = bound
            .content
            .iter()
            .map(|binding| binding.action.as_str())
            .collect();
        assert_eq!(lens_keys, ["lens.1"]);
    }
}
