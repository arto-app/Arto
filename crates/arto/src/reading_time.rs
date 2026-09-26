//! How long a document takes to read, and how long is left.
//!
//! The renderer counts what each top-level block asks a reader to read
//! (`arto_markdown::ReadingProfile`); this turns the counts into time at the
//! reader's own speed, and finds what is left from the scroll anchor, which
//! names the block at the top of the view by the line it starts on. Nothing
//! here measures the page, so a document full of tall diagrams or long code
//! does not make the estimate run ahead of the text.

use crate::config::ReadingConfig;
use crate::markdown::{ReadingBlock, ReadingProfile};
use crate::scroll_anchor::ScrollAnchor;

/// Code is scanned rather than read, a line at a time.
const SECONDS_PER_CODE_LINE: f64 = 2.0;
/// An image, a diagram or a formula is looked at as a whole.
const SECONDS_PER_FIGURE: f64 = 10.0;

/// How fast the reader reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speeds {
    pub words_per_minute: u32,
    pub characters_per_minute: u32,
}

impl From<&ReadingConfig> for Speeds {
    fn from(config: &ReadingConfig) -> Self {
        Self {
            words_per_minute: config.words_per_minute,
            characters_per_minute: config.characters_per_minute,
        }
    }
}

/// What the header shows: the estimate, and its breakdown on hover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    pub label: String,
    pub title: String,
}

/// The reading time to show for `profile` with the reader at `anchor`, or
/// `None` when there is nothing worth showing: the setting is off, the
/// document is too short for an estimate to matter, or the reader has
/// reached its end.
pub fn estimate(
    profile: &ReadingProfile,
    anchor: ScrollAnchor,
    at_end: bool,
    config: &ReadingConfig,
) -> Option<Estimate> {
    if !config.show_time {
        return None;
    }
    let speeds = Speeds::from(config);
    let total = total_seconds(profile, speeds);
    let remaining = remaining_seconds(profile, anchor, speeds);
    let label = label(
        total,
        remaining,
        anchor.is_top(),
        at_end,
        config.min_minutes,
    )?;
    Some(Estimate {
        label,
        title: tooltip(profile, total),
    })
}

fn block_seconds(block: &ReadingBlock, speeds: Speeds) -> f64 {
    // A speed of zero would be a division by zero, not a very slow reader.
    let words_per_minute = f64::from(speeds.words_per_minute.max(1));
    let characters_per_minute = f64::from(speeds.characters_per_minute.max(1));
    f64::from(block.words) * 60.0 / words_per_minute
        + f64::from(block.cjk_chars) * 60.0 / characters_per_minute
        + f64::from(block.code_lines) * SECONDS_PER_CODE_LINE
        + f64::from(block.figures) * SECONDS_PER_FIGURE
}

/// Seconds to read the whole document.
pub fn total_seconds(profile: &ReadingProfile, speeds: Speeds) -> f64 {
    profile
        .blocks
        .iter()
        .map(|block| block_seconds(block, speeds))
        .sum()
}

/// Seconds left to read below `anchor`: every block after the one at the top
/// of the view, and the part of that one not yet scrolled past.
///
/// The block is the last one starting at or above the anchor's line, so an
/// anchor left over from an earlier version of the file still lands on a
/// block rather than on nothing.
pub fn remaining_seconds(profile: &ReadingProfile, anchor: ScrollAnchor, speeds: Speeds) -> f64 {
    let blocks = &profile.blocks;
    let after = blocks.partition_point(|block| block.line <= anchor.line);
    if anchor.is_top() || after == 0 {
        return total_seconds(profile, speeds);
    }
    let unread = 1.0 - f64::from(anchor.fraction).clamp(0.0, 1.0);
    let current = block_seconds(&blocks[after - 1], speeds) * unread;
    current
        + blocks[after..]
            .iter()
            .map(|block| block_seconds(block, speeds))
            .sum::<f64>()
}

fn minutes(seconds: f64) -> u64 {
    ((seconds / 60.0).round() as u64).max(1)
}

/// The label for a document of `total` seconds with `remaining` left.
///
/// At the top it is the whole document's time, since nothing has been read
/// yet — even when the document fits on one screen and its end is already
/// in view. Past the top it is the time left, and nothing at the end.
pub fn label(
    total: f64,
    remaining: f64,
    at_top: bool,
    at_end: bool,
    min_minutes: u32,
) -> Option<String> {
    if total < f64::from(min_minutes) * 60.0 || total <= 0.0 {
        return None;
    }
    if at_top {
        return Some(format!("{} min read", minutes(total)));
    }
    if at_end {
        return None;
    }
    if remaining < 60.0 {
        return Some("<1 min left".to_string());
    }
    Some(format!("{} min left", minutes(remaining)))
}

/// The breakdown behind the estimate. A count of zero is left out: an
/// English document has no characters to report.
fn tooltip(profile: &ReadingProfile, total: f64) -> String {
    let (words, characters) = profile.blocks.iter().fold((0u64, 0u64), |(w, c), block| {
        (w + u64::from(block.words), c + u64::from(block.cjk_chars))
    });
    let mut parts = vec![format!("{} min total", minutes(total))];
    if words > 0 {
        parts.push(format!("{} words", thousands(words)));
    }
    if characters > 0 {
        parts.push(format!("{} characters", thousands(characters)));
    }
    parts.join(" · ")
}

fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEEDS: Speeds = Speeds {
        words_per_minute: 60,
        characters_per_minute: 120,
    };

    fn words(line: u32, words: u32) -> ReadingBlock {
        ReadingBlock {
            line,
            words,
            ..Default::default()
        }
    }

    fn profile(blocks: Vec<ReadingBlock>) -> ReadingProfile {
        ReadingProfile { blocks }
    }

    fn at(line: u32, fraction: f32) -> ScrollAnchor {
        ScrollAnchor { line, fraction }
    }

    #[test]
    fn every_kind_of_count_takes_its_own_time() {
        let block = ReadingBlock {
            line: 1,
            words: 60,
            cjk_chars: 120,
            code_lines: 3,
            figures: 2,
        };
        assert_eq!(
            total_seconds(&profile(vec![block]), SPEEDS),
            60.0 + 60.0 + 6.0 + 20.0
        );
    }

    #[test]
    fn a_speed_of_zero_is_not_a_division_by_zero() {
        let speeds = Speeds {
            words_per_minute: 0,
            characters_per_minute: 0,
        };
        assert!(total_seconds(&profile(vec![words(1, 1)]), speeds).is_finite());
    }

    #[test]
    fn at_the_top_everything_is_left() {
        let p = profile(vec![words(1, 60), words(5, 60)]);
        assert_eq!(remaining_seconds(&p, ScrollAnchor::TOP, SPEEDS), 120.0);
    }

    #[test]
    fn inside_a_block_only_its_unread_part_is_left() {
        let p = profile(vec![words(1, 60), words(5, 60), words(9, 60)]);
        assert_eq!(remaining_seconds(&p, at(5, 0.25), SPEEDS), 45.0 + 60.0);
        assert_eq!(remaining_seconds(&p, at(9, 0.5), SPEEDS), 30.0);
    }

    #[test]
    fn a_line_between_blocks_belongs_to_the_block_above_it() {
        let p = profile(vec![words(1, 60), words(5, 60)]);
        assert_eq!(remaining_seconds(&p, at(3, 0.0), SPEEDS), 120.0);
    }

    #[test]
    fn a_stale_anchor_does_not_panic() {
        let p = profile(vec![words(10, 60), words(20, 60)]);
        // Before the first block: nothing has been read.
        assert_eq!(remaining_seconds(&p, at(4, 0.5), SPEEDS), 120.0);
        // Past the last block: only what is left of it.
        assert_eq!(remaining_seconds(&p, at(999, 0.5), SPEEDS), 30.0);
        // Out-of-range fractions are clamped.
        assert_eq!(remaining_seconds(&p, at(20, 7.0), SPEEDS), 0.0);
        assert_eq!(
            remaining_seconds(&ReadingProfile::default(), at(3, 0.1), SPEEDS),
            0.0
        );
    }

    #[test]
    fn a_document_under_the_threshold_shows_nothing() {
        assert_eq!(label(170.0, 170.0, true, false, 3), None);
        assert_eq!(label(0.0, 0.0, true, false, 0), None);
    }

    #[test]
    fn at_the_top_the_whole_time_is_shown() {
        assert_eq!(
            label(720.0, 720.0, true, false, 3).as_deref(),
            Some("12 min read")
        );
        // A document that fits on one screen has its end in view at once.
        assert_eq!(
            label(720.0, 720.0, true, true, 3).as_deref(),
            Some("12 min read")
        );
    }

    #[test]
    fn past_the_top_the_time_left_is_shown() {
        assert_eq!(
            label(720.0, 480.0, false, false, 3).as_deref(),
            Some("8 min left")
        );
        assert_eq!(
            label(720.0, 70.0, false, false, 3).as_deref(),
            Some("1 min left")
        );
        assert_eq!(
            label(720.0, 59.0, false, false, 3).as_deref(),
            Some("<1 min left")
        );
    }

    #[test]
    fn at_the_end_nothing_is_shown() {
        assert_eq!(label(720.0, 30.0, false, true, 3), None);
    }

    #[test]
    fn the_tooltip_breaks_the_total_down() {
        let p = profile(vec![ReadingBlock {
            line: 1,
            words: 3_400,
            cjk_chars: 5_200,
            ..Default::default()
        }]);
        assert_eq!(
            tooltip(&p, 720.0),
            "12 min total · 3,400 words · 5,200 characters"
        );
        assert_eq!(
            tooltip(&profile(vec![words(1, 999)]), 240.0),
            "4 min total · 999 words"
        );
    }

    #[test]
    fn thousands_are_separated() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn the_setting_turns_it_off() {
        let p = profile(vec![words(1, 10_000)]);
        let on = ReadingConfig::default();
        let off = ReadingConfig {
            show_time: false,
            ..ReadingConfig::default()
        };
        assert!(estimate(&p, ScrollAnchor::TOP, false, &on).is_some());
        assert_eq!(estimate(&p, ScrollAnchor::TOP, false, &off), None);
    }

    #[test]
    fn the_reader_s_speed_is_used() {
        let p = profile(vec![words(1, 2_300)]);
        let config = ReadingConfig {
            words_per_minute: 230,
            ..ReadingConfig::default()
        };
        let shown = estimate(&p, ScrollAnchor::TOP, false, &config).unwrap();
        assert_eq!(shown.label, "10 min read");
        assert_eq!(shown.title, "10 min total · 2,300 words");
    }
}
