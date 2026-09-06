use super::super::form_controls::{DimensionInput, OptionCardItem, OptionCards};
use crate::components::icon::IconName;
use crate::config::{
    Config, NewWindowBehavior, StartupBehavior, WindowDimension, WindowDimensionUnit,
    WindowPosition, WindowPositionMode,
};
use dioxus::prelude::*;
use dioxus_desktop::window;

#[component]
pub fn WindowPositionTab(config: Signal<Config>, has_changes: Signal<bool>) -> Element {
    let window_config = config.read().window_position.clone();
    let use_current_position = move |_| {
        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        let mut cfg = config.write();
        cfg.window_position.default_position = WindowPosition {
            x: WindowDimension {
                value: metrics.position.x as f64,
                unit: WindowDimensionUnit::Pixels,
            },
            y: WindowDimension {
                value: metrics.position.y as f64,
                unit: WindowDimensionUnit::Pixels,
            },
        };
        cfg.window_position.default_position_mode = WindowPositionMode::Coordinates;
        has_changes.set(true);
    };

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Default Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Position" }
                    p { class: "preference-description", "Set the default window position. Percent values place the window within the available screen area (0% = top/left, 100% = bottom/right)." }
                }
                OptionCards {
                    name: "window-position-mode".to_string(),
                    options: vec![
                        OptionCardItem {
                            value: WindowPositionMode::Coordinates,
                            icon: Some(IconName::Command),
                            title: "Coordinates".to_string(),
                            description: Some("Use the X/Y values below".to_string()),
                        },
                        OptionCardItem {
                            value: WindowPositionMode::Mouse,
                            icon: Some(IconName::Click),
                            title: "Mouse Position".to_string(),
                            description: Some("Open at the current mouse location".to_string()),
                        },
                    ],
                    selected: window_config.default_position_mode,
                    on_change: move |new_mode| {
                        config.write().window_position.default_position_mode = new_mode;
                        has_changes.set(true);
                    },
                }
                div {
                    class: "dimension-row spacing-top-sm",
                    div {
                        class: "dimension-grid",
                        div {
                            class: "dimension-field",
                            label { "X" }
                            DimensionInput {
                                value: window_config.default_position.x,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: true,
                                on_change: move |new_value| {
                                    config.write().window_position.default_position.x = new_value;
                                    has_changes.set(true);
                                },
                            }
                        }
                        div {
                            class: "dimension-field",
                            label { "Y" }
                            DimensionInput {
                                value: window_config.default_position.y,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: true,
                                on_change: move |new_value| {
                                    config.write().window_position.default_position.y = new_value;
                                    has_changes.set(true);
                                },
                            }
                        }
                    }
                    button {
                        class: "use-current-button",
                        onclick: use_current_position,
                        "Use Current"
                    }
                }
            }

            h3 { class: "preference-section-title", "Behavior" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On Startup" }
                    p { class: "preference-description", "Which window position to use when the application starts." }
                }
                OptionCards {
                    name: "window-position-startup".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default window position".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::LastClosed,
                            title: "Last Closed".to_string(),
                            description: Some("Resume from last closed window".to_string()),
                        },
                    ],
                    selected: window_config.on_startup,
                    on_change: move |new_behavior| {
                        config.write().window_position.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On New Window" }
                    p { class: "preference-description", "Which window position to use for new windows." }
                }
                OptionCards {
                    name: "window-position-new-window".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default window position".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::LastFocused,
                            title: "Last Focused".to_string(),
                            description: Some("Same as current window".to_string()),
                        },
                    ],
                    selected: window_config.on_new_window,
                    on_change: move |new_behavior| {
                        config.write().window_position.on_new_window = new_behavior;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Position Offset" }
                    p { class: "preference-description", "If another window already uses a nearby position, shift the new window by this offset (pixels only)." }
                }
                div {
                    class: "dimension-grid",
                    div {
                        class: "dimension-field",
                        label { "X" }
                        div {
                            class: "dimension-input",
                            input {
                                r#type: "number",
                                inputmode: "decimal",
                                min: "0",
                                step: "1",
                                value: "{window_config.position_offset.x}",
                                oninput: move |evt| {
                                    let fallback = config.read().window_position.position_offset.x;
                                    let value = evt.value().parse::<i32>().unwrap_or(fallback);
                                    config.write().window_position.position_offset.x = value.max(0);
                                    has_changes.set(true);
                                },
                            }
                            span { class: "dimension-unit", "px" }
                        }
                    }
                    div {
                        class: "dimension-field",
                        label { "Y" }
                        div {
                            class: "dimension-input",
                            input {
                                r#type: "number",
                                inputmode: "decimal",
                                min: "0",
                                step: "1",
                                value: "{window_config.position_offset.y}",
                                oninput: move |evt| {
                                    let fallback = config.read().window_position.position_offset.y;
                                    let value = evt.value().parse::<i32>().unwrap_or(fallback);
                                    config.write().window_position.position_offset.y = value.max(0);
                                    has_changes.set(true);
                                },
                            }
                            span { class: "dimension-unit", "px" }
                        }
                    }
                }
            }
        }
    }
}
