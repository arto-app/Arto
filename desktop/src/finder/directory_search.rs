use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// A single match result from directory-wide search
#[derive(Debug, Clone, PartialEq)]
pub struct DirectorySearchMatch {
    /// Path of the file containing the match
    pub file_path: PathBuf,
    /// Line number within the file (1-based)
    pub line_number: usize,
    /// Full text of the matched line
    pub line_text: String,
    /// Fuzzy match score from nucleo
    pub score: u32,
    /// Character indices (UTF-32) where the pattern matched within line_text (source positions)
    pub match_indices: Vec<u32>,
    /// Character indices in rendered content text (source→content converted via source maps).
    /// Used by JS to place highlights at correct positions in the DOM.
    pub content_match_indices: Vec<u32>,
    /// Context lines before the match (up to CONTEXT_LINES)
    pub context_before: Vec<String>,
    /// Context lines after the match (up to CONTEXT_LINES)
    pub context_after: Vec<String>,
}

/// A single line candidate for fuzzy matching.
///
/// Used as the `T` parameter for `Nucleo<LineCandidate>` in the worker thread.
pub(crate) struct LineCandidate {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub line_text: String,
}

/// Maximum line length to index (skip very long lines for performance)
const MAX_LINE_LENGTH: usize = 500;

/// Read a single file and collect its lines as candidates.
pub(crate) fn collect_file_candidates(file: &Path) -> Vec<LineCandidate> {
    let mut candidates = Vec::new();

    let file_handle = match std::fs::File::open(file) {
        Ok(f) => f,
        Err(_) => return candidates,
    };
    let reader = std::io::BufReader::new(file_handle);

    for (line_idx, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() || line.len() > MAX_LINE_LENGTH {
            continue;
        }

        candidates.push(LineCandidate {
            file_path: file.to_path_buf(),
            line_number: line_idx + 1,
            line_text: line,
        });
    }

    candidates
}

/// Walk the directory tree and collect all text file lines as candidates.
///
/// Respects .gitignore, skips hidden files, and filters out likely binary files.
pub(crate) fn collect_candidates(root: &Path) -> Vec<LineCandidate> {
    let mut candidates = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(true) // skip hidden files
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for result in walker.flatten() {
        let file_type = match result.file_type() {
            Some(ft) => ft,
            None => continue,
        };
        if !file_type.is_file() {
            continue;
        }

        let path = result.path();
        if is_likely_binary(path) {
            continue;
        }

        // Read file lines
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let reader = std::io::BufReader::new(file);

        for (line_idx, line_result) in reader.lines().enumerate() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => break, // Binary content or encoding error
            };

            // Skip empty lines and very long lines
            let trimmed = line.trim();
            if trimmed.is_empty() || line.len() > MAX_LINE_LENGTH {
                continue;
            }

            candidates.push(LineCandidate {
                file_path: path.to_path_buf(),
                line_number: line_idx + 1, // 1-based
                line_text: line,
            });
        }
    }

    candidates
}

/// Check if a file is likely binary based on its extension.
fn is_likely_binary(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "svg" | "webp" | "tiff" | "tif"
        // Audio/Video
        | "mp3" | "mp4" | "avi" | "mkv" | "mov" | "wav" | "flac" | "ogg" | "webm"
        // Archives
        | "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst"
        // Executables/Libraries
        | "exe" | "dll" | "so" | "dylib" | "a" | "o" | "obj" | "wasm"
        // Documents (binary format)
        | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
        // Fonts
        | "ttf" | "otf" | "woff" | "woff2" | "eot"
        // Databases
        | "db" | "sqlite" | "sqlite3"
        // Other binary
        | "bin" | "dat" | "class" | "pyc" | "pyo"
        // Lock files (often very large)
        | "lock"
    )
}

/// Number of context lines to show before and after each match
const CONTEXT_LINES: usize = 2;

/// Read all lines from a file, returning None if the file cannot be read.
fn read_all_lines(path: &Path) -> Option<Vec<String>> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let lines: Result<Vec<_>, _> = reader.lines().collect();
    lines.ok()
}

/// Enrich search results with context lines (before/after) from source files.
///
/// Groups results by file path so each file is read only once, then extracts
/// surrounding lines clamped to file boundaries.
pub(crate) fn enrich_results_with_context(results: &mut [DirectorySearchMatch]) {
    // Group result indices by file path (clone paths to avoid borrowing results)
    let mut file_groups: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, r) in results.iter().enumerate() {
        file_groups.entry(r.file_path.clone()).or_default().push(i);
    }

    for (path, indices) in &file_groups {
        let Some(lines) = read_all_lines(path) else {
            continue;
        };
        for &i in indices {
            let r = &mut results[i];
            let idx = r.line_number.saturating_sub(1); // Convert to 0-based
            let start = idx.saturating_sub(CONTEXT_LINES);
            r.context_before = lines[start..idx].to_vec();
            let end = (idx + 1 + CONTEXT_LINES).min(lines.len());
            r.context_after = lines[(idx + 1)..end].to_vec();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create test files
        fs::write(
            dir.path().join("hello.md"),
            "# Hello World\nThis is a test file.\nAnother line here.\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("readme.txt"),
            "Project README\nContains some documentation.\n",
        )
        .unwrap();

        // Create a subdirectory with a file
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(
            dir.path().join("sub/nested.md"),
            "Nested file content\nWith fuzzy matching test.\n",
        )
        .unwrap();

        // Create a binary file (should be skipped)
        fs::write(dir.path().join("image.png"), vec![0x89, 0x50, 0x4E, 0x47]).unwrap();

        dir
    }

    #[test]
    fn test_collect_candidates_finds_text_files() {
        let dir = create_test_dir();
        let candidates = collect_candidates(dir.path());

        // Should find lines from hello.md, readme.txt, and sub/nested.md
        // but not from image.png
        assert!(candidates.len() >= 5);
        assert!(candidates
            .iter()
            .all(|c| !c.file_path.to_string_lossy().contains("image.png")));
    }

    #[test]
    fn test_collect_candidates_line_numbers_are_1_based() {
        let dir = create_test_dir();
        let candidates = collect_candidates(dir.path());

        let hello_candidates: Vec<_> = candidates
            .iter()
            .filter(|c| c.file_path.file_name().unwrap().to_str() == Some("hello.md"))
            .collect();

        assert!(!hello_candidates.is_empty());
        assert_eq!(hello_candidates[0].line_number, 1);
    }

    fn make_match(file_path: &Path, line_number: usize) -> DirectorySearchMatch {
        DirectorySearchMatch {
            file_path: file_path.to_path_buf(),
            line_number,
            line_text: String::new(),
            score: 0,
            match_indices: Vec::new(),
            content_match_indices: Vec::new(),
            context_before: Vec::new(),
            context_after: Vec::new(),
        }
    }

    #[test]
    fn test_enrich_results_with_context_middle_line() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "line1\nline2\nline3\nline4\nline5\nline6\nline7\n").unwrap();

        let mut results = vec![make_match(&file, 4)]; // Match on line 4 (0-based idx 3)
        enrich_results_with_context(&mut results);

        assert_eq!(results[0].context_before, vec!["line2", "line3"]);
        assert_eq!(results[0].context_after, vec!["line5", "line6"]);
    }

    #[test]
    fn test_enrich_results_with_context_first_line() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

        let mut results = vec![make_match(&file, 1)]; // Match on line 1
        enrich_results_with_context(&mut results);

        assert!(results[0].context_before.is_empty());
        assert_eq!(results[0].context_after, vec!["line2", "line3"]);
    }

    #[test]
    fn test_enrich_results_with_context_last_line() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

        let mut results = vec![make_match(&file, 4)]; // Match on line 4 (last)
        enrich_results_with_context(&mut results);

        assert_eq!(results[0].context_before, vec!["line2", "line3"]);
        // BufRead::lines() treats \n as terminator, so no trailing empty line
        assert!(results[0].context_after.is_empty());
    }

    #[test]
    fn test_enrich_results_with_context_unreadable_file() {
        let mut results = vec![make_match(Path::new("/nonexistent/file.txt"), 3)];
        enrich_results_with_context(&mut results);

        // Should not panic, context should remain empty
        assert!(results[0].context_before.is_empty());
        assert!(results[0].context_after.is_empty());
    }

    #[test]
    fn test_enrich_results_groups_by_file() {
        let dir = TempDir::new().unwrap();
        let file_a = dir.path().join("a.txt");
        let file_b = dir.path().join("b.txt");
        fs::write(&file_a, "a1\na2\na3\na4\na5\n").unwrap();
        fs::write(&file_b, "b1\nb2\nb3\n").unwrap();

        let mut results = vec![
            make_match(&file_a, 1),
            make_match(&file_a, 3),
            make_match(&file_b, 2),
        ];
        enrich_results_with_context(&mut results);

        // file_a, line 1: no before, 2 after
        assert!(results[0].context_before.is_empty());
        assert_eq!(results[0].context_after, vec!["a2", "a3"]);

        // file_a, line 3: 2 before, 2 after
        assert_eq!(results[1].context_before, vec!["a1", "a2"]);
        assert_eq!(results[1].context_after, vec!["a4", "a5"]);

        // file_b, line 2: 1 before, 1 after
        assert_eq!(results[2].context_before, vec!["b1"]);
        assert_eq!(results[2].context_after, vec!["b3"]);
    }

    #[test]
    fn test_is_likely_binary() {
        assert!(is_likely_binary(Path::new("image.png")));
        assert!(is_likely_binary(Path::new("video.mp4")));
        assert!(is_likely_binary(Path::new("archive.zip")));
        assert!(is_likely_binary(Path::new("font.woff2")));
        assert!(!is_likely_binary(Path::new("readme.md")));
        assert!(!is_likely_binary(Path::new("main.rs")));
        assert!(!is_likely_binary(Path::new("config.json")));
        assert!(!is_likely_binary(Path::new("no_extension")));
    }
}
