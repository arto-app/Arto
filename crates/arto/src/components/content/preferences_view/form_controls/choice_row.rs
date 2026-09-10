use super::ResetLine;
use dioxus::prelude::*;

/// One choice out of a few, stated in a row rather than in cards.
///
/// [`super::OptionCards`] gives every option a card because there the options
/// are the explanation. Here the label carries the question and each option is
/// a word or two, so a row of them keeps a pane of related choices comparable
/// down the page instead of a wall of boxes.
#[component]
pub fn ChoiceRow<T: PartialEq + Clone + 'static>(
    name: String,
    label: String,
    description: Option<String>,
    options: Vec<ChoiceItem<T>>,
    selected: T,
    on_change: EventHandler<T>,
    /// The value Arto ships with, named by its own segment.
    shipped: Option<T>,
) -> Element {
    let reset_to = shipped
        .as_ref()
        .filter(|value| *value != &selected)
        .and_then(|value| {
            options
                .iter()
                .find(|option| &option.value == value)
                .map(|option| option.label.clone())
        });

    rsx! {
        div {
            class: "preference-item",
            div {
                class: "preference-row",
                div {
                    class: "preference-row-text",
                    span { class: "preference-row-label", "{label}" }
                    if let Some(description) = &description {
                        span { class: "preference-description", "{description}" }
                    }
                }
                div {
                    class: "segmented",
                    role: "radiogroup",
                    for option in options {
                        {
                            let class_name = if option.value == selected {
                                "segment selected"
                            } else {
                                "segment"
                            };
                            rsx! {
                                label {
                                    class: "{class_name}",
                                    input {
                                        r#type: "radio",
                                        name: "{name}",
                                        checked: option.value == selected,
                                        onchange: {
                                            let value = option.value.clone();
                                            move |_| on_change.call(value.clone())
                                        },
                                    }
                                    span { "{option.label}" }
                                }
                            }
                        }
                    }
                }
            }
            ResetLine {
                shipped: reset_to,
                on_reset: move |_| {
                    if let Some(shipped) = shipped.clone() {
                        on_change.call(shipped);
                    }
                },
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ChoiceItem<T: Clone + PartialEq> {
    pub value: T,
    pub label: String,
}
