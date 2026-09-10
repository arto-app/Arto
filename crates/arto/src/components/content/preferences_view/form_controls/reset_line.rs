use dioxus::prelude::*;

/// The way back to the value Arto ships with.
///
/// A setting only raises the question "what was this before I touched it?"
/// while it is not on the shipped value, so that is the only time the button
/// exists — and it names the value rather than hiding it in a tooltip, which
/// is the same answer given twice: what the default is, and how to take it.
///
/// Every control formats the value itself, out of what it already knows — a
/// slider from its unit and decimals, a card group from the title on the card
/// — so no call site restates a label that is written somewhere else.
#[component]
pub fn ResetLine(
    /// How the shipped value reads. `None` when the setting is already on it,
    /// or when there is nothing to offer.
    shipped: Option<String>,
    on_reset: EventHandler<()>,
) -> Element {
    let Some(shipped) = shipped else {
        return rsx! {};
    };

    rsx! {
        div {
            class: "preference-reset-line",
            button {
                r#type: "button",
                class: "preference-reset",
                onclick: move |_| on_reset.call(()),
                "Reset to {shipped}"
            }
        }
    }
}
