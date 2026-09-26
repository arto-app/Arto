//! Opening a highlight into its card: a small overlay beside the highlight
//! on the page, where its colour is changed, it is taken away, and its note
//! is written (`components/highlight_card.rs`).
//!
//! The card is drawn by the app, over the page, so it has to be told where
//! the highlight is: the page measures it (`frontend/src/user-highlights.ts`)
//! either with the click that opened it, or when asked by id.

use dioxus::document;
use dioxus::prelude::*;
use serde::Deserialize;
use std::path::PathBuf;

use super::HighlightId;
use crate::state::AppState;
use crate::utils::task::spawn_detached;

/// A box on the screen, in the window's coordinates, as the page measures it.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// The highlight whose card is open, on the document it was opened on.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenCard {
    pub document: PathBuf,
    pub id: HighlightId,
    /// Where the highlight was drawn when the card opened.
    pub rect: Rect,
}

/// How wide the card is drawn (`highlight-card.css`).
pub const CARD_WIDTH: f64 = 280.0;
/// How tall the card is expected to be, to decide whether it fits below the
/// highlight. Its real height follows the note, so this is a fair guess
/// rather than a measure.
const CARD_HEIGHT: f64 = 180.0;
/// How far the card stands off the highlight.
const GAP: f64 = 6.0;
/// How far the card keeps from the window's edges.
const MARGIN: f64 = 8.0;

/// Where the card goes: under the highlight, or over it, pinned by its
/// bottom edge so that a card taller than expected grows away from the
/// words rather than over them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    Below { left: f64, top: f64 },
    Above { left: f64, bottom: f64 },
}

/// Place the card for a highlight drawn at `rect`, in a window `width` by
/// `height`: below it, or above it when there is no room below and more
/// above, and never outside the window.
pub fn place(rect: Rect, width: f64, height: f64) -> Placement {
    let left = rect.left.min(width - CARD_WIDTH - MARGIN).max(MARGIN);
    // The furthest from its anchored edge the card may start and still be
    // whole in the window.
    let furthest = (height - CARD_HEIGHT - MARGIN).max(MARGIN);
    let below = height - rect.bottom - GAP;
    let above = rect.top - GAP;
    if below >= CARD_HEIGHT + MARGIN || below >= above {
        let top = (rect.bottom + GAP).clamp(MARGIN, furthest);
        Placement::Below { left, top }
    } else {
        let bottom = (height - rect.top + GAP).clamp(MARGIN, furthest);
        Placement::Above { left, bottom }
    }
}

/// Open the card of the highlight `id` on `document`, beside `rect`.
///
/// Only while `document` is still the one shown: the page is asked where a
/// highlight is, and the reader can have moved on by the time it answers —
/// a card opened then would take a note for a highlight not on the page.
pub fn open(mut state: AppState, document: PathBuf, id: HighlightId, rect: Rect) {
    let shown = state
        .current_file()
        .is_some_and(|shown| super::same_document(&shown, &document));
    if !shown {
        return;
    }
    // Whatever was asked for before is overtaken by this card.
    count_ask(state);
    state
        .highlight_card
        .set(Some(OpenCard { document, id, rect }));
}

/// Count one more ask for a card, and return it.
fn count_ask(mut state: AppState) -> u64 {
    let mut asks = state.highlight_card_asks.write();
    *asks += 1;
    *asks
}

/// Open the card the page answered ask `asked` with, unless a card was
/// asked for since.
fn answer(state: AppState, asked: u64, document: PathBuf, id: HighlightId, rect: Rect) {
    if *state.highlight_card_asks.peek() == asked {
        open(state, document, id, rect);
    }
}

/// Open the card of the highlight `id`, beside where the page draws it.
pub fn open_at(state: AppState, id: HighlightId) {
    ask_and_open(state, id, "rectOf");
}

/// Bring the highlight `id` into view, then open its card beside it.
pub fn reveal(state: AppState, id: HighlightId) {
    ask_and_open(state, id, "reveal");
}

/// Ask the page where the highlight `id` is with `ask` (`rectOf` or
/// `reveal`), and open its card there if it is on the page.
fn ask_and_open(state: AppState, id: HighlightId, ask: &'static str) {
    let Some(document) = state.current_file() else {
        return;
    };
    let arg = serde_json::to_string(id.as_ref()).unwrap_or_default();
    let asked = count_ask(state);
    spawn_detached(async move {
        let js = format!(
            "(async () => dioxus.send(await window.Arto?.highlights?.{ask}?.({arg}) ?? null))()"
        );
        match document::eval(&js).recv::<Option<Rect>>().await {
            Ok(Some(rect)) => answer(state, asked, document, id, rect),
            Ok(None) => {}
            Err(error) => tracing::debug!(%error, "The highlight was not found on the page"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f64 = 1000.0;
    const HEIGHT: f64 = 800.0;

    fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn the_card_opens_below_the_highlight_at_its_left_edge() {
        assert_eq!(
            place(rect(120.0, 200.0, 300.0, 220.0), WIDTH, HEIGHT),
            Placement::Below {
                left: 120.0,
                top: 226.0
            }
        );
    }

    #[test]
    fn the_card_opens_above_a_highlight_with_no_room_below() {
        assert_eq!(
            place(rect(120.0, 700.0, 300.0, 720.0), WIDTH, HEIGHT),
            Placement::Above {
                left: 120.0,
                bottom: 106.0
            }
        );
    }

    #[test]
    fn the_card_stays_inside_the_window() {
        // Too far right for the card's width.
        assert_eq!(
            place(rect(900.0, 200.0, 980.0, 220.0), WIDTH, HEIGHT),
            Placement::Below {
                left: WIDTH - CARD_WIDTH - MARGIN,
                top: 226.0
            }
        );
        // Scrolled above the window: the card waits at the top.
        assert_eq!(
            place(rect(-20.0, -300.0, 100.0, -280.0), WIDTH, HEIGHT),
            Placement::Below {
                left: MARGIN,
                top: MARGIN
            }
        );
        // A window too short for the card either way keeps it at the top.
        assert_eq!(
            place(rect(10.0, 60.0, 100.0, 80.0), WIDTH, 150.0),
            Placement::Below {
                left: 10.0,
                top: MARGIN
            }
        );
    }

    #[test]
    fn a_rect_is_read_as_the_page_measures_it() {
        let rect: Rect =
            serde_json::from_str(r#"{"left":1.5,"top":2,"right":30,"bottom":40.25}"#).unwrap();
        assert_eq!(rect, self::rect(1.5, 2.0, 30.0, 40.25));
    }
    fn with_state(f: impl FnOnce(AppState)) {
        fn app() -> Element {
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::APP, || {
            let mut state = AppState::default();
            state
                .document
                .set(crate::state::Document::new("/tmp/shown.md"));
            f(state);
        });
    }

    #[test]
    fn the_card_opens_on_the_document_it_was_asked_for() {
        with_state(|state| {
            let id = HighlightId::from("h".to_string());
            open(
                state,
                "/tmp/shown.md".into(),
                id.clone(),
                rect(0.0, 0.0, 1.0, 1.0),
            );
            assert_eq!(
                state.highlight_card.peek().as_ref().map(|card| &card.id),
                Some(&id)
            );
        });
    }

    #[test]
    fn an_answer_overtaken_by_a_card_opened_since_is_dropped() {
        with_state(|state| {
            let asked = count_ask(state);
            let clicked = HighlightId::from("clicked".to_string());
            open(
                state,
                "/tmp/shown.md".into(),
                clicked.clone(),
                rect(0.0, 0.0, 1.0, 1.0),
            );

            let late = HighlightId::from("late".to_string());
            answer(
                state,
                asked,
                "/tmp/shown.md".into(),
                late,
                rect(0.0, 0.0, 1.0, 1.0),
            );

            assert_eq!(
                state.highlight_card.peek().as_ref().map(|card| &card.id),
                Some(&clicked)
            );
        });
    }

    #[test]
    fn an_answer_arriving_after_escape_opens_nothing() {
        with_state(|mut state| {
            let asked = count_ask(state);
            state.dismiss_overlays();

            let id = HighlightId::from("h".to_string());
            answer(
                state,
                asked,
                "/tmp/shown.md".into(),
                id,
                rect(0.0, 0.0, 1.0, 1.0),
            );

            assert_eq!(*state.highlight_card.peek(), None);
        });
    }

    #[test]
    fn an_answer_to_the_last_ask_opens_its_card() {
        with_state(|state| {
            let asked = count_ask(state);
            let id = HighlightId::from("h".to_string());
            answer(
                state,
                asked,
                "/tmp/shown.md".into(),
                id.clone(),
                rect(0.0, 0.0, 1.0, 1.0),
            );

            assert_eq!(
                state.highlight_card.peek().as_ref().map(|card| &card.id),
                Some(&id)
            );
        });
    }

    #[test]
    fn the_card_stays_shut_once_the_reader_has_moved_to_another_document() {
        with_state(|state| {
            let id = HighlightId::from("h".to_string());
            open(state, "/tmp/left.md".into(), id, rect(0.0, 0.0, 1.0, 1.0));
            assert_eq!(*state.highlight_card.peek(), None);
        });
    }
}
