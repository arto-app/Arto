use super::super::form_controls::{DimensionInput, OptionCardItem, OptionCards};
use crate::config::{
    Config, NewWindowBehavior, StartupBehavior, WindowDimension, WindowDimensionUnit, WindowSize,
};
use dioxus::prelude::*;
use dioxus_desktop::window;

#[component]
pub fn WindowSizeTab(config: Signal<Config>, has_changes: Signal<bool>) -> Element {
    let window_config = config.read().window_size.clone();
    let use_current_size = move |_| {
        let metrics = crate::window::metrics::capture_window_metrics(&window().window);
        config.write().window_size.default_size = WindowSize {
            width: WindowDimension {
                value: metrics.size.width as f64,
                unit: WindowDimensionUnit::Pixels,
            },
            height: WindowDimension {
                value: metrics.size.height as f64,
                unit: WindowDimensionUnit::Pixels,
            },
        };
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
                    label { "Default Size" }
                    p { class: "preference-description", "Set the default window size. Percent values are relative to the current screen." }
                }
                div {
                    class: "dimension-row",
                    div {
                        class: "dimension-grid",
                        div {
                            class: "dimension-field",
                            label { "Width" }
                            DimensionInput {
                                value: window_config.default_size.width,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: false,
                                on_change: move |new_value| {
                                    config.write().window_size.default_size.width = new_value;
                                    has_changes.set(true);
                                },
                            }
                        }
                        div {
                            class: "dimension-field",
                            label { "Height" }
                            DimensionInput {
                                value: window_config.default_size.height,
                                min: 0.0,
                                step: 1.0,
                                allow_negative_pixels: false,
                                on_change: move |new_value| {
                                    config.write().window_size.default_size.height = new_value;
                                    has_changes.set(true);
                                },
                            }
                        }
                    }
                    button {
                        class: "use-current-button",
                        onclick: use_current_size,
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
                    p { class: "preference-description", "Which window size to use when the application starts." }
                }
                OptionCards {
                    name: "window-size-startup".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: StartupBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default window size".to_string()),
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
                        config.write().window_size.on_startup = new_behavior;
                        has_changes.set(true);
                    },
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "On New Window" }
                    p { class: "preference-description", "Which window size to use for new windows." }
                }
                OptionCards {
                    name: "window-size-new-window".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: NewWindowBehavior::Default,
                            title: "Default".to_string(),
                            description: Some("Use default window size".to_string()),
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
                        config.write().window_size.on_new_window = new_behavior;
                        has_changes.set(true);
                    },
                }
            }
        }
    }
}
