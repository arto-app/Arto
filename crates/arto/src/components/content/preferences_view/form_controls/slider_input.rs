use dioxus::prelude::*;

/// Slider input component with numeric input and optional action button.
///
/// - `current_value`: When Some, shows "Use Current" button (for default settings).
/// - `default_value`: When Some, shows "Use Default" button (for current settings).
/// - When both are None, no button is shown.
#[component]
pub fn SliderInput(
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    unit: String,
    on_change: EventHandler<f64>,
    current_value: Option<f64>,
    default_value: Option<f64>,
    #[props(default = 0)] decimals: u32,
) -> Element {
    let handle_number_input = move |evt: Event<FormData>| {
        if let Ok(new_value) = evt.value().parse::<f64>() {
            let clamped = new_value.clamp(min, max);
            on_change.call(clamped);
        }
    };

    let display_value = if decimals == 0 {
        // Round instead of truncate to match slider/state values
        format!("{:.0}", value)
    } else {
        format!("{:.prec$}", value, prec = decimals as usize)
    };

    rsx! {
        div {
            class: "slider-input",
            input {
                r#type: "range",
                min: "{min}",
                max: "{max}",
                step: "{step}",
                value: "{value}",
                oninput: move |evt| {
                    if let Ok(new_value) = evt.value().parse::<f64>() {
                        on_change.call(new_value);
                    }
                },
            }
            div {
                class: "slider-value-input",
                input {
                    r#type: "number",
                    min: "{min}",
                    max: "{max}",
                    step: "{step}",
                    value: "{display_value}",
                    oninput: handle_number_input,
                }
                span { "{unit}" }
            }
            if let Some(current) = current_value {
                button {
                    class: "use-current-button",
                    onclick: move |_| on_change.call(current),
                    "Use Current"
                }
            } else if let Some(default) = default_value {
                button {
                    class: "use-current-button",
                    onclick: move |_| on_change.call(default),
                    "Use Default"
                }
            }
        }
    }
}
