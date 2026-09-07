use serde::{Deserialize, Serialize};

/// One of GitHub's colour themes.
///
/// The names are GitHub's own, and each has a matching block of Primer design
/// tokens in the frontend stylesheet keyed by exactly this string, so a
/// variant renamed here renders as no theme at all.
///
/// GitHub also ships a high-contrast pairing for the dimmed and colour-vision
/// themes, applied when the operating system asks for increased contrast.
/// Those are not offered as choices, here or on GitHub.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorTheme {
    #[default]
    Light,
    LightHighContrast,
    LightColorblind,
    LightTritanopia,
    Dark,
    DarkDimmed,
    DarkHighContrast,
    DarkColorblind,
    DarkTritanopia,
}

impl ColorTheme {
    /// Every theme a user can choose, in the order GitHub lists them.
    pub const ALL: [Self; 9] = [
        Self::Light,
        Self::LightHighContrast,
        Self::LightColorblind,
        Self::LightTritanopia,
        Self::Dark,
        Self::DarkDimmed,
        Self::DarkHighContrast,
        Self::DarkColorblind,
        Self::DarkTritanopia,
    ];

    /// The name the frontend knows this theme by, in `data-theme`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::LightHighContrast => "light_high_contrast",
            Self::LightColorblind => "light_colorblind",
            Self::LightTritanopia => "light_tritanopia",
            Self::Dark => "dark",
            Self::DarkDimmed => "dark_dimmed",
            Self::DarkHighContrast => "dark_high_contrast",
            Self::DarkColorblind => "dark_colorblind",
            Self::DarkTritanopia => "dark_tritanopia",
        }
    }

    /// How the theme presents itself in the interface, in GitHub's wording.
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light default",
            Self::LightHighContrast => "Light high contrast",
            Self::LightColorblind => "Light Protanopia & Deuteranopia",
            Self::LightTritanopia => "Light Tritanopia",
            Self::Dark => "Dark default",
            Self::DarkDimmed => "Dark dimmed",
            Self::DarkHighContrast => "Dark high contrast",
            Self::DarkColorblind => "Dark Protanopia & Deuteranopia",
            Self::DarkTritanopia => "Dark Tritanopia",
        }
    }

    /// Whether the theme paints a dark canvas.
    ///
    /// A light theme is a legitimate choice for dark mode — GitHub offers the
    /// whole list in both slots — so this asks the theme, never the mode it
    /// was chosen for. The stylesheet decides the same way, off the name.
    pub fn is_dark(self) -> bool {
        self.as_str().starts_with("dark")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_the_frontend_name() {
        assert_eq!(
            serde_json::to_string(&ColorTheme::DarkDimmed).unwrap(),
            r#""dark_dimmed""#
        );
        assert_eq!(
            serde_json::to_string(&ColorTheme::LightHighContrast).unwrap(),
            r#""light_high_contrast""#
        );
    }

    #[test]
    fn every_theme_is_offered_exactly_once() {
        let mut names: Vec<_> = ColorTheme::ALL.iter().map(|t| t.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ColorTheme::ALL.len());
    }

    #[test]
    fn darkness_follows_the_name() {
        assert!(ColorTheme::DarkTritanopia.is_dark());
        assert!(!ColorTheme::LightHighContrast.is_dark());
    }
}
