use super::{ChoiceItem, ResetLine};
use dioxus::prelude::*;

/// One choice out of a list too long for a [`super::ChoiceRow`], as a select
/// beside its label.
///
/// The options are told apart by their position, so a value need not be
/// something a form can carry. Children are drawn under the row, above the
/// way back to the shipped value.
#[component]
pub fn SelectRow<T: PartialEq + Clone + 'static>(
    label: String,
    description: Option<String>,
    options: Vec<ChoiceItem<T>>,
    selected: T,
    on_change: EventHandler<T>,
    /// The value Arto ships with, named by its own option.
    shipped: Option<T>,
    children: Element,
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
                select {
                    class: "preference-select",
                    aria_label: "{label}",
                    onchange: {
                        let options = options.clone();
                        move |evt: FormEvent| {
                            let chosen = evt.value().parse::<usize>().ok();
                            if let Some(option) = chosen.and_then(|index| options.get(index)) {
                                on_change.call(option.value.clone());
                            }
                        }
                    },
                    for (index, option) in options.iter().enumerate() {
                        option {
                            key: "{index}",
                            value: "{index}",
                            selected: option.value == selected,
                            "{option.label}"
                        }
                    }
                }
            }
            {children}
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

#[cfg(test)]
mod tests {
    use super::super::ChoiceItem;
    use super::*;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn options() -> Vec<ChoiceItem<u8>> {
        vec![
            ChoiceItem {
                value: 1,
                label: "One".to_string(),
            },
            ChoiceItem {
                value: 2,
                label: "Two".to_string(),
            },
        ]
    }

    #[test]
    fn the_list_sits_beside_its_label_with_the_selection_marked() {
        let html = render(|| {
            rsx! {
                SelectRow {
                    label: "Number".to_string(),
                    description: "How many.".to_string(),
                    options: options(),
                    selected: 2,
                    on_change: |_| {},
                    shipped: 2,
                }
            }
        });

        assert_eq!(
            html,
            concat!(
                r#"<div class="preference-item">"#,
                r#"<div class="preference-row">"#,
                r#"<div class="preference-row-text">"#,
                r#"<span class="preference-row-label">Number</span>"#,
                r#"<span class="preference-description">How many.</span>"#,
                r#"</div>"#,
                r#"<select class="preference-select" aria-label="Number">"#,
                r#"<option value="0">One</option>"#,
                r#"<option value="1" selected=true>Two</option>"#,
                r#"</select>"#,
                r#"</div>"#,
                r#"</div>"#,
            )
        );
    }

    #[test]
    fn what_follows_the_row_comes_before_the_way_back() {
        let html = render(|| {
            rsx! {
                SelectRow {
                    label: "Number".to_string(),
                    description: "How many.".to_string(),
                    options: options(),
                    selected: 1,
                    on_change: |_| {},
                    shipped: 2,
                    span { "note" }
                }
            }
        });

        let note = html.find("<span>note</span>").expect("the note is drawn");
        let reset = html
            .find(r#"class="preference-reset-line""#)
            .expect("the way back is drawn");
        assert!(note < reset, "{html}");
    }
}
