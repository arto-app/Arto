use super::super::form_controls::{OptionCardItem, OptionCards, SliderInput};
use crate::config::{
    normalize_content_zoom, Config, RecentTrace, MAX_CONTENT_ZOOM, MIN_CONTENT_WIDTH_RANGE,
    MIN_CONTENT_ZOOM, ZOOM_STEP,
};
use crate::events::SET_CONTENT_ZOOM_IN_WINDOW;
use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

/// The page itself: how large it is set, how much width it keeps, and what is
/// left in the margin beside it.
#[component]
pub fn ReadingTab(
    config: Signal<Config>,
    /// The window the "Current Settings" section acts on.
    window_id: WindowId,
    /// That window's zoom level, which this pane both shows and sets.
    mut current_zoom: Signal<f64>,
) -> Element {
    let zoom_cfg = config.read().zoom.clone();
    let sidebar_cfg = config.read().sidebar.clone();
    let defaults = Config::default();

    rsx! {
        div {
            class: "preferences-pane",

            h3 { class: "preference-section-title", "Current Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Current Zoom Level" }
                    p { class: "preference-description", "The zoom level for the current window's document." }
                }
                SliderInput {
                    value: current_zoom(),
                    min: MIN_CONTENT_ZOOM,
                    max: MAX_CONTENT_ZOOM,
                    step: ZOOM_STEP,
                    unit: "x".to_string(),
                    decimals: 1,
                    on_change: move |new_zoom| {
                        let normalized = normalize_content_zoom(new_zoom);
                        current_zoom.set(normalized);
                        let _ = SET_CONTENT_ZOOM_IN_WINDOW.send((window_id, normalized));
                    },
                    default_value: Some(zoom_cfg.default_zoom_level),
                }
            }

            h3 { class: "preference-section-title", "Default Settings" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Default Zoom Level" }
                    p { class: "preference-description", "The zoom level a document is set at when a window opens." }
                }
                SliderInput {
                    value: zoom_cfg.default_zoom_level,
                    min: MIN_CONTENT_ZOOM,
                    max: MAX_CONTENT_ZOOM,
                    step: ZOOM_STEP,
                    unit: "x".to_string(),
                    decimals: 1,
                    on_change: move |new_zoom| {
                        config.write().zoom.default_zoom_level = new_zoom;
                    },
                    current_value: Some(current_zoom()),
                    shipped: Some(defaults.zoom.default_zoom_level),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Minimum Content Width" }
                    p {
                        class: "preference-description",
                        "The width the document keeps while anything else can give way instead. Everything around the page folds at this number plus its own width, from the outside in: the margin trace, then the panel, then the contents gutter, then the rail. Raise it and a wide window folds them sooner."
                    }
                }
                SliderInput {
                    value: sidebar_cfg.min_content_width,
                    min: *MIN_CONTENT_WIDTH_RANGE.start(),
                    max: *MIN_CONTENT_WIDTH_RANGE.end(),
                    step: 10.0,
                    unit: "px".to_string(),
                    on_change: move |new_width| {
                        config.write().sidebar.min_content_width = new_width;
                    },
                    shipped: Some(defaults.sidebar.min_content_width),
                }
            }

            h3 { class: "preference-section-title", "Margin Trace" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "When it is drawn" }
                    p {
                        class: "preference-description",
                        "The documents read before this one, left at the edge of the page. It is the one window on the history nobody asks for, so it is the one you can turn off. A window too narrow to keep the document readable hides it whatever is chosen here."
                    }
                }
                OptionCards {
                    name: "reading-recent-trace".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::Never,
                            title: "Never".to_string(),
                            description: Some("Keep the margin empty".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::HiddenWhenSidebar,
                            title: "Not with the panel".to_string(),
                            description: Some("Hidden while the panel is out".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::Always,
                            title: "Always".to_string(),
                            description: Some("Drawn whenever it fits".to_string()),
                        },
                    ],
                    selected: sidebar_cfg.recent_trace,
                    on_change: move |new_trace| {
                        config.write().sidebar.recent_trace = new_trace;
                    },
                    shipped: Some(defaults.sidebar.recent_trace),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Documents in the Trace" }
                    p { class: "preference-description", "How many documents the margin trace names." }
                }
                SliderInput {
                    value: sidebar_cfg.recent_trace_count as f64,
                    min: 1.0,
                    max: 12.0,
                    step: 1.0,
                    unit: String::new(),
                    on_change: move |new_count: f64| {
                        config.write().sidebar.recent_trace_count = new_count.max(1.0) as usize;
                    },
                    shipped: Some(defaults.sidebar.recent_trace_count as f64),
                }
            }
        }
    }
}
