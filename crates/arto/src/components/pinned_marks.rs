//! The marks kept on this document, listed where their colour already shows.
//!
//! A pinned search is a standing highlight: it outlives the field that made
//! it, and its position is already drawn on the contents' ticks. So the list
//! of them heads the contents, and everything done to one — its colour,
//! whether it is showing, taking it away — is done there.

use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::pinned_search::{
    remove_pinned_search, set_pinned_search_color, toggle_pinned_search_disabled, HighlightColor,
    PinnedSearch,
};
use crate::state::AppState;

/// The pinned marks, above the headings.
#[component]
pub fn PinnedMarks(pinned_searches: Vec<PinnedSearch>) -> Element {
    if pinned_searches.is_empty() {
        return rsx! {};
    }

    rsx! {
        div { class: "contents-toc-label", "Pinned" }

        for pinned in pinned_searches.iter() {
            PinnedMark { key: "{pinned.id}", pinned: pinned.clone() }
        }
    }
}

/// One mark: its colour, its word, and the ways to change it.
#[component]
fn PinnedMark(pinned: PinnedSearch) -> Element {
    let state = use_context::<AppState>();
    let mut show_popover = use_signal(|| false);
    let id = pinned.id.clone();
    let count = state
        .pinned_matches
        .read()
        .get(&pinned.id)
        .map(|matches| matches.len())
        .unwrap_or_default();
    let pattern = pinned.pattern.clone();
    let color = pinned.color;
    let disabled = pinned.disabled;

    rsx! {
        // The popover hangs off this rather than off the row, so that a click
        // inside it is not also a click on the row that opened it.
        div {
            class: "contents-toc-pin-slot",

            // The row is the control, all of it: it lights up under the
            // pointer as one thing, so it has to answer as one thing.
            div {
                class: "contents-toc-row contents-toc-pin",
                class: if disabled { "disabled" },
                title: "Colour, visibility, remove",
                onclick: move |_| show_popover.toggle(),

                // Except the colour, which is the mark showing or not showing
                // on the page — the one thing here that is worth a click of
                // its own, and the thing the dot already says.
                button {
                    class: "contents-toc-pin-dot {color.css_class()}",
                    title: if disabled { "Show" } else { "Hide" },
                    onclick: {
                        let id = id.clone();
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            toggle_pinned_search_disabled(&id);
                        }
                    },
                }

                span { class: "contents-toc-name", "{pattern}" }

                span { class: "contents-toc-pin-count", "{count}" }
            }

            if *show_popover.read() {
                ColorPalettePopover {
                    current_color: color,
                    is_disabled: disabled,
                    on_select: {
                        let id = id.clone();
                        move |c| {
                            set_pinned_search_color(&id, c);
                            show_popover.set(false);
                        }
                    },
                    on_toggle: {
                        let id = id.clone();
                        move |_| {
                            toggle_pinned_search_disabled(&id);
                            show_popover.set(false);
                        }
                    },
                    on_remove: {
                        let id = id.clone();
                        move |_| {
                            remove_pinned_search(&id);
                            show_popover.set(false);
                        }
                    },
                    on_close: move |_| show_popover.set(false),
                }
            }
        }
    }
}

/// Color palette popover for pinned search settings.
#[component]
fn ColorPalettePopover(
    current_color: HighlightColor,
    is_disabled: bool,
    on_select: EventHandler<HighlightColor>,
    on_toggle: EventHandler<()>,
    on_remove: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        // Backdrop to close on outside click
        div {
            class: "color-palette-backdrop",
            onclick: move |e| {
                e.stop_propagation();
                on_close.call(());
            },
        }

        div {
            class: "color-palette-popover",

            // All controls in a single row: colors + toggle + remove
            for color in HighlightColor::ALL {
                button {
                    class: if color == current_color {
                        format!("color-palette-swatch selected {}", color.css_class())
                    } else {
                        format!("color-palette-swatch {}", color.css_class())
                    },
                    onclick: move |_| on_select.call(color),
                }
            }

            // Separator
            div { class: "color-palette-separator" }

            // Visibility toggle (Eye icon)
            button {
                class: "color-palette-action",
                title: if is_disabled { "Enable" } else { "Disable" },
                onclick: move |_| on_toggle.call(()),
                if is_disabled {
                    Icon { name: IconName::EyeOff, size: 18 }
                } else {
                    Icon { name: IconName::Eye, size: 18 }
                }
            }

            // Remove button
            button {
                class: "color-palette-action color-palette-remove",
                title: "Remove",
                onclick: move |_| on_remove.call(()),
                Icon { name: IconName::Trash, size: 18 }
            }
        }
    }
}
