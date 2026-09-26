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

// A stack is Latin faces, then the CJK faces of the chosen CJK font language,
// then the generic family and the emoji faces, as Primer's stack ends. With no
// CJK font language the CJK faces are left out and the generic family draws
// CJK text in whatever the system picks. A CJK face is named only when asked
// for, because naming one imposes its glyphs on every Han character, Chinese
// text drawn in a Japanese face included.
//
// The system faces are left out whenever CJK faces are named. WebKit gives
// them the system's own CJK fallback, picked by the system's language, so
// they draw every CJK character before a named CJK face is reached. Without
// them the Latin text falls to the named faces after (Helvetica rather than
// SF on macOS, Menlo rather than SF Mono), which cannot be named themselves.
const SANS_SYSTEM: &str = "-apple-system, BlinkMacSystemFont";
const SANS_LATIN: &str = r#""Segoe UI", "Noto Sans", Helvetica, Arial"#;
const SERIF_LATIN: &str = r#""Iowan Old Style", "Palatino Linotype", Palatino, Georgia"#;
const MONO_SYSTEM: &str = "ui-monospace, SFMono-Regular";
const MONO_LATIN: &str = r#""SF Mono", Menlo, Consolas, "Liberation Mono""#;
const EMOJI: &str = r#""Apple Color Emoji", "Segoe UI Emoji""#;

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

impl CjkFontLanguage {
    /// The sans-serif CJK faces for this language, macOS first, then
    /// Windows, then the Noto faces most Linux systems carry.
    fn sans_faces(self) -> Option<&'static str> {
        match self {
            CjkFontLanguage::Auto => None,
            CjkFontLanguage::Ja => Some(
                r#""Hiragino Sans", "Hiragino Kaku Gothic ProN", "Yu Gothic", YuGothic, Meiryo, "Noto Sans CJK JP", "Noto Sans JP""#,
            ),
            CjkFontLanguage::ZhHans => {
                Some(r#""PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", "Noto Sans SC""#)
            }
            CjkFontLanguage::ZhHant => {
                Some(r#""PingFang TC", "Microsoft JhengHei", "Noto Sans CJK TC", "Noto Sans TC""#)
            }
            CjkFontLanguage::Ko => Some(
                r#""Apple SD Gothic Neo", "Malgun Gothic", "Noto Sans CJK KR", "Noto Sans KR""#,
            ),
        }
    }

    /// The serif CJK faces for this language, in the same platform order.
    fn serif_faces(self) -> Option<&'static str> {
        match self {
            CjkFontLanguage::Auto => None,
            CjkFontLanguage::Ja => Some(
                r#""Hiragino Mincho ProN", "Yu Mincho", YuMincho, "Noto Serif CJK JP", "Noto Serif JP""#,
            ),
            CjkFontLanguage::ZhHans => {
                Some(r#""Songti SC", SimSun, "Noto Serif CJK SC", "Noto Serif SC""#)
            }
            CjkFontLanguage::ZhHant => {
                Some(r#""Songti TC", PMingLiU, "Noto Serif CJK TC", "Noto Serif TC""#)
            }
            CjkFontLanguage::Ko => {
                Some(r#"AppleMyungjo, Batang, "Noto Serif CJK KR", "Noto Serif KR""#)
            }
        }
    }
}

/// The system faces and the Latin faces, or, when CJK faces are named, the
/// Latin faces and those in place of the system faces; then the rest.
fn stack(system: Option<&str>, latin: &str, cjk: Option<&str>, rest: &str) -> String {
    let families = match cjk {
        Some(cjk) => [None, Some(latin), Some(cjk), Some(rest)],
        None => [system, Some(latin), None, Some(rest)],
    };
    families
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
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

/// Whether `value` is a `font-family` list that stays a value.
///
/// The declarations are embedded in a `style` attribute in the app and in a
/// `<style>` element by `arto page`, so a value that could end the
/// declaration (`;`), the rule (`{`, `}`), the element (`<`, `>`) or escape a
/// character (`\`) is refused outright rather than repaired. So is a value
/// that is not a family list at all (an empty entry, a name starting with a
/// digit): substituted through `var()` it would invalidate the declaration,
/// and the text would fall back to the interface's face rather than the
/// stylesheet's.
fn is_safe_font_family(value: &str) -> bool {
    split_family_list(value).is_some_and(|families| families.into_iter().all(is_family_name))
}

/// The entries of a comma-separated list, commas inside quotes kept, or
/// `None` when a quote is left open.
fn split_family_list(value: &str) -> Option<Vec<&str>> {
    let mut families = Vec::new();
    let mut open_quote: Option<char> = None;
    let mut start = 0;
    for (index, c) in value.char_indices() {
        match (open_quote, c) {
            (Some(quote), c) if c == quote => open_quote = None,
            (None, '"' | '\'') => open_quote = Some(c),
            (None, ',') => {
                families.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    families.push(&value[start..]);
    open_quote.is_none().then_some(families)
}

/// One entry of the list: a quoted name, or unquoted identifiers.
fn is_family_name(family: &str) -> bool {
    let family = family.trim();
    match family.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let inner = &family[1..];
            inner.strip_suffix(quote).is_some_and(|name| {
                !name.is_empty()
                    && name.chars().all(|c| {
                        c != quote
                            && (c.is_alphanumeric()
                                || matches!(c, ' ' | '_' | ',' | '-' | '.' | '+' | '"' | '\''))
                    })
            })
        }
        Some(_) => family.split_whitespace().all(is_identifier),
        None => false,
    }
}

/// A CSS identifier as a family name may use one, short of escapes.
fn is_identifier(word: &str) -> bool {
    const CSS_WIDE_KEYWORDS: [&str; 6] = [
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
        "default",
    ];
    let body = word.strip_prefix('-').unwrap_or(word);
    let starts_well = body
        .chars()
        .next()
        .is_some_and(|c| c == '-' || c == '_' || c.is_alphabetic());
    starts_well
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        && !CSS_WIDE_KEYWORDS
            .iter()
            .any(|keyword| word.eq_ignore_ascii_case(keyword))
}

impl TypographyConfig {
    /// The `font-family` the text is set in, or `None` for the stylesheet's
    /// own stack — both when "sans" is chosen and when a custom value is
    /// refused.
    pub fn font_stack(&self) -> Option<String> {
        match self.font_family {
            FontFamilyChoice::Sans => {
                // With no CJK faces to add, the stylesheet's own stack is
                // the same as this one.
                let cjk = self.cjk_font_language.sans_faces()?;
                Some(stack(
                    Some(SANS_SYSTEM),
                    SANS_LATIN,
                    Some(cjk),
                    &format!("sans-serif, {EMOJI}"),
                ))
            }
            FontFamilyChoice::Serif => Some(stack(
                None,
                SERIF_LATIN,
                self.cjk_font_language.serif_faces(),
                &format!("serif, {EMOJI}"),
            )),
            // Few CJK faces are monospaced, so CJK text in code-like prose
            // takes the sans-serif faces.
            FontFamilyChoice::Mono => Some(stack(
                Some(MONO_SYSTEM),
                MONO_LATIN,
                self.cjk_font_language.sans_faces(),
                "monospace",
            )),
            FontFamilyChoice::Custom => {
                let custom = self.custom_font_family.trim();
                is_safe_font_family(custom).then(|| custom.to_string())
            }
        }
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

    fn with_custom(value: &str) -> TypographyConfig {
        TypographyConfig {
            font_family: FontFamilyChoice::Custom,
            custom_font_family: value.to_string(),
            ..TypographyConfig::default()
        }
    }

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

    fn with_cjk(
        font_family: FontFamilyChoice,
        cjk_font_language: CjkFontLanguage,
    ) -> TypographyConfig {
        TypographyConfig {
            font_family,
            cjk_font_language,
            custom_font_family: "serif".to_string(),
            ..TypographyConfig::default()
        }
    }

    const CJK_FACES: [&str; 8] = [
        "Hiragino", "Yu ", "CJK", "Songti", "PingFang", "Apple SD", "Myungjo", "Malgun",
    ];

    #[test]
    fn with_no_cjk_font_language_no_cjk_face_is_named() {
        for font_family in [FontFamilyChoice::Serif, FontFamilyChoice::Mono] {
            let stack = with_cjk(font_family, CjkFontLanguage::Auto)
                .font_stack()
                .unwrap();
            for face in CJK_FACES {
                assert!(!stack.contains(face), "{stack} names {face}");
            }
        }
        assert_eq!(
            with_cjk(FontFamilyChoice::Sans, CjkFontLanguage::Auto).font_stack(),
            None
        );
    }

    #[test]
    fn a_cjk_font_language_names_its_faces_before_the_generic_family() {
        let cases = [
            (
                FontFamilyChoice::Sans,
                CjkFontLanguage::Ja,
                "\"Hiragino Sans\"",
                "sans-serif",
            ),
            (
                FontFamilyChoice::Sans,
                CjkFontLanguage::ZhHans,
                "\"PingFang SC\"",
                "sans-serif",
            ),
            (
                FontFamilyChoice::Serif,
                CjkFontLanguage::Ja,
                "\"Hiragino Mincho ProN\"",
                "serif",
            ),
            (
                FontFamilyChoice::Serif,
                CjkFontLanguage::ZhHans,
                "\"Songti SC\"",
                "serif",
            ),
            (
                FontFamilyChoice::Serif,
                CjkFontLanguage::ZhHant,
                "\"Songti TC\"",
                "serif",
            ),
            (
                FontFamilyChoice::Serif,
                CjkFontLanguage::Ko,
                "AppleMyungjo",
                "serif",
            ),
            (
                FontFamilyChoice::Mono,
                CjkFontLanguage::Ko,
                "\"Apple SD Gothic Neo\"",
                "monospace",
            ),
        ];
        for (font_family, language, face, generic) in cases {
            let stack = with_cjk(font_family, language).font_stack().unwrap();
            let families: Vec<&str> = stack.split(", ").collect();
            let named = families.iter().position(|family| *family == face);
            let generic = families.iter().position(|family| *family == generic);
            assert!(named.is_some() && named < generic, "{stack}");
        }
    }

    #[test]
    fn a_cjk_font_language_leaves_out_the_faces_that_fill_in_cjk_themselves() {
        let system_faces = [
            "-apple-system",
            "BlinkMacSystemFont",
            "system-ui",
            "ui-monospace",
            "SFMono-Regular",
        ];
        for font_family in [
            FontFamilyChoice::Sans,
            FontFamilyChoice::Serif,
            FontFamilyChoice::Mono,
        ] {
            for language in [
                CjkFontLanguage::Ja,
                CjkFontLanguage::ZhHans,
                CjkFontLanguage::ZhHant,
                CjkFontLanguage::Ko,
            ] {
                let stack = with_cjk(font_family, language).font_stack().unwrap();
                let families: Vec<&str> = stack.split(", ").collect();
                for face in system_faces {
                    assert!(!families.contains(&face), "{stack} keeps {face}");
                }
            }
        }
    }

    #[test]
    fn a_cjk_font_language_keeps_the_latin_faces_first() {
        let stack = with_cjk(FontFamilyChoice::Sans, CjkFontLanguage::Ja)
            .font_stack()
            .unwrap();
        assert!(
            stack.starts_with(r#""Segoe UI", "Noto Sans", Helvetica, Arial, "Hiragino Sans""#),
            "{stack}"
        );
        let stack = with_cjk(FontFamilyChoice::Mono, CjkFontLanguage::Ja)
            .font_stack()
            .unwrap();
        assert!(
            stack.starts_with(r#""SF Mono", Menlo, Consolas, "Liberation Mono", "Hiragino Sans""#),
            "{stack}"
        );
    }

    #[test]
    fn with_no_cjk_font_language_the_system_faces_stay() {
        let stack = with_cjk(FontFamilyChoice::Mono, CjkFontLanguage::Auto)
            .font_stack()
            .unwrap();
        assert!(
            stack.starts_with("ui-monospace, SFMono-Regular, "),
            "{stack}"
        );
    }

    #[test]
    fn a_cjk_font_language_names_no_other_languages_faces() {
        let stack = with_cjk(FontFamilyChoice::Serif, CjkFontLanguage::ZhHans)
            .font_stack()
            .unwrap();
        assert!(!stack.contains("Hiragino"), "{stack}");
        let stack = with_cjk(FontFamilyChoice::Serif, CjkFontLanguage::Ja)
            .font_stack()
            .unwrap();
        assert!(!stack.contains("Songti"), "{stack}");
    }

    #[test]
    fn a_custom_face_is_left_as_written_whatever_the_cjk_font_language() {
        assert_eq!(
            with_cjk(FontFamilyChoice::Custom, CjkFontLanguage::Ja)
                .font_stack()
                .as_deref(),
            Some("serif")
        );
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

    #[test]
    fn every_preset_stack_is_itself_safe() {
        for font_family in [
            FontFamilyChoice::Sans,
            FontFamilyChoice::Serif,
            FontFamilyChoice::Mono,
        ] {
            for language in [
                CjkFontLanguage::Auto,
                CjkFontLanguage::Ja,
                CjkFontLanguage::ZhHans,
                CjkFontLanguage::ZhHant,
                CjkFontLanguage::Ko,
            ] {
                if let Some(stack) = with_cjk(font_family, language).font_stack() {
                    assert!(is_safe_font_family(&stack), "{stack}");
                }
            }
        }
    }

    #[test]
    fn a_custom_face_is_accepted_when_it_stays_a_value() {
        for value in [
            r#""Source Han Serif", serif"#,
            "'M+ 1p', sans-serif",
            "Noto_Sans-JP",
            r#""ヒラギノ明朝 ProN W3", serif"#,
            r#""It's", serif"#,
        ] {
            assert_eq!(with_custom(value).font_stack().as_deref(), Some(value));
        }
    }

    #[test]
    fn a_custom_face_is_trimmed() {
        assert_eq!(
            with_custom("  Georgia  ").font_stack().as_deref(),
            Some("Georgia")
        );
    }

    #[test]
    fn a_custom_face_that_could_leave_its_declaration_is_refused() {
        for value in [
            "",
            "   ",
            "serif; color: red",
            "serif } body { display: none",
            "serif</style><script>alert(1)</script>",
            r"\73 erif",
            r#""Unclosed, serif"#,
            "serif !important",
            "url(x)",
        ] {
            let config = with_custom(value);
            assert_eq!(config.font_stack(), None, "{value:?}");
            assert!(
                config
                    .css_declarations()
                    .ends_with(" --reading-font-family: initial;"),
                "{value:?}"
            );
        }
    }

    #[test]
    fn a_custom_face_that_is_not_a_font_family_list_is_refused() {
        // Each of these would invalidate the whole declaration, which then
        // falls back to the interface's face rather than the stylesheet's.
        for value in [
            ",",
            "Georgia,",
            "serif,,sans-serif",
            "123",
            "-1a",
            "M+ 1p",
            "Georgia.Pro",
            r#""Source Han" Serif"#,
            "inherit",
            "Georgia, initial",
        ] {
            assert_eq!(with_custom(value).font_stack(), None, "{value:?}");
        }
    }

    #[test]
    fn a_custom_value_is_ignored_unless_custom_is_chosen() {
        let config = TypographyConfig {
            custom_font_family: "Georgia".to_string(),
            ..TypographyConfig::default()
        };
        assert_eq!(config.font_stack(), None);
    }
}
