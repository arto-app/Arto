//! The language of Arto's own interface.
//!
//! Strings live in `locales/<area>.<code>.yml`, compiled in by `rust_i18n::i18n!`
//! at the crate root and looked up with `t!`. English is the
//! fallback, so a key missing from another locale still shows words.
//!
//! The locale is chosen once at launch. Native menus are built once and
//! most components only render again when their signals change, so
//! switching mid-session would leave the interface in two languages; the
//! preference therefore takes effect on the next launch, which the
//! preferences offer to bring about with a restart.

use crate::config::{Language, CONFIG};

const FALLBACK: &str = "en";

/// Every choice the language preference offers, in the order it lists them.
pub(crate) const CHOICES: &[Language] = &[Language::Auto, Language::En, Language::Ja];

/// Set the interface locale from the configuration and the system language.
pub(crate) fn init() {
    let language = CONFIG.read().language;
    let locale = locale_at_launch(language);
    tracing::debug!(?language, locale, "interface locale chosen");
    rust_i18n::set_locale(locale);
}

/// The locale a launch with `language` configured would show.
pub(crate) fn locale_at_launch(language: Language) -> &'static str {
    locale_for(language, sys_locale::get_locale().as_deref())
}

/// A language named in itself, so a reader who cannot read the current
/// interface can still find their own. `Auto` has no name of its own.
pub(crate) fn native_name(language: Language) -> Option<&'static str> {
    match language {
        Language::Auto => None,
        Language::En => Some("English"),
        Language::Ja => Some("日本語"),
    }
}

/// The locale chosen at launch.
pub(crate) fn locale() -> &'static str {
    match &*rust_i18n::locale() {
        "ja" => "ja",
        _ => FALLBACK,
    }
}

/// `key.one` or `key.other` by `count`, with `%{count}` filled in.
///
/// English is the only locale whose words change with the count; Japanese
/// writes both forms the same.
pub(crate) fn plural(key: &str, count: usize) -> String {
    rust_i18n::t!(plural_key(key, count), count = count).into_owned()
}

fn plural_key(key: &str, count: usize) -> String {
    let form = if count == 1 { "one" } else { "other" };
    format!("{key}.{form}")
}

/// The `<script>` that hands the frontend its words for the current locale,
/// read by `frontend/src/i18n.ts`.
pub(crate) fn frontend_catalog() -> String {
    let messages = match locale() {
        "ja" => include_str!("../locales/frontend.ja.json"),
        _ => include_str!("../locales/frontend.en.json"),
    };
    // `</` would end the script element early; JSON reads `<\/` as `</`.
    let catalog =
        format!(r#"{{"locale":"{}","messages":{messages}}}"#, locale()).replace("</", r"<\/");
    format!(r#"<script type="application/json" id="arto-i18n">{catalog}</script>"#)
}

fn locale_for(language: Language, system: Option<&str>) -> &'static str {
    match language {
        Language::En => "en",
        Language::Ja => "ja",
        Language::Auto => system
            .and_then(language_of)
            .map_or(FALLBACK, |language| locale_for(language, None)),
    }
}

/// The language Arto has for a system locale such as `ja-JP` or
/// `ja_JP.UTF-8`, matched on its language subtag alone.
fn language_of(system: &str) -> Option<Language> {
    let primary = system.split(['-', '_', '.']).next()?;
    match primary.to_ascii_lowercase().as_str() {
        "en" => Some(Language::En),
        "ja" => Some(Language::Ja),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_choice_but_auto_is_named_in_itself() {
        for &language in CHOICES {
            assert_eq!(
                native_name(language).is_none(),
                language == Language::Auto,
                "{language:?}"
            );
        }
    }

    #[test]
    fn every_language_is_offered() {
        // A variant added to `Language` without a place in `CHOICES` could
        // be set in config.json but never chosen in the preferences. The
        // match stops compiling when a variant is added, which sends whoever
        // added it here.
        for language in [Language::Auto, Language::En, Language::Ja] {
            match language {
                Language::Auto | Language::En | Language::Ja => {}
            }
            assert!(CHOICES.contains(&language), "{language:?}");
        }
    }

    #[test]
    fn an_explicit_language_wins_over_the_system() {
        assert_eq!(locale_for(Language::En, Some("ja-JP")), "en");
        assert_eq!(locale_for(Language::Ja, Some("en-US")), "ja");
        assert_eq!(locale_for(Language::Ja, None), "ja");
    }

    #[test]
    fn auto_follows_the_language_subtag_of_the_system() {
        assert_eq!(locale_for(Language::Auto, Some("ja-JP")), "ja");
        assert_eq!(locale_for(Language::Auto, Some("ja_JP.UTF-8")), "ja");
        assert_eq!(locale_for(Language::Auto, Some("JA")), "ja");
        assert_eq!(locale_for(Language::Auto, Some("en-GB")), "en");
    }

    #[test]
    fn auto_falls_back_to_english_for_an_unknown_or_missing_system_language() {
        assert_eq!(locale_for(Language::Auto, Some("fr-FR")), "en");
        assert_eq!(locale_for(Language::Auto, Some("")), "en");
        assert_eq!(locale_for(Language::Auto, None), "en");
    }

    #[test]
    fn plural_picks_one_for_a_single_item_and_other_for_the_rest() {
        assert_eq!(plural_key("sidebar.files", 1), "sidebar.files.one");
        assert_eq!(plural_key("sidebar.files", 0), "sidebar.files.other");
        assert_eq!(plural_key("sidebar.files", 2), "sidebar.files.other");
    }

    #[test]
    fn the_frontend_catalog_is_json_for_the_current_locale() {
        let script = frontend_catalog();
        let json = script
            .strip_prefix(r#"<script type="application/json" id="arto-i18n">"#)
            .and_then(|rest| rest.strip_suffix("</script>"))
            .unwrap();
        let catalog: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(catalog["locale"], locale());
        assert!(catalog["messages"]["frontend"].is_object());
    }

    #[test]
    fn every_explicit_language_has_a_locale_file() {
        let available: BTreeSet<_> = rust_i18n::available_locales!().into_iter().collect();
        for language in [Language::En, Language::Ja] {
            let locale = locale_for(language, None);
            assert!(available.contains(locale), "no locales/{locale}.yml");
        }
    }

    fn load_locales(
    ) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/locales");
        rust_i18n_support::try_load_locales(path, |_| false, true).unwrap()
    }

    /// Every key `source` gives to `t!` or `plural` as a string literal; a
    /// plural key stands for both its forms.
    fn keys_in(source: &str) -> Vec<String> {
        let openings = [("t!(", &[""][..]), ("plural(", &[".one", ".other"])];
        let mut keys = Vec::new();
        for (opening, forms) in openings {
            for (index, _) in source.match_indices(opening) {
                // Preceded by an identifier character, it is another macro
                // or function, such as `format!(` or `assert!(`.
                let preceding = source[..index].chars().next_back();
                if preceding.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                // rustfmt moves a long call's key onto the next line, so the
                // literal may start after whitespace. Anything else — a key
                // built at run time, a definition — is not a literal.
                let Some(rest) = source[index + opening.len()..]
                    .trim_start()
                    .strip_prefix('"')
                else {
                    continue;
                };
                let key = &rest[..rest.find('"').unwrap()];
                keys.extend(forms.iter().map(|form| format!("{key}{form}")));
            }
        }
        keys
    }

    /// [`keys_in`] for every source file under `dir`, with the file each key
    /// was found in.
    fn literal_keys(dir: &std::path::Path, found: &mut Vec<(String, std::path::PathBuf)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                literal_keys(&path, found);
                continue;
            }
            // This file asks for no key of its own, and its tests write calls
            // that are not meant to be found.
            if path.extension().is_none_or(|ext| ext != "rs") || path.ends_with("src/i18n.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            found.extend(keys_in(&source).into_iter().map(|key| (key, path.clone())));
        }
    }

    #[test]
    fn the_scan_finds_keys_wherever_rustfmt_puts_them() {
        let source = [
            r#"let a = t!("same.line");"#,
            "let b = t!(\n    \"next.line\",\n    name = value,\n);",
            r#"let c = plural("counted", n);"#,
            r#"let d = format!("not.a.key"); let e = t!(key_at_run_time);"#,
        ]
        .join("\n");
        assert_eq!(
            keys_in(&source),
            ["same.line", "next.line", "counted.one", "counted.other"]
        );
    }

    #[test]
    fn every_key_the_source_asks_for_is_in_english() {
        let locales = load_locales();
        let english = &locales[FALLBACK];
        let mut used = Vec::new();
        literal_keys(
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
            &mut used,
        );
        let unknown: Vec<_> = used
            .iter()
            .filter(|(key, _)| !english.contains_key(key))
            .collect();
        assert!(
            unknown.is_empty(),
            "keys missing from English: {unknown:#?}"
        );
    }

    #[test]
    fn every_locale_has_the_same_keys_as_english() {
        let locales = load_locales();
        let english: BTreeSet<_> = locales[FALLBACK].keys().collect();
        for (locale, translations) in &locales {
            let keys: BTreeSet<_> = translations.keys().collect();
            let missing: Vec<_> = english.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&english).collect();
            assert!(missing.is_empty(), "{locale} lacks {missing:?}");
            assert!(
                extra.is_empty(),
                "{locale} has keys English lacks: {extra:?}"
            );
        }
    }
}
