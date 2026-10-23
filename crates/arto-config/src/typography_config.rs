use crate::font_stack::font_stack;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Line length range, in em. An em is one full-width character, so a CJK
/// line holds about as many characters as the number and a Latin line
/// about twice as many.
pub const MIN_MEASURE: f64 = 30.0;
pub const MAX_MEASURE: f64 = 100.0;
/// 60em at the 16px base size is the 960px the page has always had.
pub const DEFAULT_MEASURE: f64 = 60.0;

/// Line height range, unitless. Snapped to [`LINE_HEIGHT_STEP`].
pub const MIN_LINE_HEIGHT: f64 = 1.2;
pub const MAX_LINE_HEIGHT: f64 = 2.2;
pub const DEFAULT_LINE_HEIGHT: f64 = 1.5;
pub const LINE_HEIGHT_STEP: f64 = 0.05;

/// Base text size range, in px.
pub const MIN_FONT_SIZE: f64 = 12.0;
pub const MAX_FONT_SIZE: f64 = 24.0;
pub const DEFAULT_FONT_SIZE: f64 = 16.0;

/// The face the document text is set in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FontFamilyChoice {
    /// The sans-serif stack GitHub renders Markdown in.
    #[default]
    Sans,
    /// A serif stack.
    Serif,
    /// A monospace stack.
    Mono,
    /// The CSS `font-family` given in `customFontFamily`.
    Custom,
}

/// The language whose faces Chinese, Japanese and Korean characters are drawn
/// in. One Han character takes different glyphs in Japanese, Chinese and
/// Korean faces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CjkFontLanguage {
    /// Leave the face to the system.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// Japanese.
    #[serde(rename = "ja")]
    Ja,
    /// Simplified Chinese.
    #[serde(rename = "zh-Hans")]
    ZhHans,
    /// Traditional Chinese.
    #[serde(rename = "zh-Hant")]
    ZhHant,
    /// Korean.
    #[serde(rename = "ko")]
    Ko,
}

/// How the text of a document is set: how long its lines run, how far
/// apart they are, and in what face and size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct TypographyConfig {
    /// The longest a line of text runs, in em (30–100). One em is about one
    /// full-width character or two half-width ones. A window narrower than
    /// this sets the text at its own width; full-width content ignores it.
    #[schemars(range(min = MIN_MEASURE, max = MAX_MEASURE))]
    pub measure: f64,
    /// The height of a line as a multiple of the text size (1.2–2.2).
    #[schemars(range(min = MIN_LINE_HEIGHT, max = MAX_LINE_HEIGHT))]
    pub line_height: f64,
    /// The face the text is set in. Code keeps its monospace face.
    pub font_family: FontFamilyChoice,
    /// A CSS `font-family` list used when `fontFamily` is "custom", such as
    /// `"Source Han Serif", serif`. Only letters, digits, spaces and
    /// `_ , " ' - . +` may appear, and a name holding anything but letters,
    /// digits, `_` and `-` has to be quoted; anything else falls back to
    /// "sans".
    pub custom_font_family: String,
    /// The size of the text in px (12–24). Headings and code scale with it.
    /// Unlike zoom, it leaves images and diagrams as they are.
    #[schemars(range(min = MIN_FONT_SIZE, max = MAX_FONT_SIZE))]
    pub font_size: f64,
    /// The language whose faces CJK characters are drawn in ("auto", "ja",
    /// "zh-Hans", "zh-Hant" or "ko"): one Han character takes different
    /// glyphs in each. "auto" leaves the face to the system. It picks faces
    /// only; the language documents are read as is left alone. A "custom"
    /// face is used as written.
    pub cjk_font_language: CjkFontLanguage,
}

impl Default for TypographyConfig {
    fn default() -> Self {
        Self {
            measure: DEFAULT_MEASURE,
            line_height: DEFAULT_LINE_HEIGHT,
            font_family: FontFamilyChoice::default(),
            custom_font_family: String::new(),
            font_size: DEFAULT_FONT_SIZE,
            cjk_font_language: CjkFontLanguage::default(),
        }
    }
}

/// Clamp into `[min, max]`, with non-finite input taking `default`.
fn clamp_or(value: f64, min: f64, max: f64, default: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

/// A line length in whole em within the accepted range.
pub fn normalize_measure(measure: f64) -> f64 {
    clamp_or(measure.round(), MIN_MEASURE, MAX_MEASURE, DEFAULT_MEASURE)
}

/// A line height on the [`LINE_HEIGHT_STEP`] grid within the accepted range.
pub fn normalize_line_height(line_height: f64) -> f64 {
    let steps = 1.0 / LINE_HEIGHT_STEP;
    clamp_or(
        (line_height * steps).round() / steps,
        MIN_LINE_HEIGHT,
        MAX_LINE_HEIGHT,
        DEFAULT_LINE_HEIGHT,
    )
}

/// A base text size in whole px within the accepted range.
pub fn normalize_font_size(font_size: f64) -> f64 {
    clamp_or(
        font_size.round(),
        MIN_FONT_SIZE,
        MAX_FONT_SIZE,
        DEFAULT_FONT_SIZE,
    )
}

impl TypographyConfig {
    /// The `font-family` the text is set in, or `None` for the stylesheet's
    /// own stack — both when "sans" is chosen and when a custom value is
    /// refused.
    pub fn font_stack(&self) -> Option<String> {
        font_stack(
            self.font_family,
            self.cjk_font_language,
            &self.custom_font_family,
        )
    }

    /// The CSS custom properties the Markdown stylesheet reads, as
    /// declarations for the element that holds the document.
    ///
    /// Values out of range are clamped here, so a hand-edited `config.json`
    /// never reaches the page as something the stylesheet would reject.
    ///
    /// Every property is declared whatever is chosen. The app updates the
    /// element's style property by property, so a declaration that is left
    /// out keeps the value it had; the stylesheet's own stack is reached with
    /// `initial`, which makes the variable fall back instead.
    pub fn css_declarations(&self) -> String {
        format!(
            "--reading-measure: {}em; --reading-line-height: {}; --reading-font-size: {}px; --reading-font-family: {};",
            normalize_measure(self.measure),
            normalize_line_height(self.line_height),
            normalize_font_size(self.font_size),
            self.font_stack().as_deref().unwrap_or("initial"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn default_matches_the_page_as_it_has_always_been() {
        let config = TypographyConfig::default();
        assert_eq!(config.measure, 60.0);
        assert_eq!(config.line_height, 1.5);
        assert_eq!(config.font_family, FontFamilyChoice::Sans);
        assert_eq!(
            serde_json::to_string(&config.font_family).unwrap(),
            r#""sans""#
        );
        assert_eq!(config.custom_font_family, "");
        assert_eq!(config.font_size, 16.0);
        assert_eq!(config.cjk_font_language, CjkFontLanguage::Auto);
    }

    #[test]
    fn missing_keys_take_their_defaults() {
        let parsed: TypographyConfig = serde_json::from_str(r#"{"lineHeight": 1.8}"#).unwrap();
        assert_eq!(parsed.line_height, 1.8);
        assert_eq!(parsed.measure, DEFAULT_MEASURE);
        assert_eq!(parsed.font_size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn serializes_in_camel_case_and_snake_case_values() {
        let config = TypographyConfig {
            measure: 45.0,
            line_height: 1.75,
            font_family: FontFamilyChoice::Serif,
            custom_font_family: "Foo".to_string(),
            font_size: 18.0,
            cjk_font_language: CjkFontLanguage::ZhHans,
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert_eq!(
            json,
            indoc! {r#"
                {
                  "measure": 45.0,
                  "lineHeight": 1.75,
                  "fontFamily": "serif",
                  "customFontFamily": "Foo",
                  "fontSize": 18.0,
                  "cjkFontLanguage": "zh-Hans"
                }"#}
        );
        let parsed: TypographyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        assert_eq!(normalize_measure(10.0), MIN_MEASURE);
        assert_eq!(normalize_measure(500.0), MAX_MEASURE);
        assert_eq!(normalize_measure(44.6), 45.0);
        assert_eq!(normalize_line_height(0.5), MIN_LINE_HEIGHT);
        assert_eq!(normalize_line_height(3.0), MAX_LINE_HEIGHT);
        assert_eq!(normalize_font_size(4.0), MIN_FONT_SIZE);
        assert_eq!(normalize_font_size(40.0), MAX_FONT_SIZE);
    }

    #[test]
    fn line_height_snaps_to_its_step() {
        assert_eq!(normalize_line_height(1.62), 1.6);
        assert_eq!(normalize_line_height(1.63), 1.65);
        assert_eq!(normalize_line_height(1.2), 1.2);
    }

    #[test]
    fn non_finite_values_take_the_default() {
        assert_eq!(normalize_measure(f64::NAN), DEFAULT_MEASURE);
        assert_eq!(normalize_line_height(f64::INFINITY), DEFAULT_LINE_HEIGHT);
        assert_eq!(normalize_font_size(f64::NEG_INFINITY), DEFAULT_FONT_SIZE);
    }

    #[test]
    fn default_declarations_leave_the_face_to_the_stylesheet() {
        assert_eq!(
            TypographyConfig::default().css_declarations(),
            "--reading-measure: 60em; --reading-line-height: 1.5; --reading-font-size: 16px; --reading-font-family: initial;"
        );
    }

    #[test]
    fn every_face_declares_the_same_properties() {
        let names = |config: TypographyConfig| -> Vec<String> {
            config
                .css_declarations()
                .split("; ")
                .map(|declaration| declaration.split(':').next().unwrap().to_string())
                .collect()
        };
        let sans = names(TypographyConfig::default());
        for font_family in [
            FontFamilyChoice::Serif,
            FontFamilyChoice::Mono,
            FontFamilyChoice::Custom,
        ] {
            let config = TypographyConfig {
                font_family,
                custom_font_family: "serif".to_string(),
                ..TypographyConfig::default()
            };
            assert_eq!(names(config), sans, "{font_family:?}");
        }
    }

    #[test]
    fn declarations_are_clamped() {
        let config = TypographyConfig {
            measure: 1000.0,
            line_height: f64::NAN,
            font_size: 2.0,
            ..TypographyConfig::default()
        };
        assert_eq!(
            config.css_declarations(),
            "--reading-measure: 100em; --reading-line-height: 1.5; --reading-font-size: 12px; --reading-font-family: initial;"
        );
    }

    #[test]
    fn a_preset_face_is_declared() {
        let config = TypographyConfig {
            font_family: FontFamilyChoice::Serif,
            ..TypographyConfig::default()
        };
        let serif = r#""Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif, "Apple Color Emoji", "Segoe UI Emoji""#;
        assert!(config
            .css_declarations()
            .ends_with(&format!(" --reading-font-family: {serif};")));
        assert_eq!(config.font_stack().as_deref(), Some(serif));
    }

    #[test]
    fn a_cjk_font_language_is_written_as_its_lang_tag() {
        for (language, json) in [
            (CjkFontLanguage::Auto, r#""auto""#),
            (CjkFontLanguage::Ja, r#""ja""#),
            (CjkFontLanguage::ZhHans, r#""zh-Hans""#),
            (CjkFontLanguage::ZhHant, r#""zh-Hant""#),
            (CjkFontLanguage::Ko, r#""ko""#),
        ] {
            assert_eq!(serde_json::to_string(&language).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<CjkFontLanguage>(json).unwrap(),
                language
            );
        }
    }
}
