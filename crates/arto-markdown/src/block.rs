//! One block at a time: the source range a rendered block names, and the
//! content of that block read back out of the source.
//!
//! A range covers the block as written, so it carries the syntax around the
//! content: a heading's `#`, a list item's marker, and on every line after
//! the first the quote markers and indentation of the containers the block
//! sits in. [`block_content`] takes those off, leaving the Markdown a reader
//! would call the block's text — what a translation or a summary is about.
//! It works on the text alone, which is why the caller says what kind of
//! block the range names: the rendered element knows, the range does not.

use crate::line_endings;
use crate::options::RenderOptions;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A position in the file: 1-based line and 1-based column in code points,
/// written `L:C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// An inclusive range of positions, written `L:C-L:C` in
/// `data-source-range`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

/// Why a position or a range was not read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a source position or range: {0:?}")]
pub struct InvalidSourceRange(String);

impl fmt::Display for SourcePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

impl FromStr for SourcePosition {
    type Err = InvalidSourceRange;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parse = || -> Option<Self> {
            let (line, column) = value.split_once(':')?;
            let position = Self {
                line: line.parse().ok()?,
                column: column.parse().ok()?,
            };
            (position.line >= 1 && position.column >= 1).then_some(position)
        };
        parse().ok_or_else(|| InvalidSourceRange(value.to_string()))
    }
}

impl fmt::Display for SourceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl FromStr for SourceRange {
    type Err = InvalidSourceRange;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidSourceRange(value.to_string());
        let (start, end) = value.split_once('-').ok_or_else(invalid)?;
        let range = Self {
            start: start.parse().map_err(|_| invalid())?,
            end: end.parse().map_err(|_| invalid())?,
        };
        if range.start > range.end {
            return Err(invalid());
        }
        Ok(range)
    }
}

macro_rules! string_form {
    ($type:ty) => {
        impl TryFrom<String> for $type {
            type Error = InvalidSourceRange;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                value.parse()
            }
        }

        impl From<$type> for String {
            fn from(value: $type) -> Self {
                value.to_string()
            }
        }
    };
}

string_form!(SourcePosition);
string_form!(SourceRange);

/// What a rendered block is, which decides the syntax around its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockKind {
    Paragraph,
    Heading,
    ListItem,
    TableCell,
    DefinitionTerm,
    Definition,
}

/// The content of the `kind` block at `range` in `markdown`, the whole file
/// the range was rendered from.
///
/// `until` ends the block early, before the position given: a list item
/// whose text is followed by a nested list or code block names the item's
/// whole range, and only the text before the first of those is its own.
/// `options` are the ones the file was rendered with, because they decide
/// what counts as syntax — a heading's `{#id}` block is text to a parser
/// that does not read attributes.
///
/// `None` when the range lies outside the file or holds nothing but syntax.
pub fn block_content(
    markdown: &str,
    kind: BlockKind,
    range: &SourceRange,
    until: Option<SourcePosition>,
    options: &RenderOptions,
) -> Option<String> {
    let markdown = line_endings::normalize(markdown);
    let text = slice(&markdown, range, until)?;
    let mut lines: Vec<&str> = text.lines().collect();
    // An ATX heading is one line, so a heading over several is a setext
    // one, whose last line is its underline — however much container syntax
    // stands in front of it.
    let setext = kind == BlockKind::Heading && lines.len() > 1;

    let first = *lines.first()?;
    let content = match kind {
        BlockKind::ListItem => strip_task_marker(strip_list_marker(first)),
        BlockKind::Definition => first.trim_start_matches(':').trim_start(),
        BlockKind::Heading if !setext => first.trim_start_matches('#').trim_start(),
        _ => first,
    };
    let content_column = range.start.column + first.chars().count() - content.chars().count();
    lines[0] = content;

    // Every later line carries the markers and indentation of the
    // containers, up to the column the content started at.
    for line in lines.iter_mut().skip(1) {
        *line = strip_prefix(line, content_column - 1);
    }

    if setext {
        lines.pop();
    } else if kind == BlockKind::Heading {
        if let Some(last) = lines.last_mut() {
            *last = strip_closing_hashes(last);
            if options.heading_attributes {
                *last = strip_attribute_block(last);
            }
        }
    }

    let content = lines.join("\n");
    let content = content.trim();
    (!content.is_empty()).then(|| content.to_string())
}

/// The text of `markdown` that `range` covers, as written: container syntax
/// and all. `None` when the file has no such range.
pub fn source_text(markdown: &str, range: &SourceRange) -> Option<String> {
    let markdown = line_endings::normalize(markdown);
    slice(&markdown, range, None).map(str::to_string)
}

/// The text of `markdown` from `range.start` through `range.end`, or up to
/// `until` when that comes first.
fn slice<'a>(
    markdown: &'a str,
    range: &SourceRange,
    until: Option<SourcePosition>,
) -> Option<&'a str> {
    let start = offset(markdown, range.start)?;
    let end = match until {
        Some(until) if until <= range.end => offset(markdown, until)?,
        _ => {
            let last = offset(markdown, range.end)?;
            last + markdown[last..].chars().next()?.len_utf8()
        }
    };
    markdown.get(start..end.max(start))
}

/// Byte offset of `position`, or `None` when the file has no such character.
fn offset(markdown: &str, position: SourcePosition) -> Option<usize> {
    let line_start: usize = markdown
        .split_inclusive('\n')
        .take(position.line.checked_sub(1)?)
        .map(str::len)
        .sum();
    let line = markdown[line_start..].split('\n').next()?;
    let (index, _) = line.char_indices().nth(position.column.checked_sub(1)?)?;
    Some(line_start + index)
}

/// `line` without up to `width` leading characters of container syntax.
fn strip_prefix(line: &str, width: usize) -> &str {
    let mut rest = line;
    for _ in 0..width {
        match rest.strip_prefix([' ', '\t', '>']) {
            Some(next) => rest = next,
            None => break,
        }
    }
    rest
}

/// `line` without a bullet (`-`, `*`, `+`) or ordinal (`1.`, `1)`) marker.
fn strip_list_marker(line: &str) -> &str {
    let rest = line
        .strip_prefix(['-', '*', '+'])
        .or_else(|| {
            let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            (1..=9)
                .contains(&digits)
                .then(|| line[digits..].strip_prefix(['.', ')']))
                .flatten()
        })
        .unwrap_or(line);
    rest.trim_start()
}

/// `line` without a task list checkbox.
fn strip_task_marker(line: &str) -> &str {
    ["[ ]", "[x]", "[X]"]
        .iter()
        .find_map(|marker| line.strip_prefix(marker))
        .filter(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
        .map_or(line, str::trim_start)
}

/// `line` without the optional closing sequence of an ATX heading.
fn strip_closing_hashes(line: &str) -> &str {
    let trimmed = line.trim_end();
    let without = trimmed.trim_end_matches('#');
    if without.len() == trimmed.len() {
        return line;
    }
    if without.is_empty() || without.ends_with([' ', '\t']) {
        without.trim_end()
    } else {
        line
    }
}

/// `line` without a trailing `{#id .class}` block. Braces holding anything
/// else are text: `# What {this means}` keeps them.
fn strip_attribute_block(line: &str) -> &str {
    let trimmed = line.trim_end();
    let Some(body) = trimmed.strip_suffix('}') else {
        return line;
    };
    let Some(open) = body.rfind('{') else {
        return line;
    };
    let tokens: Vec<&str> = body[open + 1..].split_whitespace().collect();
    let is_attributes = !tokens.is_empty()
        && tokens
            .iter()
            .all(|token| token.len() > 1 && token.starts_with(['#', '.']));
    if is_attributes {
        body[..open].trim_end()
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closing_sequence_needs_a_space_before_it() {
        assert_eq!(strip_closing_hashes("Closed ##"), "Closed");
        assert_eq!(strip_closing_hashes("C#"), "C#");
        assert_eq!(strip_closing_hashes("Plain"), "Plain");
    }

    #[test]
    fn a_marker_needs_room_after_it() {
        assert_eq!(strip_list_marker("- one"), "one");
        assert_eq!(strip_list_marker("12) twelve"), "twelve");
        assert_eq!(strip_task_marker("[x]done"), "[x]done");
    }

    #[test]
    fn only_the_container_width_is_taken_from_a_later_line() {
        assert_eq!(strip_prefix(">   indented", 2), "  indented");
        assert_eq!(strip_prefix("lazy", 2), "lazy");
    }
}
