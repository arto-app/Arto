//! The colours a mark the reader keeps is drawn in.
//!
//! A pinned search and a highlight are different things — a word marked
//! wherever it appears, a place marked in one document — but they are drawn
//! from the same set, so a colour means the same on the page, on the ticks
//! and in the contents whichever of the two put it there.

use serde::{Deserialize, Serialize};

/// Colour of a mark the reader keeps.
///
/// Note: Yellow is excluded because it's reserved for the active search
/// (which has navigation support). This visual distinction helps users
/// differentiate between navigable search results and persistent marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightColor {
    #[default]
    Green,
    Blue,
    Pink,
    Orange,
    Purple,
}

impl HighlightColor {
    /// All available colors (Yellow excluded).
    pub const ALL: [HighlightColor; 5] = [
        HighlightColor::Green,
        HighlightColor::Blue,
        HighlightColor::Pink,
        HighlightColor::Orange,
        HighlightColor::Purple,
    ];

    /// Return the next color in the fixed rotation order.
    pub fn next(self) -> HighlightColor {
        match self {
            HighlightColor::Green => HighlightColor::Blue,
            HighlightColor::Blue => HighlightColor::Pink,
            HighlightColor::Pink => HighlightColor::Orange,
            HighlightColor::Orange => HighlightColor::Purple,
            HighlightColor::Purple => HighlightColor::Green,
        }
    }

    /// Get CSS class name for this color.
    pub fn css_class(&self) -> &'static str {
        match self {
            HighlightColor::Green => "highlight-green",
            HighlightColor::Blue => "highlight-blue",
            HighlightColor::Pink => "highlight-pink",
            HighlightColor::Orange => "highlight-orange",
            HighlightColor::Purple => "highlight-purple",
        }
    }

    /// Get the color name for JavaScript.
    pub fn to_js_name(self) -> &'static str {
        match self {
            HighlightColor::Green => "green",
            HighlightColor::Blue => "blue",
            HighlightColor::Pink => "pink",
            HighlightColor::Orange => "orange",
            HighlightColor::Purple => "purple",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_color_css_class() {
        assert_eq!(HighlightColor::Green.css_class(), "highlight-green");
        assert_eq!(HighlightColor::Blue.css_class(), "highlight-blue");
        assert_eq!(HighlightColor::Pink.css_class(), "highlight-pink");
        assert_eq!(HighlightColor::Orange.css_class(), "highlight-orange");
        assert_eq!(HighlightColor::Purple.css_class(), "highlight-purple");
    }

    #[test]
    fn test_highlight_color_next_rotation() {
        assert_eq!(HighlightColor::Green.next(), HighlightColor::Blue);
        assert_eq!(HighlightColor::Blue.next(), HighlightColor::Pink);
        assert_eq!(HighlightColor::Pink.next(), HighlightColor::Orange);
        assert_eq!(HighlightColor::Orange.next(), HighlightColor::Purple);
        assert_eq!(HighlightColor::Purple.next(), HighlightColor::Green);
    }

    #[test]
    fn the_js_name_is_the_serialized_name() {
        for color in HighlightColor::ALL {
            assert_eq!(
                serde_json::to_string(&color).unwrap(),
                format!("\"{}\"", color.to_js_name())
            );
        }
    }
}
