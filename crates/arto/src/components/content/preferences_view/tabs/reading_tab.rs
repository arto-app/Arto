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
use rust_i18n::t;

/// Line lengths offered by name. Standard is the measure most books settle
/// on; Wide is the length the page has always had.
fn measure_presets() -> [(String, f64); 3] {
    [
        (
            t!("preferences.reading.typography.presets.narrow").to_string(),
            40.0,
        ),
        (
            t!("preferences.reading.typography.presets.standard").to_string(),
            50.0,
        ),
        (
            t!("preferences.reading.typography.presets.wide").to_string(),
            60.0,
        ),
    ]
}

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
const LANGUAGES: [CjkFontLanguage; 5] = [
    CjkFontLanguage::Auto,
    CjkFontLanguage::Ja,
    CjkFontLanguage::ZhHans,
    CjkFontLanguage::ZhHant,
    CjkFontLanguage::Ko,
];

/// Each language is named in itself, so only Auto is translated.
fn cjk_language_label(language: CjkFontLanguage) -> String {
    match language {
        CjkFontLanguage::Auto => {
            t!("preferences.reading.typography.cjk_font_language.auto").to_string()
        }
        CjkFontLanguage::Ja => "日本語".to_string(),
        CjkFontLanguage::ZhHans => "简体中文".to_string(),
        CjkFontLanguage::ZhHant => "繁體中文".to_string(),
        CjkFontLanguage::Ko => "한국어".to_string(),
    }
}

/// A line length told as the characters it holds: an em is one full-width
/// character or about two half-width ones.
fn measure_hint(measure: f64) -> String {
    let full_width = normalize_measure(measure) as u32;
    t!(
        "preferences.reading.typography.measure.hint",
        full_width = full_width,
        half_width = full_width * 2
    )
    .to_string()
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

            h3 { class: "preference-section-title", {t!("preferences.reading.current.title").to_string()} }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.reading.current.zoom.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.current.zoom.description").to_string()} }
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

            h3 { class: "preference-section-title", {t!("preferences.reading.defaults.title").to_string()} }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.reading.defaults.zoom.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.defaults.zoom.description").to_string()} }
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
                    label { {t!("preferences.reading.defaults.min_content_width.label").to_string()} }
                    p {
                        class: "preference-description",
                        {t!("preferences.reading.defaults.min_content_width.description").to_string()}
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

            h3 { class: "preference-section-title", {t!("preferences.reading.typography.title").to_string()} }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.reading.typography.measure.label").to_string()} }
                    p {
                        class: "preference-description",
                        {t!("preferences.reading.typography.measure.description", hint = measure_hint(typography_cfg.measure)).to_string()}
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
                label: t!("preferences.reading.typography.presets.label").to_string(),
                description: None,
                options: measure_presets()
                    .into_iter()
                    .map(|(label, measure)| ChoiceItem { value: measure, label })
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
                    label { {t!("preferences.reading.typography.line_height.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.typography.line_height.description").to_string()} }
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
                    label { {t!("preferences.reading.typography.font_family.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.typography.font_family.description").to_string()} }
                }
                OptionCards {
                    name: "reading-font-family".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Sans,
                            title: t!("preferences.reading.typography.font_family.sans.title").to_string(),
                            description: Some(t!("preferences.reading.typography.font_family.sans.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Serif,
                            title: t!("preferences.reading.typography.font_family.serif.title").to_string(),
                            description: Some(t!("preferences.reading.typography.font_family.serif.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Mono,
                            title: t!("preferences.reading.typography.font_family.mono.title").to_string(),
                            description: Some(t!("preferences.reading.typography.font_family.mono.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: FontFamilyChoice::Custom,
                            title: t!("preferences.reading.typography.font_family.custom.title").to_string(),
                            description: Some(t!("preferences.reading.typography.font_family.custom.description").to_string()),
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
                                {t!("preferences.reading.typography.font_family.invalid").to_string()}
                            }
                        }
                    }
                }
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.reading.typography.font_size.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.typography.font_size.description").to_string()} }
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
                label: t!("preferences.reading.typography.cjk_font_language.label").to_string(),
                description: Some(t!("preferences.reading.typography.cjk_font_language.description").to_string()),
                options: LANGUAGES
                    .into_iter()
                    .map(|language| ChoiceItem {
                        value: language,
                        label: cjk_language_label(language),
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

            h3 { class: "preference-section-title", {t!("preferences.reading.trace.title").to_string()} }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.reading.trace.when.label").to_string()} }
                    p {
                        class: "preference-description",
                        {t!("preferences.reading.trace.when.description").to_string()}
                    }
                }
                OptionCards {
                    name: "reading-recent-trace".to_string(),
                    options: vec![
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::Never,
                            title: t!("preferences.reading.trace.when.never.title").to_string(),
                            description: Some(t!("preferences.reading.trace.when.never.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::HiddenWhenSidebar,
                            title: t!("preferences.reading.trace.when.not_with_panel.title").to_string(),
                            description: Some(t!("preferences.reading.trace.when.not_with_panel.description").to_string()),
                        },
                        OptionCardItem {
                            icon: None,
                            value: RecentTrace::Always,
                            title: t!("preferences.reading.trace.when.always.title").to_string(),
                            description: Some(t!("preferences.reading.trace.when.always.description").to_string()),
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
                    label { {t!("preferences.reading.trace.count.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.trace.count.description").to_string()} }
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

            h3 { class: "preference-section-title", {t!("preferences.reading.reading_time.title").to_string()} }

            ToggleRow {
                label: t!("preferences.reading.reading_time.show.label").to_string(),
                description: Some(t!("preferences.reading.reading_time.show.description").to_string()),
                checked: reading_cfg.show_time,
                on_change: move |on| config.write().reading.show_time = on,
                shipped: Some(defaults.reading.show_time),
            }

            div {
                class: "preference-item",
                div {
                    class: "preference-item-header",
                    label { {t!("preferences.reading.reading_time.words_per_minute.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.reading_time.words_per_minute.description").to_string()} }
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
                    label { {t!("preferences.reading.reading_time.characters_per_minute.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.reading_time.characters_per_minute.description").to_string()} }
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
                    label { {t!("preferences.reading.reading_time.min_minutes.label").to_string()} }
                    p { class: "preference-description", {t!("preferences.reading.reading_time.min_minutes.description").to_string()} }
                }
                SliderInput {
                    value: reading_cfg.min_minutes as f64,
                    min: 0.0,
                    max: 30.0,
                    step: 1.0,
                    unit: t!("preferences.reading.reading_time.min_minutes.unit").to_string(),
                    on_change: move |minutes: f64| {
                        config.write().reading.min_minutes = minutes.max(0.0) as u32;
                    },
                    shipped: Some(defaults.reading.min_minutes as f64),
                }
            }

            h3 { class: "preference-section-title", {t!("preferences.reading.changes.title").to_string()} }

            ToggleRow {
                label: t!("preferences.reading.changes.show.label").to_string(),
                description: Some(t!("preferences.reading.changes.show.description").to_string()),
                checked: reading_cfg.show_changes,
                on_change: move |on| config.write().reading.show_changes = on,
                shipped: Some(defaults.reading.show_changes),
            }

            ToggleRow {
                label: t!("preferences.reading.changes.ignore_whitespace.label").to_string(),
                description: Some(t!("preferences.reading.changes.ignore_whitespace.description").to_string()),
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
