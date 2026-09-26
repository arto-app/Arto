use super::super::form_controls::{
    ChoiceItem, ChoiceRow, OptionCardItem, OptionCards, SliderInput, ToggleRow,
};
use crate::config::{
    normalize_content_zoom, normalize_font_size, normalize_line_height, normalize_measure,
    CjkFontLanguage, Config, FontFamilyChoice, RecentTrace, LINE_HEIGHT_STEP, MAX_CONTENT_ZOOM,
    MAX_FONT_SIZE, MAX_LINE_HEIGHT, MAX_MEASURE, MIN_CONTENT_WIDTH_RANGE, MIN_CONTENT_ZOOM,
    MIN_FONT_SIZE, MIN_LINE_HEIGHT, MIN_MEASURE, ZOOM_STEP,
};
use crate::events::SET_CONTENT_ZOOM_IN_WINDOW;
use dioxus::desktop::tao::window::WindowId;
use dioxus::prelude::*;

/// Line lengths offered by name. Standard is the measure most books settle
/// on; Wide is the length the page has always had.
const MEASURE_PRESETS: [(&str, f64); 3] = [("Narrow", 40.0), ("Standard", 50.0), ("Wide", 60.0)];

const SAMPLE_LATIN: &str = "The quick brown fox jumps over the lazy dog. A line that runs too long loses the eye on its way back to the start of the next; one that is too short breaks the sentence into pieces.";

// Drawn in the faces of the CJK font language chosen, as a document's are,
// so what it does to Han characters shows here.
// The last line gathers characters whose glyphs differ most between the
// languages.
const SAMPLES_CJK: [&str; 4] = [
    "吾輩は猫である。名前はまだ無い。行が長すぎると次の行頭を見失い、短すぎると文が細切れになる。",
    "行太长，视线就难以回到下一行的开头；行太短，句子又会被切得支离破碎。",
    "줄이 너무 길면 다음 줄의 시작을 놓치고, 너무 짧으면 문장이 조각난다.",
    "直 骨 角 今 令 户 雪 画 海 次",
];

/// CJK font languages offered by name, in the order they are offered.
const LANGUAGES: [(CjkFontLanguage, &str); 5] = [
    (CjkFontLanguage::Auto, "Auto"),
    (CjkFontLanguage::Ja, "日本語"),
    (CjkFontLanguage::ZhHans, "简体中文"),
    (CjkFontLanguage::ZhHant, "繁體中文"),
    (CjkFontLanguage::Ko, "한국어"),
];

/// A line length told as the characters it holds: an em is one full-width
/// character or about two half-width ones.
fn measure_hint(measure: f64) -> String {
    let full_width = normalize_measure(measure) as u32;
    format!(
        "About {full_width} full-width or {} half-width characters to a line.",
        full_width * 2
    )
}

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
    let typography_cfg = config.read().typography.clone();
    let reading_cfg = config.read().reading.clone();
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

            h3 { class: "preference-section-title", "Typography" }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Line Length" }
                    p {
                        class: "preference-description",
                        "The longest a line of text runs, in em. {measure_hint(typography_cfg.measure)} A window narrower than this sets the text at its own width, and full-width content ignores it."
                    }
                }
                SliderInput {
                    value: typography_cfg.measure,
                    min: MIN_MEASURE,
                    max: MAX_MEASURE,
                    step: 1.0,
                    unit: "em".to_string(),
                    on_change: move |measure| {
                        config.write().typography.measure = normalize_measure(measure);
                    },
                    shipped: Some(defaults.typography.measure),
                }
            }

            ChoiceRow {
                name: "reading-measure-preset".to_string(),
                label: "Line Length Presets".to_string(),
                description: None,
                options: MEASURE_PRESETS
                    .iter()
                    .map(|(label, measure)| ChoiceItem {
                        value: *measure,
                        label: label.to_string(),
                    })
                    .collect(),
                selected: typography_cfg.measure,
                on_change: move |measure| {
                    config.write().typography.measure = measure;
                },
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Line Height" }
                    p { class: "preference-description", "The height of a line as a multiple of the text size. Text set in long lines, or in Japanese, reads easier with more." }
                }
                SliderInput {
                    value: typography_cfg.line_height,
                    min: MIN_LINE_HEIGHT,
                    max: MAX_LINE_HEIGHT,
                    step: LINE_HEIGHT_STEP,
                    unit: String::new(),
                    decimals: 2,
                    on_change: move |line_height| {
                        config.write().typography.line_height = normalize_line_height(line_height);
                    },
                    shipped: Some(defaults.typography.line_height),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Typeface" }
                    p { class: "preference-description", "The face the text is set in. Code keeps its monospace face." }
                }
                OptionCards {
                    name: "reading-font-family".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Sans,
                            title: "Sans".to_string(),
                            description: Some("As GitHub sets it".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Serif,
                            title: "Serif".to_string(),
                            description: Some("A book face".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Mono,
                            title: "Mono".to_string(),
                            description: Some("Every character one width".to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Custom,
                            title: "Custom".to_string(),
                            description: Some("A font-family of your own".to_string()),
                        },
                    ],
                    selected: typography_cfg.font_family,
                    on_change: move |font_family| {
                        config.write().typography.font_family = font_family;
                    },
                    shipped: Some(defaults.typography.font_family),
                }
                if typography_cfg.font_family == FontFamilyChoice::Custom {
                    div {
                        class: "typography-custom-font",
                        input {
                            r#type: "text",
                            spellcheck: false,
                            placeholder: "\"Source Han Serif\", serif",
                            value: "{typography_cfg.custom_font_family}",
                            oninput: move |event| {
                                config.write().typography.custom_font_family = event.value();
                            },
                        }
                        if typography_cfg.font_stack().is_none() {
                            p {
                                class: "preference-description",
                                "Not used, so the text stays in the Sans face: give a comma-separated list, quoting any name with more than letters, digits, _ and -."
                            }
                        }
                    }
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Text Size" }
                    p { class: "preference-description", "The size of the text, which headings and code follow. Zoom enlarges the whole page, images included; this sets only the text." }
                }
                SliderInput {
                    value: typography_cfg.font_size,
                    min: MIN_FONT_SIZE,
                    max: MAX_FONT_SIZE,
                    step: 1.0,
                    unit: "px".to_string(),
                    on_change: move |font_size| {
                        config.write().typography.font_size = normalize_font_size(font_size);
                    },
                    shipped: Some(defaults.typography.font_size),
                }
            }

            ChoiceRow {
                name: "reading-cjk-font-language".to_string(),
                label: "CJK Font Language".to_string(),
                description: Some("Whose faces Chinese, Japanese and Korean characters are drawn in. One Han character takes different glyphs in each language's faces; Auto leaves the face to the system. Choosing one moves Latin text off the system face on macOS (SF becomes Helvetica), since that face would draw CJK text in the system's own choice. A custom typeface is used as written.".to_string()),
                options: LANGUAGES
                    .iter()
                    .map(|(language, label)| ChoiceItem {
                        value: *language,
                        label: label.to_string(),
                    })
                    .collect(),
                selected: typography_cfg.cjk_font_language,
                on_change: move |language| {
                    config.write().typography.cjk_font_language = language;
                },
                shipped: Some(defaults.typography.cjk_font_language),
            }

            div {
                class: "typography-sample",
                style: "{typography_cfg.css_declarations()}",
                div {
                    class: "markdown-body",
                    p { lang: "en", "{SAMPLE_LATIN}" }
                    for text in SAMPLES_CJK {
                        p { "{text}" }
                    }
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

            h3 { class: "preference-section-title", "Reading Time" }

            ToggleRow {
                label: "Show reading time".to_string(),
                description: Some("How long the document takes to read, and how long is left once you have started, beside the controls in the header. It counts the text, not the height of the page, so long code and tall diagrams do not run it ahead.".to_string()),
                checked: reading_cfg.show_time,
                on_change: move |on| config.write().reading.show_time = on,
                shipped: Some(defaults.reading.show_time),
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Words per Minute" }
                    p { class: "preference-description", "How fast you read scripts written in words, such as English." }
                }
                SliderInput {
                    value: reading_cfg.words_per_minute as f64,
                    min: 100.0,
                    max: 600.0,
                    step: 10.0,
                    unit: String::new(),
                    on_change: move |speed: f64| {
                        config.write().reading.words_per_minute = speed.max(1.0) as u32;
                    },
                    shipped: Some(defaults.reading.words_per_minute as f64),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Characters per Minute" }
                    p { class: "preference-description", "How fast you read Chinese, Japanese and Korean, which are read a character at a time." }
                }
                SliderInput {
                    value: reading_cfg.characters_per_minute as f64,
                    min: 200.0,
                    max: 1200.0,
                    step: 10.0,
                    unit: String::new(),
                    on_change: move |speed: f64| {
                        config.write().reading.characters_per_minute = speed.max(1.0) as u32;
                    },
                    shipped: Some(defaults.reading.characters_per_minute as f64),
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { "Shortest Document Timed" }
                    p { class: "preference-description", "A document that takes less than this to read shows no reading time." }
                }
                SliderInput {
                    value: reading_cfg.min_minutes as f64,
                    min: 0.0,
                    max: 30.0,
                    step: 1.0,
                    unit: " min".to_string(),
                    on_change: move |minutes: f64| {
                        config.write().reading.min_minutes = minutes.max(0.0) as u32;
                    },
                    shipped: Some(defaults.reading.min_minutes as f64),
                }
            }

            h3 { class: "preference-section-title", "Changes Since Last Read" }

            ToggleRow {
                label: "Mark what changed".to_string(),
                description: Some("A line beside each block added or rewritten since the document was last read, a hairline where text was taken out, and a dot on the headings they fall under. A document counts as read when you leave it. Turned off, Arto stops keeping a copy of each document you read.".to_string()),
                checked: reading_cfg.show_changes,
                on_change: move |on| config.write().reading.show_changes = on,
                shipped: Some(defaults.reading.show_changes),
            }

            ToggleRow {
                label: "Ignore spacing".to_string(),
                description: Some("A line whose words are only spaced differently is not marked as changed.".to_string()),
                checked: reading_cfg.ignore_whitespace_changes,
                on_change: move |on| config.write().reading.ignore_whitespace_changes = on,
                shipped: Some(defaults.reading.ignore_whitespace_changes),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_measure_is_told_in_characters_of_either_width() {
        assert_eq!(
            measure_hint(60.0),
            "About 60 full-width or 120 half-width characters to a line."
        );
    }

    #[test]
    fn the_measure_hint_names_the_length_the_page_will_use() {
        assert_eq!(
            measure_hint(500.0),
            "About 100 full-width or 200 half-width characters to a line."
        );
    }
}
