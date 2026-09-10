use dioxus::prelude::*;

use super::ResetLine;
use crate::config::ColorTheme;

/// A radio group of GitHub's themes, each card showing the palette it selects.
///
/// The swatch is not a picture of the theme, it is the theme: the card carries
/// `data-theme`, so the same token block that would paint the whole app paints
/// these few boxes instead. A reader can therefore tell dimmed from default,
/// or see what a colour-vision theme does, without applying it first.
///
/// Only the themes matching this slot are shown at first. Picking a light
/// theme for dark mode is allowed — GitHub allows it too — but it is a rare
/// thing to want, and offering all nine at once buries the four or five that
/// are actually being chosen between.
#[component]
pub fn ThemePicker(
    name: String,
    selected: ColorTheme,
    /// Whether this slot paints dark mode.
    dark_mode: bool,
    on_change: EventHandler<ColorTheme>,
    /// The theme Arto ships in this slot.
    shipped: Option<ColorTheme>,
) -> Element {
    let reset_to = shipped
        .filter(|shipped| shipped != &selected)
        .map(|shipped| shipped.label().to_string());

    let (matching, others): (Vec<ColorTheme>, Vec<ColorTheme>) = ColorTheme::ALL
        .into_iter()
        .partition(|theme| theme.is_dark() == dark_mode);

    // A theme already chosen from the other side has to stay visible, so it
    // cannot be folded away again.
    let pinned_open = selected.is_dark() != dark_mode;
    let mut expanded = use_signal(|| pinned_open);
    let showing_others = pinned_open || expanded();

    rsx! {
        div {
            class: "option-cards theme-cards",
            for theme in matching {
                ThemeCard { name: name.clone(), theme, selected, on_change }
            }
            if showing_others {
                for theme in others {
                    ThemeCard { name: name.clone(), theme, selected, on_change }
                }
            }
        }
        if !pinned_open {
            button {
                r#type: "button",
                class: "theme-cards-toggle",
                onclick: move |_| expanded.toggle(),
                if showing_others {
                    if dark_mode { "Hide light themes" } else { "Hide dark themes" }
                } else if dark_mode {
                    "Show light themes too"
                } else {
                    "Show dark themes too"
                }
            }
        }
        ResetLine {
            shipped: reset_to,
            on_reset: move |_| {
                if let Some(shipped) = shipped {
                    on_change.call(shipped);
                }
            },
        }
    }
}

#[component]
fn ThemeCard(
    name: String,
    theme: ColorTheme,
    selected: ColorTheme,
    on_change: EventHandler<ColorTheme>,
) -> Element {
    rsx! {
        label {
            class: "option-card theme-card",
            class: if theme == selected { "selected" },
            input {
                r#type: "radio",
                name: "{name}",
                checked: theme == selected,
                onchange: move |_| on_change.call(theme),
            }
            span {
                class: "theme-preview",
                "data-theme": theme.as_str(),
                span { class: "theme-preview-chrome" }
                span {
                    class: "theme-preview-body",
                    span { class: "theme-preview-line" }
                    span { class: "theme-preview-line theme-preview-line--muted" }
                    // The semantic hues, which is where the themes actually
                    // differ: the colour-vision ones recolour success and
                    // danger, and high contrast deepens every one of them.
                    span {
                        class: "theme-preview-hues",
                        span { class: "theme-preview-hue theme-preview-hue--accent" }
                        span { class: "theme-preview-hue theme-preview-hue--success" }
                        span { class: "theme-preview-hue theme-preview-hue--attention" }
                        span { class: "theme-preview-hue theme-preview-hue--danger" }
                        span { class: "theme-preview-hue theme-preview-hue--done" }
                    }
                }
            }
            span { class: "option-card-title", "{theme.label()}" }
        }
    }
}
