use super::ResetLine;
use dioxus::prelude::*;

/// One on/off setting: what it is on the left, a switch on the right.
///
/// A setting that is genuinely on or off says so in its label, so a pair of
/// cards spelling out both states would be two boxes to read where one word
/// and a switch will do — and a pane of ten of them would be unreadable. The
/// control is a real checkbox, so it focuses, takes the space bar, and gets
/// the platform's focus ring for free.
#[component]
pub fn ToggleRow(
    label: String,
    description: Option<String>,
    checked: bool,
    on_change: EventHandler<bool>,
    /// The state Arto ships this setting in.
    shipped: Option<bool>,
) -> Element {
    let reset_to = shipped
        .filter(|shipped| shipped != &checked)
        .map(|shipped| if shipped { "on" } else { "off" }.to_string());

    rsx! {
        div {
            class: "preference-item",
            label {
                class: "preference-row",
                div {
                    class: "preference-row-text",
                    span { class: "preference-row-label", "{label}" }
                    if let Some(description) = &description {
                        span { class: "preference-description", "{description}" }
                    }
                }
                input {
                    class: "toggle-switch",
                    r#type: "checkbox",
                    role: "switch",
                    checked,
                    onchange: move |evt| on_change.call(evt.checked()),
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
}
