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
pub fn Rail(on_peek: EventHandler<()>) -> Element {
    let mut state = use_context::<AppState>();
    let mut dwell = use_signal(|| 0u32);
    let face = state.sidebar.read().face;
    // A face is marked only while its panel is actually on screen: with the
    // panel away there is no section selected, so the rail carries no
    // indicator at all.
    let showing = state.panel_is_showing();

    rsx! {
        div {
            class: "left-rail",
            class: if showing { "open" },
            // Dwell, not entry: passing over the rail on the way somewhere
            // else leaves before this fires, and nothing opens.
            onmouseenter: move |_| {
                let generation = dwell() + 1;
                dwell.set(generation);
                spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(PEEK_DWELL_MS)).await;
                    if dwell() == generation {
                        on_peek.call(());
                    }
                });
            },
            onmouseleave: move |_| {
                dwell.set(dwell() + 1);
            },

            RailButton {
                icon: IconName::Folder,
                label: "Files",
                active: showing && face == Face::Files,
                on_click: move |_| state.toggle_face(Face::Files),
            }
            RailButton {
                icon: IconName::History,
                label: "Recent",
                active: showing && face == Face::Recent,
                on_click: move |_| state.toggle_face(Face::Recent),
            }
            RailButton {
                icon: IconName::StarFilled,
                label: "Starred",
                active: showing && face == Face::Starred,
                on_click: move |_| state.toggle_face(Face::Starred),
            }

            div { class: "left-rail-spacer" }

            RailButton {
                icon: IconName::Gear,
                label: "Settings",
                active: false,
                on_click: move |_| state.open_preferences(),
            }
        }
    }
}

/// One mark on the rail.
///
/// Selected is one step up the same opacity scale everything else moves along,
/// plus a neutral bar at the rail's edge — position rather than colour, since a
/// glyph's own ink varies too much for density alone to rank them.
#[component]
fn RailButton(
    icon: IconName,
    label: &'static str,
    active: bool,
    on_click: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "left-rail-button",
            class: if active { "active" },
            title: "{label}",
            "aria-label": "{label}",
            onclick: move |_| on_click.call(()),
            Icon { name: icon }
        }
    }
}
