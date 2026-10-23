use crate::{CjkFontLanguage, FontFamilyChoice};

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

/// The `font-family` the text is set in, or `None` for the stylesheet's
/// own stack — both when "sans" is chosen and when a custom value is
/// refused.
pub(crate) fn font_stack(
    font_family: FontFamilyChoice,
    cjk_font_language: CjkFontLanguage,
    custom: &str,
) -> Option<String> {
    match font_family {
        FontFamilyChoice::Sans => {
            // With no CJK faces to add, the stylesheet's own stack is
            // the same as this one.
            let cjk = cjk_font_language.sans_faces()?;
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
            cjk_font_language.serif_faces(),
            &format!("serif, {EMOJI}"),
        )),
        // Few CJK faces are monospaced, so CJK text in code-like prose
        // takes the sans-serif faces.
        FontFamilyChoice::Mono => Some(stack(
            Some(MONO_SYSTEM),
            MONO_LATIN,
            cjk_font_language.sans_faces(),
            "monospace",
        )),
        FontFamilyChoice::Custom => {
            let custom = custom.trim();
            is_safe_font_family(custom).then(|| custom.to_string())
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TypographyConfig;

    fn with_custom(value: &str) -> TypographyConfig {
        TypographyConfig {
            font_family: FontFamilyChoice::Custom,
            custom_font_family: value.to_string(),
            ..TypographyConfig::default()
        }
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
