//! Ticking a task off in the file it was read from.
//!
//! A task list item is found by where its `<li>` starts in the source — the
//! `data-source-range` the renderer puts on it — and only the one character
//! between its brackets is replaced.

use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

/// A task the reader ticked, as the page names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub struct Tick {
    /// The line its list item starts on, from 1, in the whole file.
    pub line: usize,
    /// The column its list item starts at, from 1, in code points.
    pub column: usize,
    /// Whether the reader ticked it on or off.
    pub checked: bool,
}

/// Why a tick was not placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TickError {
    #[error("no task list item starts there")]
    NotATask,
    #[error("the task is no longer marked as the page showed it")]
    Stale,
}

/// Why a tick was not written to its file.
#[derive(Debug, thiserror::Error)]
pub enum TickFileError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Tick(#[from] TickError),
}

/// Tick the task at `tick` in the file at `path`, whose page was drawn from
/// `drawn`, and return what the file held before.
///
/// The file is read again rather than trusted to be what the page was drawn
/// from: a tick is written only while the task's line is still the line the
/// page showed, so a task that moved, or another one that took its place,
/// is never ticked in its stead.
///
/// Only the mark's byte is written, in place. The file stays the file it
/// was — its extended attributes, its ACL, every name it goes by — and an
/// editor that saved it meanwhile by putting another file in its place loses
/// nothing: the byte lands in the file it replaced.
pub fn tick_file(path: &Path, drawn: &str, tick: Tick) -> Result<String, TickFileError> {
    // Ticks from two windows on one file would otherwise both read the file
    // before either wrote it, and the second write would undo the first.
    static WRITING: Mutex<()> = Mutex::new(());
    let _writing = WRITING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Opened for writing before anything is read, so a file the reader made
    // read-only refuses here.
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut before = String::new();
    file.read_to_string(&mut before)?;
    // The mark itself is left out: the reader may tick the task again before
    // the page is drawn from what the first tick wrote.
    if unmarked_line(&before, tick) != unmarked_line(drawn, tick) {
        return Err(TickError::Stale.into());
    }
    let (offset, mark) = place(&before, tick)?;
    file.seek(SeekFrom::Start(offset as u64))?;
    file.write_all(&[mark])?;
    file.sync_data()?;
    Ok(before)
}

/// Whether the task whose list item starts at `line`:`column` is ticked, or
/// `None` when no task starts there.
pub fn checked_at(source: &str, line: usize, column: usize) -> Option<bool> {
    mark_at(source, line, column).map(|mark| source.as_bytes()[mark] != b' ')
}

/// `source` with the task at `tick` marked as `tick.checked`.
pub fn tick(source: &str, tick: Tick) -> Result<String, TickError> {
    let (offset, mark) = place(source, tick)?;
    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..offset]);
    out.push(char::from(mark));
    out.push_str(&source[offset + 1..]);
    Ok(out)
}

/// Where in `source` the mark of the task at `tick` is, and the byte that
/// marks it as `tick.checked`.
fn place(source: &str, tick: Tick) -> Result<(usize, u8), TickError> {
    let offset = mark_at(source, tick.line, tick.column).ok_or(TickError::NotATask)?;
    let checked = source.as_bytes()[offset] != b' ';
    if checked == tick.checked {
        return Err(TickError::Stale);
    }
    Ok((offset, if tick.checked { b'x' } else { b' ' }))
}

/// The byte offset of the character between the brackets of the task whose
/// list item starts at `line`:`column`.
fn mark_at(source: &str, line: usize, column: usize) -> Option<usize> {
    let line_start = line_start(source, line)?;
    let text = self::line(source, line)?;
    let item_start = match column.checked_sub(1)? {
        0 => 0,
        skip => text.char_indices().nth(skip)?.0,
    };
    let item = &text[item_start..];

    let after_marker = strip_list_marker(item)?;
    let gap =
        &after_marker[..after_marker.len() - after_marker.trim_start_matches([' ', '\t']).len()];
    if !(gap == "\t" || (1..=4).contains(&gap.len()) && !gap.contains('\t')) {
        return None;
    }
    let bracket = item.len() - after_marker.len() + gap.len();

    // `item` ends where the line does, so a mark with nothing after it ends
    // the slice.
    let is_task = matches!(
        item.as_bytes().get(bracket..),
        Some([b'[', b' ' | b'x' | b'X', b']'] | [b'[', b' ' | b'x' | b'X', b']', b' ' | b'\t', ..])
    );
    is_task.then_some(line_start + item_start + bracket + 1)
}

/// The line of `source` the task at `tick` is on, with its mark blanked out,
/// or the line as it is when no task starts there.
fn unmarked_line(source: &str, tick: Tick) -> Option<String> {
    let text = line(source, tick.line)?;
    let Some(mark) = mark_at(source, tick.line, tick.column) else {
        return Some(text.to_string());
    };
    let at = mark - line_start(source, tick.line)?;
    Some(format!("{} {}", &text[..at], &text[at + 1..]))
}

/// Line `line` (from 1) of `source`, without its line ending.
pub(crate) fn line(source: &str, line: usize) -> Option<&str> {
    let text = &source[line_start(source, line)?..];
    Some(&text[..text.find(['\n', '\r']).unwrap_or(text.len())])
}

/// The byte offset `line` (from 1) starts at, counting `\n`, `\r\n` and a
/// lone `\r` each as one line ending, as the renderer does.
fn line_start(source: &str, line: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let mut start = 0;
    for _ in 1..line {
        let end = start + source[start..].find(['\n', '\r'])?;
        start = if source[end..].starts_with("\r\n") {
            end + 2
        } else {
            end + 1
        };
    }
    Some(start)
}

/// `text` after a bullet (`-`, `*`, `+`) or an ordered marker (`1.`, `1)`).
fn strip_list_marker(text: &str) -> Option<&str> {
    if let Some(rest) = text.strip_prefix(['-', '*', '+']) {
        return Some(rest);
    }
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if !(1..=9).contains(&digits) {
        return None;
    }
    text[digits..].strip_prefix(['.', ')'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;

    fn at(line: usize, column: usize, checked: bool) -> Tick {
        Tick {
            line,
            column,
            checked,
        }
    }

    #[test]
    fn an_open_task_is_ticked_on() {
        let source = "- [ ] Write\n- [ ] Read\n";
        assert_eq!(
            tick(source, at(2, 1, true)),
            Ok("- [ ] Write\n- [x] Read\n".to_string())
        );
    }

    #[test]
    fn a_done_task_is_ticked_off_whichever_case_its_x_is() {
        assert_eq!(
            tick("- [x] a\n", at(1, 1, false)),
            Ok("- [ ] a\n".to_string())
        );
        assert_eq!(
            tick("- [X] a\n", at(1, 1, false)),
            Ok("- [ ] a\n".to_string())
        );
    }

    #[test]
    fn every_list_marker_is_read() {
        for marker in ["-", "*", "+", "1.", "12)"] {
            let source = format!("{marker} [ ] a\n");
            assert_eq!(
                tick(&source, at(1, 1, true)),
                Ok(format!("{marker} [x] a\n")),
                "{marker}"
            );
        }
    }

    #[test]
    fn a_nested_task_is_found_by_its_column() {
        let source = indoc! {"
            - [ ] outer
              - [ ] inner
        "};
        assert_eq!(
            tick(source, at(2, 3, true)),
            Ok(indoc! {"
                - [ ] outer
                  - [x] inner
            "}
            .to_string())
        );
    }

    #[test]
    fn a_task_in_a_quote_is_found_by_its_column() {
        assert_eq!(
            tick("> - [ ] a\n", at(1, 3, true)),
            Ok("> - [x] a\n".to_string())
        );
    }

    #[test]
    fn columns_count_code_points_not_bytes() {
        // `　` is one code point and three bytes.
        assert_eq!(
            tick("　- [ ] 日本語\n", at(1, 2, true)),
            Ok("　- [x] 日本語\n".to_string())
        );
    }

    #[test]
    fn the_marker_may_be_followed_by_a_tab_or_several_spaces() {
        assert_eq!(
            tick("-\t[ ] a\n", at(1, 1, true)),
            Ok("-\t[x] a\n".to_string())
        );
        assert_eq!(
            tick("1.   [ ] a\n", at(1, 1, true)),
            Ok("1.   [x] a\n".to_string())
        );
    }

    #[test]
    fn every_line_ending_counts_as_one_and_is_kept() {
        assert_eq!(
            tick("- [ ] a\r\n- [ ] b\r\n", at(2, 1, true)),
            Ok("- [ ] a\r\n- [x] b\r\n".to_string())
        );
        assert_eq!(
            tick("- [ ] a\r- [ ] b\r", at(2, 1, true)),
            Ok("- [ ] a\r- [x] b\r".to_string())
        );
    }

    #[test]
    fn a_task_on_the_last_line_without_a_newline_is_found() {
        assert_eq!(tick("- [ ] a", at(1, 1, true)), Ok("- [x] a".to_string()));
    }

    #[test]
    fn a_mark_that_already_says_what_was_asked_is_stale() {
        assert_eq!(tick("- [x] a\n", at(1, 1, true)), Err(TickError::Stale));
        assert_eq!(tick("- [ ] a\n", at(1, 1, false)), Err(TickError::Stale));
    }

    /// Where each checkbox the renderer draws says its list item starts,
    /// and whether it is drawn checked.
    fn drawn_tasks(source: &str) -> Vec<(usize, usize, bool)> {
        let html = arto_markdown::render_to_html(
            source,
            "/notes/a.md",
            &arto_markdown::RenderOptions::default(),
        )
        .unwrap()
        .html;
        html.split(r#"<li data-source-range=""#)
            .skip(1)
            .filter_map(|item| {
                let (range, rest) = item.split_once('"')?;
                let checkbox = rest.strip_prefix("><input type=\"checkbox\"")?;
                let (start, _) = range.split_once('-')?;
                let (line, column) = start.split_once(':')?;
                Some((
                    line.parse().ok()?,
                    column.parse().ok()?,
                    checkbox.starts_with(" checked"),
                ))
            })
            .collect()
    }

    #[test]
    fn every_checkbox_the_renderer_draws_is_found_where_its_item_starts() {
        let source = indoc! {"
            ---
            title: Tasks
            ---

            - [ ] plain
            - [x] done
            * [ ] star
            1. [ ] ordered
            2) [X] paren
              - [ ] not nested, lazy

            - [ ] outer
              - [ ] nested
                1. [x] deeper

            > - [ ] quoted
            > > * [x] twice quoted

            > [!NOTE]
            > - [ ] in an alert

            - [ ] 日本語の　タスク
            - [ ] **bold** and `code`

            - [ ]
            - [x]
        "};
        let drawn = drawn_tasks(source);
        assert_eq!(drawn.len(), 16, "{drawn:?}");

        for (line, column, checked) in drawn {
            let ticked = tick(source, at(line, column, !checked))
                .unwrap_or_else(|e| panic!("{line}:{column}: {e}"));
            let redrawn = drawn_tasks(&ticked);
            assert!(
                redrawn.contains(&(line, column, !checked)),
                "{line}:{column} was not flipped: {redrawn:?}"
            );
        }
    }

    #[test]
    fn a_tick_writes_the_file_and_returns_what_it_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "- [ ] a\n").unwrap();

        assert_eq!(
            tick_file(&path, "- [ ] a\n", at(1, 1, true)).unwrap(),
            "- [ ] a\n"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [x] a\n");
    }

    #[test]
    fn a_tick_the_file_no_longer_matches_leaves_it_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "- [x] a\n").unwrap();

        assert!(matches!(
            tick_file(&path, "- [ ] a\n", at(1, 1, true)),
            Err(TickFileError::Tick(TickError::Stale))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [x] a\n");
    }

    #[test]
    fn a_task_that_moved_under_the_page_is_not_ticked_in_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        // Drawn as `a` then `b`; the file has since been reordered.
        fs::write(&path, "- [ ] b\n- [ ] a\n").unwrap();

        assert!(matches!(
            tick_file(&path, "- [ ] a\n- [ ] b\n", at(1, 1, true)),
            Err(TickFileError::Tick(TickError::Stale))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [ ] b\n- [ ] a\n");
    }

    #[test]
    fn a_task_ticked_again_before_the_page_is_drawn_again_is_ticked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "- [ ] a\n").unwrap();
        let drawn = "- [ ] a\n";

        tick_file(&path, drawn, at(1, 1, true)).unwrap();
        tick_file(&path, drawn, at(1, 1, false)).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "- [ ] a\n");
    }

    #[test]
    fn a_change_elsewhere_in_the_file_does_not_stop_a_tick() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "- [ ] a\n- [ ] b\n- [ ] c, added\n").unwrap();

        tick_file(&path, "- [ ] a\n- [ ] b\n", at(2, 1, true)).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "- [ ] a\n- [x] b\n- [ ] c, added\n"
        );
    }

    #[test]
    fn a_file_made_read_only_is_not_ticked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "- [ ] a\n").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();

        assert!(matches!(
            tick_file(&path, "- [ ] a\n", at(1, 1, true)),
            Err(TickFileError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [ ] a\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_tick_keeps_the_file_it_writes_rather_than_replacing_it() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        let other_name = dir.path().join("hard-link.md");
        fs::write(&path, "- [ ] a\n").unwrap();
        fs::hard_link(&path, &other_name).unwrap();
        let inode = fs::metadata(&path).unwrap().ino();

        tick_file(&path, "- [ ] a\n", at(1, 1, true)).unwrap();

        // The same file, so its extended attributes, its ACL and every name
        // it goes by stay with it.
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(fs::read_to_string(&other_name).unwrap(), "- [x] a\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_tick_through_a_symlink_writes_the_file_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.md");
        let link = dir.path().join("link.md");
        fs::write(&real, "- [ ] a\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        tick_file(&link, "- [ ] a\n", at(1, 1, true)).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "- [x] a\n");
    }

    #[test]
    fn a_task_reads_as_its_mark_says() {
        let source = "- [ ] open\n- [X] done\n- not a task\n";
        assert_eq!(checked_at(source, 1, 1), Some(false));
        assert_eq!(checked_at(source, 2, 1), Some(true));
        assert_eq!(checked_at(source, 3, 1), None);
    }

    #[test]
    fn a_task_with_nothing_after_its_mark_is_ticked() {
        assert_eq!(tick("- [ ]\n", at(1, 1, true)), Ok("- [x]\n".to_string()));
        assert_eq!(tick("- [x]", at(1, 1, false)), Ok("- [ ]".to_string()));
    }

    #[test]
    fn a_place_that_holds_no_task_writes_nothing() {
        for (source, place) in [
            ("- a\n", at(1, 1, true)),
            ("[ ] a\n", at(1, 1, true)),
            ("-[ ] a\n", at(1, 1, true)),
            ("- [y] a\n", at(1, 1, true)),
            ("-     [ ] a\n", at(1, 1, true)),
            ("- [ ] a\n", at(1, 2, true)),
            ("- [ ] a\n", at(2, 1, true)),
            ("- [ ] a\n", at(0, 1, true)),
            ("- [ ] a\n", at(1, 0, true)),
            ("- [ ] a\n", at(1, 99, true)),
        ] {
            assert_eq!(
                tick(source, place),
                Err(TickError::NotATask),
                "{source:?} {place:?}"
            );
        }
    }
}
