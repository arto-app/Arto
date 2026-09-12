use arto_lsp::{WindowExtent, WindowPoint};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOpenMode {
    /// Use the behavior from config.json (fileOpen setting).
    Config,
    LastFocused,
    CurrentScreen,
    NewWindow,
}

impl CliOpenMode {
    pub(crate) fn to_file_open_behavior(self) -> Option<crate::config::FileOpenBehavior> {
        match self {
            Self::Config => None,
            Self::LastFocused => Some(crate::config::FileOpenBehavior::LastFocused),
            Self::CurrentScreen => Some(crate::config::FileOpenBehavior::CurrentScreen),
            Self::NewWindow => Some(crate::config::FileOpenBehavior::NewWindow),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliInvocation {
    pub paths: Vec<PathBuf>,
    pub directory: Option<PathBuf>,
    pub open_mode: CliOpenMode,
    /// Open without activating Arto, leaving the keyboard focus where it is.
    pub behind: bool,
    /// Geometry and theme asked for by this invocation, for the window it
    /// lands in. Empty when the invocation asked for none of them.
    pub window: arto_lsp::WindowOptions,
    /// Return only once the target window has drawn the document, rather
    /// than as soon as the request is handed over.
    pub wait_ready: bool,
}

/// Read `--position=<x>,<y>`.
///
/// Negative coordinates are ordinary: a display left of or above the main
/// one starts at a negative origin, and a window placed there is placed
/// deliberately.
pub fn parse_position(value: &str) -> Result<WindowPoint, String> {
    let (x, y) = split_pair(value, "x,y")?;
    Ok(WindowPoint {
        x: parse_field::<i32>(x, "x")?,
        y: parse_field::<i32>(y, "y")?,
    })
}

/// Read `--size=<w>,<h>`.
pub fn parse_size(value: &str) -> Result<WindowExtent, String> {
    let (width, height) = split_pair(value, "width,height")?;
    let extent = WindowExtent {
        width: parse_field::<u32>(width, "width")?,
        height: parse_field::<u32>(height, "height")?,
    };
    if extent.width == 0 || extent.height == 0 {
        return Err("width and height must both be greater than zero".to_string());
    }
    Ok(extent)
}

fn split_pair<'a>(value: &'a str, shape: &str) -> Result<(&'a str, &'a str), String> {
    match value.split_once(',') {
        Some((first, second)) => Ok((first.trim(), second.trim())),
        None => Err(format!(
            "expected {shape}, as two numbers separated by a comma"
        )),
    }
}

fn parse_field<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{name} is not a number: {value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_is_two_numbers_separated_by_a_comma() {
        assert_eq!(parse_position("120,64"), Ok(WindowPoint { x: 120, y: 64 }));
        assert_eq!(
            parse_position(" 120 , 64 "),
            Ok(WindowPoint { x: 120, y: 64 })
        );
    }

    #[test]
    fn a_display_left_of_the_main_one_has_a_negative_origin() {
        assert_eq!(
            parse_position("-1920,-200"),
            Ok(WindowPoint { x: -1920, y: -200 })
        );
    }

    #[test]
    fn a_size_is_two_numbers_separated_by_a_comma() {
        assert_eq!(
            parse_size("1400,920"),
            Ok(WindowExtent {
                width: 1400,
                height: 920
            })
        );
    }

    #[test]
    fn a_window_cannot_be_asked_for_with_no_extent() {
        assert!(parse_size("0,920").is_err());
        assert!(parse_size("1400,0").is_err());
        assert!(parse_size("-1,920").is_err());
    }

    #[test]
    fn a_pair_that_is_not_a_pair_says_what_was_expected() {
        assert_eq!(
            parse_position("120"),
            Err("expected x,y, as two numbers separated by a comma".to_string())
        );
        assert_eq!(
            parse_size("1400"),
            Err("expected width,height, as two numbers separated by a comma".to_string())
        );
    }

    #[test]
    fn a_field_that_is_not_a_number_says_which_one() {
        assert_eq!(
            parse_position("120,middle"),
            Err(r#"y is not a number: "middle""#.to_string())
        );
        assert_eq!(
            parse_size("wide,920"),
            Err(r#"width is not a number: "wide""#.to_string())
        );
    }
}
