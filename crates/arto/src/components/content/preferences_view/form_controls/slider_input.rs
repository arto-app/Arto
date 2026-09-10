use super::ResetLine;
use dioxus::prelude::*;

/// Slider input component with numeric input and optional action button.
///
/// - `current_value`: When Some, shows "Use Current" button (for default settings).
/// - `default_value`: When Some, shows "Use Default" button (for current settings).
/// - When both are None, no button is shown.
/// - `shipped`: the value Arto ships with, which the slider offers a way back
///   to whenever it is not on it. Formatted here rather than by the caller,
///   from the same unit and decimals the field is read in.
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
    shipped: Option<f64>,
    #[props(default = 0)] decimals: u32,
) -> Element {
    // A number typed digit by digit passes through values the setting does
    // not allow — "3" on the way to "300" in a field that starts at 200 — and
    // clamping one of those would rewrite the field under the reader, leaving
    // the number they were aiming at untypable. Such a value is held here
    // instead and clamped once the field is left.
    let mut out_of_range = use_signal(|| None::<f64>);

    let handle_number_input = move |evt: Event<FormData>| {
        let Ok(typed) = evt.value().parse::<f64>() else {
            return;
        };
        if (min..=max).contains(&typed) {
            out_of_range.set(None);
            on_change.call(typed);
        } else {
            out_of_range.set(Some(typed));
        }
    };

    let handle_number_blur = move |_| {
        let typed = *out_of_range.read();
        if let Some(typed) = typed {
            out_of_range.set(None);
            on_change.call(typed.clamp(min, max));
        }
    };

    // Round instead of truncate to match slider/state values
    let display_value = format!("{:.prec$}", value, prec = decimals as usize);

    let reset_to = shipped.filter(|shipped| shipped != &value).map(|shipped| {
        format!(
            "{:.prec$}{unit}",
            shipped,
            prec = decimals as usize,
            unit = unit
        )
    });

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
                    onblur: handle_number_blur,
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
