use crate::components::icon::{Icon, IconName};
use dioxus::prelude::*;

/// Reusable option cards component for radio button groups
///
/// Automatically adapts layout based on content:
/// - Text mode: title + optional description (horizontal layout)
/// - Icon mode: icon + label (vertical layout, automatically applied when icon is present)
#[component]
pub fn OptionCards<T: PartialEq + Clone + 'static>(
    name: String,
    options: Vec<OptionCardItem<T>>,
    selected: T,
    on_change: EventHandler<T>,
) -> Element {
    rsx! {
        div {
            class: "option-cards",
            for option in options {
                {
                    let has_icon = option.icon.is_some();
                    let base_class = if has_icon { "option-card option-card--icon" } else { "option-card" };
                    let class_name = if option.value == selected {
                        format!("{base_class} selected")
                    } else {
                        base_class.to_string()
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
                            if let Some(icon) = option.icon {
                                Icon { name: icon, size: 24 }
                            }
                            span { class: "option-card-title", "{option.title}" }
                            if let Some(desc) = &option.description {
                                span { class: "option-card-desc", "{desc}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Option card item data structure
#[derive(Clone, PartialEq)]
pub struct OptionCardItem<T: Clone + PartialEq> {
    pub value: T,
    pub icon: Option<IconName>,
    pub title: String,
    pub description: Option<String>,
}
