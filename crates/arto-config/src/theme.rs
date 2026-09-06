use serde::{Deserialize, Serialize};

/// The user's theme preference.
///
/// `Auto` follows the operating system; resolving it to light or dark is the
/// app's job, since it needs the system appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl From<&str> for Theme {
    fn from(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::Auto,
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
    fn unknown_strings_fall_back_to_auto() {
        assert_eq!(Theme::from("light"), Theme::Light);
        assert_eq!(Theme::from("dark"), Theme::Dark);
        assert_eq!(Theme::from("anything else"), Theme::Auto);
    }
}
