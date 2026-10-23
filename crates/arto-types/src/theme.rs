use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// The user's theme preference.
///
/// `Auto` follows the operating system; resolving it to light or dark is the
/// app's job, since it needs the system appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

/// A string that names none of [`Theme`]'s variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown theme {0:?}; expected \"auto\", \"light\" or \"dark\"")]
pub struct UnknownTheme(pub String);

impl FromStr for Theme {
    type Err = UnknownTheme;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Theme::Auto),
            "light" => Ok(Theme::Light),
            "dark" => Ok(Theme::Dark),
            _ => Err(UnknownTheme(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Theme::Auto).unwrap(), r#""auto""#);
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), r#""dark""#);
    }

    #[test]
    fn parses_the_names_it_serializes_to() {
        for theme in [Theme::Auto, Theme::Light, Theme::Dark] {
            let name = serde_json::to_value(theme).unwrap();
            assert_eq!(name.as_str().unwrap().parse::<Theme>(), Ok(theme));
        }
    }

    #[test]
    fn rejects_a_name_it_does_not_know() {
        assert_eq!(
            "system".parse::<Theme>(),
            Err(UnknownTheme("system".to_string()))
        );
        assert!("Dark".parse::<Theme>().is_err());
        assert!("".parse::<Theme>().is_err());
    }
}
