//! A highlight's card: a small overlay beside the highlight, holding its
//! colours, a way to take it away, and the reader's note on it.
//!
//! The note is saved when the card closes, however it closes — Escape,
//! Cmd/Ctrl+Enter, a click outside, the document changing under it, or the
//! highlight going away — and only when it was changed: a card opened to
//! read a note must not write back over one edited meanwhile in another
//! window.

use dioxus::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::components::icon::{Icon, IconName};
use crate::highlights::card::{place, OpenCard, Placement};
use crate::highlights::HighlightColor;
use crate::state::AppState;

/// The card of the highlight [`AppState::highlight_card`] names, if any.
#[component]
pub fn HighlightCardHost() -> Element {
    let state = use_context::<AppState>();
    let card = state.highlight_card.read().clone();

    rsx! {
        if let Some(card) = card {
            // Keyed by the highlight, so another highlight's card starts
            // from its own note rather than from what was typed in this one.
            HighlightCard { key: "{card.id}", card }
        }
    }
}

/// What the note said when the card opened, and what the field says now.
struct Draft {
    was: String,
    now: String,
}

impl Draft {
    fn changed(&self) -> bool {
        self.now.trim() != self.was.trim()
    }

    /// Leave the note as it was, whatever the field says.
    fn discard(&mut self) {
        self.now = self.was.clone();
    }
}

#[component]
fn HighlightCard(card: OpenCard) -> Element {
    let mut state = use_context::<AppState>();
    let OpenCard { document, id, rect } = card;

    let highlight = state
        .highlights
        .read()
        .iter()
        .find(|highlight| highlight.id == id)
        .cloned();
    let color = highlight.as_ref().map(|highlight| highlight.color);

    // Not signals: the field is not controlled (see the textarea below), and
    // the note is saved as the card is dropped, when its signals may be gone.
    let draft = use_hook(|| {
        let note = highlight
            .as_ref()
            .and_then(|highlight| highlight.note.clone())
            .unwrap_or_default();
        Rc::new(RefCell::new(Draft {
            was: note.clone(),
            now: note,
        }))
    });
    let seen = use_hook(|| Rc::new(Cell::new(false)));

    use_drop({
        let draft = draft.clone();
        let document = document.clone();
        let id = id.clone();
        move || {
            let draft = draft.borrow();
            // The card is already gone, so a note that could not be written
            // cannot be handed back to it; the reader is at least told.
            if draft.changed() && !crate::highlights::annotate(&document, &id, &draft.now) {
                crate::keybindings::dispatcher::show_action_feedback("The note could not be kept");
            }
        }
    });

    let mut close = move || state.highlight_card.set(None);

    // Close with the document it was opened on, and with the highlight. A
    // highlight just made is not in the state until the store has announced
    // it, so it counts as gone only once it has been there.
    use_effect({
        let document = document.clone();
        let id = id.clone();
        move || {
            let present = state
                .highlights
                .read()
                .iter()
                .any(|highlight| highlight.id == id);
            let shown = state.current_file().as_deref() == Some(document.as_path());
            if present {
                seen.set(true);
            }
            if !shown || (!present && seen.get()) {
                state.highlight_card.set(None);
            }
        }
    });

    // Placed by where the highlight was when it opened: a window resized or
    // a page zoomed moves the highlight away from it, so the card goes (and
    // keeps its note) rather than point at other words.
    let laid_out = use_hook(|| (*state.size.peek(), *state.zoom_level.peek()));
    use_effect(move || {
        if (*state.size.read(), *state.zoom_level.read()) != laid_out {
            state.highlight_card.set(None);
        }
    });

    // Written into at once: the card is opened to write in.
    use_effect(|| {
        spawn(async {
            let _ = dioxus::document::eval(
                "const note = document.querySelector('.highlight-card-note');\
                 note?.focus();\
                 note?.setSelectionRange(note.value.length, note.value.length);",
            )
            .await;
        });
    });

    let size = *state.size.read();
    let position = match place(rect, f64::from(size.width), f64::from(size.height)) {
        Placement::Below { left, top } => format!("left: {left}px; top: {top}px;"),
        Placement::Above { left, bottom } => format!("left: {left}px; bottom: {bottom}px;"),
    };
    let initial = draft.borrow().was.clone();

    rsx! {
        div {
            class: "highlight-card-backdrop",
            onclick: move |_| close(),
        }

        div {
            class: "highlight-card",
            style: "{position}",
            onclick: move |evt| evt.stop_propagation(),

            div {
                class: "highlight-card-bar",
                for choice in HighlightColor::ALL {
                    button {
                        key: "{choice.to_js_name()}",
                        class: if Some(choice) == color {
                            format!("color-palette-swatch selected {}", choice.css_class())
                        } else {
                            format!("color-palette-swatch {}", choice.css_class())
                        },
                        onclick: {
                            let document = document.clone();
                            let id = id.clone();
                            move |_| crate::highlights::recolor(&document, &id, choice)
                        },
                    }
                }

                div { class: "highlight-card-spacer" }

                button {
                    class: "color-palette-action color-palette-remove",
                    title: "Remove Highlight",
                    onclick: {
                        let document = document.clone();
                        let id = id.clone();
                        let draft = draft.clone();
                        move |_| {
                            // The note goes with the highlight; there is
                            // nothing left to write it on as the card closes.
                            draft.borrow_mut().discard();
                            crate::highlights::remove_highlights(&document, std::slice::from_ref(&id));
                            close();
                        }
                    },
                    Icon { name: IconName::Trash, size: 16 }
                }
            }

            // Not controlled, as the palette's field is not: writing the value
            // back on each keystroke tears an input method's composition apart.
            textarea {
                class: "highlight-card-note",
                rows: "4",
                placeholder: "Add a note\u{2026}",
                aria_label: "Note",
                initial_value: "{initial}",
                oninput: {
                    let draft = draft.clone();
                    move |evt: Event<FormData>| draft.borrow_mut().now = evt.value()
                },
                onkeydown: move |evt: Event<KeyboardData>| {
                    // A key that ends a composition belongs to the input method.
                    if evt.is_composing() || evt.key() == Key::Process {
                        return;
                    }
                    let modifiers = evt.modifiers();
                    let done = match evt.key() {
                        Key::Escape => true,
                        Key::Enter => modifiers.meta() || modifiers.ctrl(),
                        _ => false,
                    };
                    if done {
                        evt.prevent_default();
                        evt.stop_propagation();
                        close();
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_changed_only_when_its_words_are() {
        let draft = |was: &str, now: &str| Draft {
            was: was.to_string(),
            now: now.to_string(),
        };

        assert!(!draft("", "").changed());
        assert!(!draft("note", "note\n").changed());
        assert!(draft("", "new").changed());
        assert!(draft("note", "").changed());
        assert!(draft("note", "noted").changed());
    }

    #[test]
    fn a_discarded_note_is_not_written() {
        let mut draft = Draft {
            was: "note".to_string(),
            now: "noted".to_string(),
        };
        draft.discard();
        assert!(!draft.changed());
    }
}
