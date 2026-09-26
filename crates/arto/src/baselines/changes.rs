//! What changed between the version of a document last read and the one on
//! screen, line by line.

use serde::Serialize;
use similar::{capture_diff_slices_deadline, Algorithm, DiffOp};
use std::time::{Duration, Instant};

/// How long a comparison may search for the smallest set of changes before
/// it settles for a larger one. A document rewritten from top to bottom has
/// no small answer, and looking for one costs time that grows with the
/// square of its length.
const DEADLINE: Duration = Duration::from_millis(500);

/// A run of lines that changed, named by the lines of the version on screen
/// (1-based, both ends inclusive), which is what the page's
/// `data-source-range` counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    /// Lines that were not there before.
    Added { start: usize, end: usize },
    /// Lines that were there before, written differently.
    Modified { start: usize, end: usize },
    /// Lines that are gone, from just after line `after` (0: the top).
    Removed { after: usize },
}

/// What changed from `old` to `new`.
///
/// Lines end at `\n`, `\r\n` or a lone `\r`, as they do for the renderer,
/// so a file saved with other line endings is not changed on every line.
/// Blank lines separate blocks rather than being part of one, so a run that
/// only adds or takes away blank lines is no change at all, and one that
/// empties lines is a removal. `ignore_whitespace` also takes a line whose
/// words are spaced differently to be the same line.
pub fn changes(old: &str, new: &str, ignore_whitespace: bool) -> Vec<Change> {
    let old = lines(old, ignore_whitespace);
    let new = lines(new, ignore_whitespace);
    capture_diff_slices_deadline(
        Algorithm::Myers,
        &old,
        &new,
        Some(Instant::now() + DEADLINE),
    )
    .into_iter()
    .filter_map(|op| change(op, &old, &new))
    .collect()
}

fn change(op: DiffOp, old: &[String], new: &[String]) -> Option<Change> {
    let (old_lines, new_index, new_lines) = match op {
        DiffOp::Equal { .. } => return None,
        DiffOp::Delete {
            old_index,
            old_len,
            new_index,
        } => (&old[old_index..old_index + old_len], new_index, &new[0..0]),
        DiffOp::Insert {
            old_index,
            new_index,
            new_len,
        } => (
            &old[old_index..old_index],
            new_index,
            &new[new_index..new_index + new_len],
        ),
        DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        } => (
            &old[old_index..old_index + old_len],
            new_index,
            &new[new_index..new_index + new_len],
        ),
    };
    let had_text = old_lines.iter().any(|line| !is_blank(line));
    match written(new_lines) {
        Some((first, last)) => {
            let (start, end) = (new_index + first + 1, new_index + last + 1);
            Some(if had_text {
                Change::Modified { start, end }
            } else {
                Change::Added { start, end }
            })
        }
        None if had_text => Some(Change::Removed { after: new_index }),
        None => None,
    }
}

/// The first and last lines of `lines` that are not blank, as offsets.
fn written(lines: &[String]) -> Option<(usize, usize)> {
    let first = lines.iter().position(|line| !is_blank(line))?;
    let last = lines.iter().rposition(|line| !is_blank(line))?;
    Some((first, last))
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn lines(source: &str, ignore_whitespace: bool) -> Vec<String> {
    source
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| {
            if ignore_whitespace {
                line.split_whitespace().collect::<Vec<_>>().join(" ")
            } else {
                line.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    const BEFORE: &str = indoc! {"
        # Title

        First paragraph.

        Second paragraph.
    "};

    #[test]
    fn the_same_text_has_no_changes() {
        assert_eq!(changes(BEFORE, BEFORE, true), []);
    }

    #[test]
    fn lines_written_in_are_added() {
        let after = indoc! {"
            # Title

            First paragraph.

            A new one,
            over two lines.

            Second paragraph.
        "};

        assert_eq!(
            changes(BEFORE, after, true),
            [Change::Added { start: 5, end: 6 }]
        );
    }

    #[test]
    fn lines_rewritten_are_modified() {
        let after = BEFORE.replace("First paragraph.", "First paragraph, revised.");

        assert_eq!(
            changes(BEFORE, &after, true),
            [Change::Modified { start: 3, end: 3 }]
        );
    }

    #[test]
    fn lines_taken_out_are_removed_after_the_line_before_them() {
        let after = indoc! {"
            # Title

            Second paragraph.
        "};

        assert_eq!(changes(BEFORE, after, true), [Change::Removed { after: 2 }]);
    }

    #[test]
    fn lines_taken_from_the_top_are_removed_after_nothing() {
        let after = indoc! {"
            First paragraph.

            Second paragraph.
        "};

        assert_eq!(changes(BEFORE, after, true), [Change::Removed { after: 0 }]);
    }

    #[test]
    fn lines_taken_from_the_end_are_removed_after_the_last_line() {
        let after = indoc! {"
            # Title

            First paragraph.
        "};

        assert_eq!(changes(BEFORE, after, true), [Change::Removed { after: 3 }]);
    }

    #[test]
    fn whitespace_inside_a_line_is_ignored_when_asked() {
        let after = BEFORE.replace("First paragraph.", "First   paragraph.  ");

        assert_eq!(changes(BEFORE, &after, true), []);
        assert_eq!(
            changes(BEFORE, &after, false),
            [Change::Modified { start: 3, end: 3 }]
        );
    }

    #[test]
    fn other_line_endings_are_the_same_lines() {
        let crlf = BEFORE.replace('\n', "\r\n");
        let cr = BEFORE.replace('\n', "\r");

        assert_eq!(changes(BEFORE, &crlf, false), []);
        assert_eq!(changes(&crlf, &cr, false), []);
    }

    #[test]
    fn a_missing_newline_at_the_end_is_no_change() {
        assert_eq!(changes(BEFORE, BEFORE.trim_end(), false), []);
    }

    #[test]
    fn blank_lines_alone_are_no_change() {
        let after = BEFORE.replace("\n\n", "\n\n\n");

        assert_eq!(changes(BEFORE, &after, false), []);
    }

    #[test]
    fn a_run_is_named_by_its_written_lines_only() {
        let after = indoc! {"
            # Title

            First paragraph.


            Added.


            Second paragraph.
        "};

        assert_eq!(
            changes(BEFORE, after, false),
            [Change::Added { start: 6, end: 6 }]
        );
    }

    #[test]
    fn changes_serialize_the_way_the_page_reads_them() {
        let json = serde_json::to_value([
            Change::Added { start: 1, end: 2 },
            Change::Removed { after: 3 },
        ])
        .unwrap();

        assert_eq!(
            json,
            serde_json::json!([
                { "kind": "added", "start": 1, "end": 2 },
                { "kind": "removed", "after": 3 },
            ])
        );
    }
}
