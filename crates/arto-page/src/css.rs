//! The stylesheet a page carries: the app's own, trimmed to what the page
//! can paint with, and the overrides a standalone document needs.

use std::sync::LazyLock;

/// The frontend stylesheet, embedded at compile time.
const FRONTEND_CSS: &str = include_str!("../assets/frontend/main.css");

/// Style overrides for a standalone page, applied after [`FRONTEND_CSS`].
///
/// The shared stylesheet sets `body { overflow: hidden }` because the app
/// scrolls inside an inner `.content` container. A standalone document has no
/// such container, so inheriting that rule clips long documents and leaves the
/// page unscrollable. Restore natural document scrolling.
///
/// Only the root may name the viewport's overflow. The body's own value
/// reaches the viewport only while the root's is `visible`, so a body left at
/// `auto` next to an `auto` root becomes a scroll container of its own — one
/// that never scrolls, since it is as tall as the document. Anything sticky
/// holds against the nearest scroll container, and would hold against that.
pub(crate) const STANDALONE_OVERRIDE_CSS: &str =
    "html{overflow:auto!important;height:auto!important;}\
     body{overflow:visible!important;height:auto!important;}";

/// [`FRONTEND_CSS`] without the KaTeX font faces.
///
/// Those faces carry their woff2 inline and are most of the stylesheet by
/// weight, yet nothing but typeset math ever has a glyph to set in them.
/// Computed once per process: Quick Look builds a page on every press of
/// Space, and the scan runs over a megabyte-scale string.
static CSS_WITHOUT_MATH_FONTS: LazyLock<String> = LazyLock::new(|| {
    let mut css = strip_font_faces(FRONTEND_CSS, "KaTeX_");
    // The buffer was sized for a stylesheet with nothing to strip, and what is
    // left is a fraction of that. It is held for the life of the process.
    css.shrink_to_fit();
    css
});

/// The stylesheet for a page that sets math only when `math` is on and paints
/// only the `light` and `dark` themes.
pub(crate) fn stylesheet(math: bool, light: &str, dark: &str) -> String {
    let with_math_settled = if math {
        FRONTEND_CSS
    } else {
        &CSS_WITHOUT_MATH_FONTS
    };
    strip_unused_themes(with_math_settled, light, dark)
}

/// Remove every `@font-face` block that names a font family starting with
/// `prefix`.
fn strip_font_faces(css: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;

    while let Some(at) = rest.find("@font-face") {
        let Some(open) = rest[at..].find('{').map(|i| at + i) else {
            break;
        };
        let Some(end) = block_end(rest, open) else {
            break;
        };
        out.push_str(if rest[open..end].contains(prefix) {
            &rest[..at]
        } else {
            &rest[..end]
        });
        rest = &rest[end..];
    }

    out.push_str(rest);
    out
}

/// Remove the `[data-theme=…]` rules for every theme but `light` and `dark`.
///
/// The stylesheet carries all of GitHub's themes because the app lets the
/// reader pick any of them at any time. A page is written for one pair: the
/// bootstrap writes one of these two names onto the root element and nothing
/// afterwards can name a third.
fn strip_unused_themes(css: &str, light: &str, dark: &str) -> String {
    const MARKER: &str = "[data-theme=";

    let mut out = String::with_capacity(css.len());
    let mut rest = css;

    while let Some(at) = rest.find(MARKER) {
        let name_start = at + MARKER.len();
        let Some(name_end) = rest[name_start..].find(']').map(|i| name_start + i) else {
            break;
        };
        let Some(open) = rest[name_end..].find('{').map(|i| name_end + i) else {
            break;
        };
        let Some(end) = block_end(rest, open) else {
            break;
        };
        let name = &rest[name_start..name_end];
        if name == light || name == dark {
            out.push_str(&rest[..end]);
        } else {
            // A theme can share its rule with another selector
            // (`:root:not([data-theme]),[data-theme=light]{…}`). Cutting at the
            // selector itself would leave the rest of the list behind, to bind
            // to whatever rule follows — so cut where the rule starts.
            let rule_start = rest[..at].rfind(['}', '{', ';']).map_or(0, |i| i + 1);
            out.push_str(&rest[..rule_start]);
        }
        rest = &rest[end..];
    }

    out.push_str(rest);
    out
}

/// The index just past the `}` that closes the block opening at `open`.
fn block_end(css: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, c) in css[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_font_faces_removes_only_the_matching_faces() {
        let css = "a{color:red}@font-face{font-family:KaTeX_Main;src:url(x)}b{color:blue}@font-face{font-family:Other;src:url(y)}c{color:green}";

        assert_eq!(
            strip_font_faces(css, "KaTeX_"),
            "a{color:red}b{color:blue}@font-face{font-family:Other;src:url(y)}c{color:green}"
        );
    }

    #[test]
    fn test_strip_font_faces_leaves_a_stylesheet_without_a_match_alone() {
        let css = "a{color:red}@font-face{font-family:Other;src:url(y)}";

        assert_eq!(strip_font_faces(css, "KaTeX_"), css);
    }

    #[test]
    fn test_strip_unused_themes_keeps_only_the_named_pair() {
        let css = "a{color:red}[data-theme=light]{--x:1}[data-theme=dark_dimmed]{--x:2}[data-theme=dark]{--x:3}b{color:blue}";

        assert_eq!(
            strip_unused_themes(css, "light", "dark"),
            "a{color:red}[data-theme=light]{--x:1}[data-theme=dark]{--x:3}b{color:blue}"
        );
    }

    #[test]
    fn test_strip_unused_themes_drops_the_whole_selector_list_of_a_dropped_theme() {
        let css = "a{color:red}:root:not([data-theme]),[data-theme=light]{--x:1}@media print{b{color:blue}}";

        assert_eq!(
            strip_unused_themes(css, "light_high_contrast", "dark"),
            "a{color:red}@media print{b{color:blue}}"
        );
    }

    #[test]
    fn test_strip_unused_themes_allows_the_same_theme_on_both_sides() {
        let css = "[data-theme=light]{--x:1}[data-theme=dark]{--x:2}";

        assert_eq!(
            strip_unused_themes(css, "light", "light"),
            "[data-theme=light]{--x:1}"
        );
    }
}
