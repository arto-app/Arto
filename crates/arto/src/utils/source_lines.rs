use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Read a source file and extract lines in the range `start..=end` (1-based, inclusive).
///
/// The rendered HTML carries `data-source-line` attributes (see arto-markdown);
/// this is the step that turns such a range back into the original text.
///
/// Uses `BufReader` to read line-by-line, avoiding loading the entire file into memory.
///
/// Returns `None` when the file cannot be read, when the range is invalid
/// (`start == 0` or `end < start`), or when the range yields no lines because
/// `start` lies beyond the end of the file. A range that runs past the end
/// but starts inside the file returns the lines that exist, so the result
/// may contain fewer lines than requested.
pub fn extract_source_lines(file: impl AsRef<Path>, start: u32, end: u32) -> Option<String> {
    // Lines are 1-based; reject 0 or inverted ranges
    if start == 0 || end < start {
        return None;
    }
    let reader = BufReader::new(File::open(file.as_ref()).ok()?);
    let mut result = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line_num = (idx as u32) + 1;
        if line_num > end {
            break;
        }
        if line_num >= start {
            result.push(line.ok()?);
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_single_line() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            line 1
            line 2
            line 3
        "},
        )
        .unwrap();

        assert_eq!(
            extract_source_lines(&file, 2, 2),
            Some("line 2".to_string())
        );
    }

    #[test]
    fn test_extract_range() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            alpha
            beta
            gamma
            delta
        "},
        )
        .unwrap();

        assert_eq!(
            extract_source_lines(&file, 2, 3),
            Some("beta\ngamma".to_string())
        );
    }

    #[test]
    fn test_extract_entire_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            one
            two
            three
        "},
        )
        .unwrap();

        assert_eq!(
            extract_source_lines(&file, 1, 3),
            Some("one\ntwo\nthree".to_string())
        );
    }

    #[test]
    fn test_extract_beyond_file_length() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(
            &file,
            indoc! {"
            only
            two
        "},
        )
        .unwrap();

        // Request lines 1-100, should return only existing lines
        assert_eq!(
            extract_source_lines(&file, 1, 100),
            Some("only\ntwo".to_string())
        );
    }

    #[test]
    fn test_extract_nonexistent_file() {
        assert_eq!(extract_source_lines("/nonexistent/path.md", 1, 5), None);
    }

    #[test]
    fn test_extract_start_beyond_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(&file, "one line\n").unwrap();

        assert_eq!(extract_source_lines(&file, 10, 20), None);
    }

    #[test]
    fn test_extract_zero_start() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(&file, "line 1\nline 2\n").unwrap();

        // start=0 is invalid for 1-based line numbers
        assert_eq!(extract_source_lines(&file, 0, 5), None);
    }

    #[test]
    fn test_extract_inverted_range() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.md");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        // end < start is an inverted range
        assert_eq!(extract_source_lines(&file, 5, 2), None);
    }
}
