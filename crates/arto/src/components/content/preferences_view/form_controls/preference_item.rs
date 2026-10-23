use dioxus::prelude::*;

/// A setting whose control sits under its label and description.
///
/// For a control too wide to share a line with its label — a slider, a set
/// of cards, a pair of dimension fields. One that fits beside it is a
/// [`super::ToggleRow`] or a [`super::ChoiceRow`].
#[component]
pub fn PreferenceItem(label: String, description: String, children: Element) -> Element {
    rsx! {
        div {
            class: "preference-item",
            div {
                class: "preference-item-header",
                label { "{label}" }
                p { class: "preference-description", "{description}" }
            }
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_ssr::render_element;

    #[test]
    fn a_label_and_its_description_head_the_control() {
        let html = render_element(rsx! {
            PreferenceItem {
                label: "Default size".to_string(),
                description: "How large a window opens.".to_string(),
                span { "control" }
            }
        });

        assert_eq!(
            html,
            concat!(
                r#"<div class="preference-item">"#,
                r#"<div class="preference-item-header">"#,
                r#"<label>Default size</label>"#,
                r#"<p class="preference-description">How large a window opens.</p>"#,
                r#"</div>"#,
                r#"<span>control</span>"#,
                r#"</div>"#,
            )
        );
    }
}
