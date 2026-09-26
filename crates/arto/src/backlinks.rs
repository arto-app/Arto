//! The documents that link to the one being read.
//!
//! A link is found the way a click follows it: `arto_markdown::document_links`
//! says which links a source makes and what path each one opens, and the path
//! is joined onto the source's own folder. What is left here is deciding
//! which of those land on the document on screen, and saying where in the
//! source each one was written.

use crate::markdown::RenderOptions;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// How much of a line is kept to show where a link was written.
///
/// Enough to read the sentence around the link in a panel row; a longer line
/// is cut down to a window around the link rather than to its start, which
/// could leave the link itself out.
const EXCERPT_CHARS: usize = 120;

/// How much of what comes before a link is kept when its line is cut down.
const EXCERPT_LEAD_CHARS: usize = 30;

/// A line of a source, cut down to show one link in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    pub text: String,
    /// The bytes of `text` the link itself covers.
    pub mark: Range<usize>,
}

impl Excerpt {
    /// The text before the link, the link, and the text after it.
    pub fn parts(&self) -> (&str, &str, &str) {
        (
            &self.text[..self.mark.start],
            &self.text[self.mark.clone()],
            &self.text[self.mark.end..],
        )
    }
}

/// One link a source makes, resolved to the file it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLink {
    /// The file the link opens, as [`crate::roots::canonical_key`] spells it.
    pub target: PathBuf,
    /// The 1-based line of the source the link was written on.
    pub line: u32,
    pub excerpt: Excerpt,
}

/// Every link in `content` that opens an existing Markdown document.
///
/// `source` is the file `content` was read from; relative links are joined
/// onto its folder, as a click on them would be.
pub fn links_of(source: &Path, content: &str, options: &RenderOptions) -> Vec<ResolvedLink> {
    let folder = source.parent().unwrap_or_else(|| Path::new("."));
    let lines = lines(content);
    arto_markdown::document_links(content, options)
        .into_iter()
        .filter_map(|link| {
            let target = resolve(folder, &link.path)?;
            let start = link.range.start;
            let text = lines.get(start.line.checked_sub(1)?)?;
            // A link that runs onto the next line is marked to the end of
            // the line it starts on, which is the one the row shows.
            let end = if link.range.end.line == start.line {
                link.range.end.column
            } else {
                text.chars().count()
            };
            Some(ResolvedLink {
                target,
                line: u32::try_from(start.line).ok()?,
                excerpt: excerpt(text, start.column, end),
            })
        })
        .collect()
}

/// The file a link opens from `folder`, or `None` when there is none — by
/// the rule a click follows ([`crate::document_link::resolve_link_path`]),
/// spelled for comparison by [`crate::roots::canonical_key`].
pub fn resolve(folder: &Path, path: &str) -> Option<PathBuf> {
    crate::document_link::resolve_link_path(folder, path)
        .filter(|path| path.is_file())
        .map(|path| crate::roots::canonical_key(&path))
}

/// Whether a source could name the document whose file stem is `stem`.
///
/// Read before a source is parsed, and only to decide whether to parse it:
/// a link to a document spells its name, give or take case — the disk may
/// ignore it — and percent-encoding, which Markdown asks for in a path with
/// a space in it. Most of a folder does not mention a given document at all,
/// and those files are never parsed on its account.
///
/// A link through a symlink of another name is the one it cannot see; that
/// source is found once it has been parsed for some other document.
pub fn mentions(content: &[u8], stem: &str) -> bool {
    let stem = stem.to_lowercase();
    String::from_utf8_lossy(content)
        .to_lowercase()
        .contains(&stem)
        || (content.contains(&b'%')
            && percent_encoding::percent_decode(content)
                .decode_utf8_lossy()
                .to_lowercase()
                .contains(&stem))
}

/// The line a link was written on, cut down to show it.
///
/// `start` and `end` are the 1-based columns, in characters, of the first
/// and last character of the link. The indentation goes; a line longer than
/// [`EXCERPT_CHARS`] keeps a window that starts a little before the link.
pub fn excerpt(line: &str, start: usize, end: usize) -> Excerpt {
    let chars: Vec<char> = line.chars().collect();
    let first = chars
        .iter()
        .position(|c| !c.is_whitespace())
        .unwrap_or(chars.len());
    let last = chars.len()
        - chars
            .iter()
            .rev()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);
    let mark_start = start.saturating_sub(1).clamp(first, last);
    let mark_end = end.clamp(mark_start, last);

    let (from, to) = if last - first <= EXCERPT_CHARS {
        (first, last)
    } else {
        let to =
            (mark_start.saturating_sub(EXCERPT_LEAD_CHARS).max(first) + EXCERPT_CHARS).min(last);
        (to - EXCERPT_CHARS, to)
    };
    let mark_start = mark_start.clamp(from, to);
    let mark_end = mark_end.clamp(mark_start, to);

    let slice = |range: Range<usize>| chars[range].iter().collect::<String>();
    let mut text = String::new();
    if from > first {
        text.push('…');
    }
    text.push_str(&slice(from..mark_start));
    let mark_from = text.len();
    text.push_str(&slice(mark_start..mark_end));
    let mark = mark_from..text.len();
    text.push_str(&slice(mark_end..to));
    if to < last {
        text.push('…');
    }
    Excerpt { text, mark }
}

/// The lines of `content`, ended the way the Markdown pipeline ends them: by
/// `\n`, `\r\n` or a lone `\r`.
fn lines(content: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = content;
    while let Some(at) = rest.find(['\n', '\r']) {
        lines.push(&rest[..at]);
        let skip = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
        rest = &rest[at + skip..];
    }
    lines.push(rest);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    fn key(path: &Path) -> PathBuf {
        crate::roots::canonical_key(path)
    }

    /// `docs/guide.md`, `docs/api/auth.md`, `my notes.md` and `README.md`.
    fn tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("docs/api")).unwrap();
        fs::write(root.join("docs/guide.md"), "# Guide\n").unwrap();
        fs::write(root.join("docs/api/auth.md"), "# Auth\n").unwrap();
        fs::write(root.join("my notes.md"), "# Notes\n").unwrap();
        fs::write(root.join("README.md"), "# Readme\n").unwrap();
        dir
    }

    #[test]
    fn a_link_resolves_from_the_folder_of_its_source() {
        let dir = tree();
        let docs = dir.path().join("docs");

        assert_eq!(
            resolve(&docs, "./guide.md"),
            Some(key(&docs.join("guide.md")))
        );
        assert_eq!(
            resolve(&docs, "api/auth.md"),
            Some(key(&docs.join("api/auth.md")))
        );
        assert_eq!(
            resolve(&docs.join("api"), "../../README.md"),
            Some(key(&dir.path().join("README.md")))
        );
    }

    #[test]
    fn a_link_to_nothing_resolves_to_nothing() {
        let dir = tree();
        assert_eq!(resolve(dir.path(), "missing.md"), None);
        assert_eq!(resolve(dir.path(), "docs"), None);
    }

    #[test]
    fn a_percent_encoded_link_names_the_file_it_spells() {
        let dir = tree();
        assert_eq!(
            resolve(dir.path(), "my%20notes.md"),
            Some(key(&dir.path().join("my notes.md")))
        );
    }

    #[test]
    fn an_absolute_path_is_its_own_answer() {
        let dir = tree();
        let readme = dir.path().join("README.md");
        assert_eq!(
            resolve(Path::new("/elsewhere"), &readme.to_string_lossy()),
            Some(key(&readme))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_through_a_symlink_lands_on_the_file_it_points_at() {
        let dir = tree();
        std::os::unix::fs::symlink(
            dir.path().join("docs/guide.md"),
            dir.path().join("alias.md"),
        )
        .unwrap();

        assert_eq!(
            resolve(dir.path(), "alias.md"),
            Some(key(&dir.path().join("docs/guide.md")))
        );
    }

    #[test]
    fn the_links_of_a_source_carry_their_line_and_what_it_says() {
        let dir = tree();
        let source = dir.path().join("docs/api/auth.md");
        let content = indoc! {"
            # Auth

            Start with [the guide](../guide.md), then [[../../README]].
            See [elsewhere](https://example.com) and [nothing](missing.md).
        "};

        let found = links_of(&source, content, &RenderOptions::default());

        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].target, key(&dir.path().join("docs/guide.md")));
        assert_eq!(found[0].line, 3);
        assert_eq!(found[0].excerpt.parts().1, "[the guide](../guide.md)");
        assert_eq!(found[1].target, key(&dir.path().join("README.md")));
        assert_eq!(found[1].excerpt.parts().1, "[[../../README]]");
    }

    #[test]
    fn a_file_url_resolves_like_a_path() {
        let dir = tree();
        let guide = dir.path().join("docs/guide.md");
        // A Windows path has no leading slash and uses backslashes, so the URL
        // is spelled out rather than built by prefixing `file://`.
        let path = guide.to_string_lossy().replace('\\', "/");
        let content = format!("[guide](file:///{})\n", path.trim_start_matches('/'));

        let found = links_of(
            &dir.path().join("README.md"),
            &content,
            &RenderOptions::default(),
        );

        assert_eq!(found[0].target, key(&guide));
    }

    #[test]
    fn a_wiki_link_without_an_extension_finds_its_document() {
        let dir = tree();
        let found = links_of(
            &dir.path().join("docs/index.md"),
            "Read [[guide]].\n",
            &RenderOptions::default(),
        );
        assert_eq!(found[0].target, key(&dir.path().join("docs/guide.md")));
    }

    #[test]
    fn a_mention_ignores_case_and_percent_encoding() {
        assert!(mentions(b"see [x](./Guide.md)", "guide"));
        assert!(mentions(b"see [x](my%20notes.md)", "my notes"));
        assert!(mentions(b"see [x](g%75ide.md)", "guide"));
        assert!(mentions(b"see [x](caf%C3%A9.md)", "caf\u{e9}"));
        assert!(!mentions(b"nothing to see here", "guide"));
    }

    #[test]
    fn an_excerpt_drops_the_indentation_and_marks_the_link() {
        let found = excerpt("    - see [a](a.md) now", 11, 19);
        assert_eq!(found.text, "- see [a](a.md) now");
        assert_eq!(found.parts(), ("- see ", "[a](a.md)", " now"));
    }

    #[test]
    fn a_long_line_keeps_a_window_around_the_link() {
        let line = format!("{} [a](a.md) {}", "x".repeat(200), "y".repeat(200));
        let found = excerpt(&line, 202, 210);

        assert!(found.text.starts_with('…'), "{}", found.text);
        assert!(found.text.ends_with('…'), "{}", found.text);
        assert_eq!(found.parts().1, "[a](a.md)");
        assert!(found.text.chars().count() <= EXCERPT_CHARS + 2);
    }

    #[test]
    fn an_excerpt_counts_characters_rather_than_bytes() {
        let found = excerpt("日本語の[説明](a.md)です", 5, 14);
        assert_eq!(found.parts(), ("日本語の", "[説明](a.md)", "です"));
    }

    #[test]
    fn lines_end_however_the_file_ends_them() {
        assert_eq!(lines("a\nb\r\nc\rd"), vec!["a", "b", "c", "d"]);
    }
}
