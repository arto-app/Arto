use dioxus::prelude::*;
use rust_i18n::t;

use crate::components::icon::{Icon, IconName};
use crate::config::PinPanelOn;
use crate::state::{AppState, Face};

/// How long the pointer has to stay on the rail before the panel peeks.
///
/// Short enough to feel immediate, long enough that crossing the rail on the
/// way somewhere else does not open anything.
const PEEK_DWELL_MS: u64 = 120;

/// The panel's spine, and the only part of it that never goes away.
///
/// It has two jobs at once. It switches the panel between its faces, and it is
/// the thing the pointer aims at to bring the panel back — which is what makes
/// opening on hover safe here when it was not safe at the window's edge. An
/// invisible strip cannot tell "I want the panel" apart from "I am moving past
/// this"; a visible 40px band that exists for the purpose can.
///
/// It carries no surface of its own. While the panel is out it shares the
/// panel's ground and there is no line between them; collapsed, it is the
/// document's own ground with a few marks in it.
#[component]
pub fn Rail(on_peek: EventHandler<Face>) -> Element {
    let mut state = use_context::<AppState>();
    let mut dwell = use_signal(|| 0u32);
    let clicks = use_signal(Clicks::default);
    let face = state.sidebar.read().face;
    // Two states, two marks. `showing` is "this is the face on screen" and is
    // said in ink; `held` is "and it is staying" and is said with the bar at
    // the rail's edge. One mark for both would leave a peek looking exactly
    // like a panel that had been pinned — which is the difference the reader
    // most needs to see, because it decides whether moving the pointer away
    // takes the panel with it.
    let showing = state.panel_is_showing();
    let held = state.sidebar.read().pinned && state.visible_chrome().panel;

    rsx! {
        div {
            class: "left-rail",
            class: if showing { "open" },
            // Dwell, not entry: passing over the rail on the way somewhere
            // else leaves before this fires, and nothing opens. The rail
            // itself peeks whatever face is current; a glyph peeks its own,
            // which is what makes resting on Starred worth doing while the
            // panel is showing the places.
            onmouseenter: move |_| peek(dwell, on_peek, face),
            onmouseleave: move |_| {
                dwell.set(dwell() + 1);
            },

            RailButton {
                icon: IconName::Folder,
                label: t!("sidebar.rail.places").to_string(),
                active: showing && face == Face::Places,
                held: held && face == Face::Places,
                on_click: move |_| clicked(state, on_peek, clicks, Face::Places),
                on_double_click: move |_| double_clicked(state, on_peek, clicks, Face::Places),
                on_dwell: move |_| peek(dwell, on_peek, Face::Places),
            }
            RailButton {
                icon: IconName::Star,
                label: t!("sidebar.rail.starred").to_string(),
                active: showing && face == Face::Starred,
                held: held && face == Face::Starred,
                on_click: move |_| clicked(state, on_peek, clicks, Face::Starred),
                on_double_click: move |_| double_clicked(state, on_peek, clicks, Face::Starred),
                on_dwell: move |_| peek(dwell, on_peek, Face::Starred),
            }
            RailButton {
                icon: IconName::History,
                label: t!("sidebar.rail.recent").to_string(),
                active: showing && face == Face::Recent,
                held: held && face == Face::Recent,
                on_click: move |_| clicked(state, on_peek, clicks, Face::Recent),
                on_double_click: move |_| double_clicked(state, on_peek, clicks, Face::Recent),
                on_dwell: move |_| peek(dwell, on_peek, Face::Recent),
            }
            RailButton {
                icon: IconName::Link,
                label: t!("sidebar.rail.links").to_string(),
                active: showing && face == Face::Links,
                held: held && face == Face::Links,
                on_click: move |_| clicked(state, on_peek, clicks, Face::Links),
                on_double_click: move |_| double_clicked(state, on_peek, clicks, Face::Links),
                on_dwell: move |_| peek(dwell, on_peek, Face::Links),
            }

            div { class: "left-rail-spacer" }

            // Settings opens a window rather than a face, so resting on it
            // peeks nothing: the panel it would show is not this panel.
            RailButton {
                icon: IconName::Gear,
                label: t!("sidebar.rail.settings").to_string(),
                active: false,
                held: false,
                on_click: move |_| state.open_preferences(),
                on_double_click: move |_| {},
                on_dwell: move |_| {
                    dwell.set(dwell() + 1);
                },
            }
        }
    }
}

/// One click on a glyph: the face on screen when it landed, and the glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Click {
    before: Face,
    target: Face,
}

/// The last two clicks on the rail.
///
/// The WebView reports a double click as two clicks and then the double click,
/// with no count on the clicks themselves, and by then the first click has
/// already acted. A double click therefore asks what the click before the last
/// one found. Two clicks are enough and no clock is needed: the double click
/// always follows its own pair, whatever interval the system allows between
/// them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Clicks {
    previous: Option<Click>,
    last: Option<Click>,
}

impl Clicks {
    fn record(&mut self, before: Face, target: Face) {
        self.previous = self.last;
        self.last = Some(Click { before, target });
    }

    /// The face on screen before the first click of a double click on
    /// `target`, when both of its clicks landed there.
    fn before_double_click(&self, target: Face) -> Option<Face> {
        match (self.previous, self.last) {
            (Some(first), Some(second)) if first.target == target && second.target == target => {
                Some(first.before)
            }
            _ => None,
        }
    }
}

/// What pressing a glyph does to the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    /// Bring the face out over the document, as resting on it would.
    Peek(Face),
    /// Hold the face beside the document.
    Hold(Face),
    /// Put the panel away.
    Hide,
    Nothing,
}

/// What a single click means, by `sidebar.pinOn`.
///
/// With a click to pin, a click holds the face, and one on the face already
/// held puts the panel away. It answers to holding, never to peeking: resting
/// on a glyph is what brought that face up in the first place, so a click that
/// counted the peek as "already open" would close what the pointer had just
/// asked for.
///
/// With a double click to pin, a click is the same as resting on the glyph.
/// It is how a reader who has turned peeking on hover off asks for a look, and
/// a look goes away again when the pointer leaves. A held panel is the
/// exception: it is already out, so a click on another glyph switches what it
/// holds, and one on the face it holds leaves it be — closing it is the double
/// click's too.
fn click(pin_on: PinPanelOn, held: bool, current: Face, face: Face) -> Press {
    match (pin_on, held, current == face) {
        (PinPanelOn::Click, true, true) => Press::Hide,
        (PinPanelOn::Click, _, _) => Press::Hold(face),
        (PinPanelOn::DoubleClick, true, true) => Press::Nothing,
        (PinPanelOn::DoubleClick, true, false) => Press::Hold(face),
        (PinPanelOn::DoubleClick, false, _) => Press::Peek(face),
    }
}

/// What a double click means, by `sidebar.pinOn`.
///
/// With a click to pin, nothing beyond its two clicks. With a double click to
/// pin, hold this face, or put the panel away when it is the face already
/// held. `before` is the face that was on screen when the double click's
/// first click landed: that click may already have switched a held panel to this
/// face, and counting that as "already held" would close what was just asked
/// for.
fn double_click(pin_on: PinPanelOn, held: bool, before: Face, face: Face) -> Press {
    match pin_on {
        PinPanelOn::Click => Press::Nothing,
        PinPanelOn::DoubleClick if held && before == face => Press::Hide,
        PinPanelOn::DoubleClick => Press::Hold(face),
    }
}

fn pin_on() -> PinPanelOn {
    crate::config::CONFIG.read().sidebar.pin_on
}

fn is_held(state: &AppState) -> bool {
    state.sidebar.read().pinned && state.visible_chrome().panel
}

fn apply(mut state: AppState, on_peek: EventHandler<Face>, press: Press) {
    match press {
        Press::Peek(face) => on_peek.call(face),
        Press::Hold(face) => state.show_face(face),
        Press::Hide => state.hide_panel(),
        Press::Nothing => {}
    }
}

fn clicked(state: AppState, on_peek: EventHandler<Face>, mut clicks: Signal<Clicks>, face: Face) {
    let current = state.sidebar.read().face;
    clicks.write().record(current, face);
    apply(
        state,
        on_peek,
        click(pin_on(), is_held(&state), current, face),
    );
}

fn double_clicked(
    state: AppState,
    on_peek: EventHandler<Face>,
    mut clicks: Signal<Clicks>,
    face: Face,
) {
    let before = clicks
        .peek()
        .before_double_click(face)
        .unwrap_or(state.sidebar.read().face);
    clicks.set(Clicks::default());
    apply(
        state,
        on_peek,
        double_click(pin_on(), is_held(&state), before, face),
    );
}

/// Wait out the dwell, then peek `face`, unless the reader has turned peeking
/// on hover off.
///
/// `dwell` is the generation the wait is checked against. Every place the
/// pointer settles on the rail bumps it, so a pointer that has moved on
/// leaves a wait that can no longer open anything — which is what keeps a
/// glyph crossed on the way to another one from swapping the panel's face.
fn peek(mut dwell: Signal<u32>, on_peek: EventHandler<Face>, face: Face) {
    let generation = dwell() + 1;
    dwell.set(generation);
    if !crate::config::CONFIG.read().sidebar.peek_on_hover {
        return;
    }
    spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(PEEK_DWELL_MS)).await;
        if dwell() == generation {
            on_peek.call(face);
        }
    });
}

/// One mark on the rail.
///
/// The face on screen is one step up the same opacity scale everything else
/// moves along. The face being *held* there adds a neutral bar at the rail's
/// edge — position rather than colour, since a glyph's own ink varies too much
/// for density alone to carry two things at once.
#[component]
fn RailButton(
    icon: IconName,
    label: String,
    active: bool,
    held: bool,
    on_click: EventHandler<()>,
    on_double_click: EventHandler<()>,
    on_dwell: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "left-rail-button",
            class: if active { "active" },
            class: if held { "held" },
            title: "{label}",
            "aria-label": "{label}",
            onclick: move |_| on_click.call(()),
            ondoubleclick: move |_| on_double_click.call(()),
            // The rail is watching for the same thing one level up, and it
            // asks for the face that is already current. Both fire when the
            // pointer arrives on a glyph, and the one that fires last is the
            // one that decides — so this one keeps the event to itself.
            onmouseenter: move |evt: Event<MouseData>| {
                evt.stop_propagation();
                on_dwell.call(());
            },
            Icon { name: icon }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_double_click_answers_to_the_face_before_its_first_click() {
        // Places held, Recent double-clicked: the first click switched to
        // Recent, the second found Recent already there.
        let mut clicks = Clicks::default();
        clicks.record(Face::Places, Face::Recent);
        clicks.record(Face::Recent, Face::Recent);
        assert_eq!(clicks.before_double_click(Face::Recent), Some(Face::Places));
    }

    #[test]
    fn a_click_on_another_glyph_is_not_part_of_the_double_click() {
        // Places held, Recent clicked, then Places double-clicked: the double
        // click is about Places, which Recent's click had replaced.
        let mut clicks = Clicks::default();
        clicks.record(Face::Places, Face::Recent);
        clicks.record(Face::Recent, Face::Places);
        clicks.record(Face::Places, Face::Places);
        assert_eq!(clicks.before_double_click(Face::Places), Some(Face::Recent));
    }

    #[test]
    fn a_double_click_without_two_clicks_on_its_glyph_has_nothing_to_answer_to() {
        let mut clicks = Clicks::default();
        assert_eq!(clicks.before_double_click(Face::Places), None);
        clicks.record(Face::Places, Face::Places);
        assert_eq!(clicks.before_double_click(Face::Places), None);
        clicks.record(Face::Places, Face::Recent);
        assert_eq!(clicks.before_double_click(Face::Recent), None);
    }

    #[test]
    fn with_a_click_to_pin_a_click_holds_the_face() {
        assert_eq!(
            click(PinPanelOn::Click, false, Face::Places, Face::Starred),
            Press::Hold(Face::Starred)
        );
        assert_eq!(
            click(PinPanelOn::Click, true, Face::Places, Face::Recent),
            Press::Hold(Face::Recent)
        );
    }

    #[test]
    fn with_a_click_to_pin_a_click_on_the_held_face_puts_the_panel_away() {
        assert_eq!(
            click(PinPanelOn::Click, true, Face::Places, Face::Places),
            Press::Hide
        );
    }

    #[test]
    fn with_a_click_to_pin_a_double_click_adds_nothing_to_its_clicks() {
        assert_eq!(
            double_click(PinPanelOn::Click, true, Face::Places, Face::Places),
            Press::Nothing
        );
        assert_eq!(
            double_click(PinPanelOn::Click, false, Face::Places, Face::Starred),
            Press::Nothing
        );
    }

    #[test]
    fn with_a_double_click_to_pin_a_click_peeks_a_panel_that_is_not_held() {
        assert_eq!(
            click(PinPanelOn::DoubleClick, false, Face::Places, Face::Starred),
            Press::Peek(Face::Starred)
        );
        assert_eq!(
            click(PinPanelOn::DoubleClick, false, Face::Places, Face::Places),
            Press::Peek(Face::Places)
        );
    }

    #[test]
    fn with_a_double_click_to_pin_a_click_switches_the_face_of_a_held_panel_and_keeps_it_held() {
        assert_eq!(
            click(PinPanelOn::DoubleClick, true, Face::Places, Face::Recent),
            Press::Hold(Face::Recent)
        );
    }

    #[test]
    fn with_a_double_click_to_pin_a_click_on_the_held_face_leaves_it_alone() {
        assert_eq!(
            click(PinPanelOn::DoubleClick, true, Face::Places, Face::Places),
            Press::Nothing
        );
    }

    #[test]
    fn with_a_double_click_to_pin_a_double_click_holds_the_face() {
        assert_eq!(
            double_click(PinPanelOn::DoubleClick, false, Face::Places, Face::Starred),
            Press::Hold(Face::Starred)
        );
        assert_eq!(
            double_click(PinPanelOn::DoubleClick, false, Face::Places, Face::Places),
            Press::Hold(Face::Places)
        );
    }

    #[test]
    fn with_a_double_click_to_pin_a_double_click_on_the_face_already_held_puts_the_panel_away() {
        assert_eq!(
            double_click(PinPanelOn::DoubleClick, true, Face::Places, Face::Places),
            Press::Hide
        );
    }

    #[test]
    fn with_a_double_click_to_pin_a_double_click_that_switched_a_held_face_keeps_it_held() {
        // The face shown before the gesture is what counts: its first click
        // already switched to Recent, and that is not Recent being held.
        assert_eq!(
            double_click(PinPanelOn::DoubleClick, true, Face::Places, Face::Recent),
            Press::Hold(Face::Recent)
        );
    }
}
