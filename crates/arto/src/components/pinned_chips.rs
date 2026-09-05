//! Pinned search chips component for the search bar.
//!
//! Displays pinned search items as compact chips with color and settings popover.

use dioxus::prelude::*;

use crate::components::icon::{Icon, IconName};
use crate::pinned_search::{
    remove_pinned_search, set_pinned_search_color, toggle_pinned_search_disabled, HighlightColor,
    PinnedSearch,
};

/// Pinned chips row displayed below the search bar.
///
/// Returns `None` if there are no pinned searches (hiding the row).
#[component]
pub fn PinnedChipsRow(pinned_searches: Vec<PinnedSearch>) -> Element {
    if pinned_searches.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "search-bar-pinned",
            span {
                class: "search-bar-pinned-label",
                Icon { name: IconName::Pin, size: 16 }
            }
            div {
                class: "pinned-chips",
                for pinned in pinned_searches.iter() {
                    PinnedChip {
                        key: "{pinned.id}",
                        pinned: pinned.clone(),
                    }
                }
            }
        }
    }
}

/// A single pinned search chip with settings popover.
#[component]
pub fn PinnedChip(pinned: PinnedSearch) -> Element {
    let mut show_popover = use_signal(|| false);
    let id = pinned.id.clone();
    let pattern = pinned.pattern.clone();
    let color = pinned.color;
    let disabled = pinned.disabled;

    let chip_class = if disabled {
        format!("pinned-chip disabled {}", color.css_class())
    } else {
        format!("pinned-chip {}", color.css_class())
    };

    rsx! {
        div {
            class: "{chip_class}",

            // Chip body (clickable to show edit popover)
            button {
                class: "pinned-chip-body",
                title: "Edit",
                onclick: move |e| {
                    e.stop_propagation();
                    show_popover.toggle();
                },
                "{pattern}"
            }

            // Remove button (X)
            button {
                class: "pinned-chip-remove",
                title: "Remove",
                onclick: {
                    let id = id.clone();
                    move |e| {
                        e.stop_propagation();
                        remove_pinned_search(&id);
                    }
                },
                Icon { name: IconName::Close, size: 12 }
            }

            // Edit popover
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
