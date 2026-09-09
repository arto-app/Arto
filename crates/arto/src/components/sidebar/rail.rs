use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
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
                label: "Places",
                active: showing && face == Face::Places,
                held: held && face == Face::Places,
                on_click: move |_| press(state, Face::Places),
                on_dwell: move |_| peek(dwell, on_peek, Face::Places),
            }
            RailButton {
                icon: IconName::Star,
                label: "Starred",
                active: showing && face == Face::Starred,
                held: held && face == Face::Starred,
                on_click: move |_| press(state, Face::Starred),
                on_dwell: move |_| peek(dwell, on_peek, Face::Starred),
            }
            RailButton {
                icon: IconName::History,
                label: "Recent",
                active: showing && face == Face::Recent,
                held: held && face == Face::Recent,
                on_click: move |_| press(state, Face::Recent),
                on_dwell: move |_| peek(dwell, on_peek, Face::Recent),
            }

            div { class: "left-rail-spacer" }

            // Settings opens a window rather than a face, so resting on it
            // peeks nothing: the panel it would show is not this panel.
            RailButton {
                icon: IconName::Gear,
                label: "Settings",
                active: false,
                held: false,
                on_click: move |_| state.open_preferences(),
                on_dwell: move |_| {
                    dwell.set(dwell() + 1);
                },
            }
        }
    }
}

/// What a click on a glyph means: hold this face open, or put the panel away
/// when this is the face already being held.
///
/// It answers to holding, never to peeking. Resting on a glyph is what brought
/// that face up in the first place, so a click that counted the peek as
/// "already open" would close what the pointer had just asked for — and take
/// two more clicks to bring it back.
fn press(mut state: AppState, face: Face) {
    let held = state.sidebar.read().pinned && state.visible_chrome().panel;
    if held && state.sidebar.read().face == face {
        state.hide_panel();
    } else {
        state.show_face(face);
    }
}

/// Wait out the dwell, then peek `face`.
///
/// `dwell` is the generation the wait is checked against. Every place the
/// pointer settles on the rail bumps it, so a pointer that has moved on
/// leaves a wait that can no longer open anything — which is what keeps a
/// glyph crossed on the way to another one from swapping the panel's face.
fn peek(mut dwell: Signal<u32>, on_peek: EventHandler<Face>, face: Face) {
    let generation = dwell() + 1;
    dwell.set(generation);
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
    label: &'static str,
    active: bool,
    held: bool,
    on_click: EventHandler<()>,
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
