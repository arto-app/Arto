use crate::config::{WindowDimension, WindowDimensionUnit};
use dioxus::prelude::*;

#[component]
pub fn DimensionInput(
    value: WindowDimension,
    on_change: EventHandler<WindowDimension>,
    min: f64,
    step: f64,
    allow_negative_pixels: bool,
) -> Element {
    let current_unit = value.unit;
    let handle_value_change = move |evt: Event<FormData>| {
        let input = evt.value();
        let parsed = input.parse::<f64>().unwrap_or(value.value);
        let next = WindowDimension {
            value: parsed,
            unit: current_unit,
        }
        .clamp_percent();
        on_change.call(next);
    };

    let handle_unit_change = move |evt: Event<FormData>| {
        let unit = match evt.value().as_str() {
            "percent" => WindowDimensionUnit::Percent,
            _ => WindowDimensionUnit::Pixels,
        };
        let next = WindowDimension {
            value: value.value,
            unit,
        }
        .clamp_percent();
        on_change.call(next);
    };

    let max_value = (current_unit == WindowDimensionUnit::Percent).then_some(100.0);
    let min_value = if current_unit == WindowDimensionUnit::Percent || !allow_negative_pixels {
        Some(min)
    } else {
        None
    };

    rsx! {
        div {
            class: "dimension-input",
            input {
                r#type: "number",
                inputmode: "decimal",
                min: min_value.map(|v| v.to_string()),
                max: max_value.map(|v| v.to_string()),
                step: "{step}",
                value: "{value.value}",
                oninput: handle_value_change,
            }
            select {
                class: "dimension-select",
                value: match current_unit {
                    WindowDimensionUnit::Pixels => "pixels",
                    WindowDimensionUnit::Percent => "percent",
                },
                onchange: handle_unit_change,
                option { value: "pixels", "px" }
                option { value: "percent", "%" }
            }
        }
    }
}
