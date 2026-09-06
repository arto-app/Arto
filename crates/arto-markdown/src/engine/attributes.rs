//! The `{#id .class}` block a heading may end with.
//!
//! ox-content does not implement the Pandoc-style attribute syntax, so the
//! block arrives as ordinary text at the end of the heading. It always lands
//! in one `Text` node — `{` and `}` are not inline markup — which is what
//! lets [`super::hooks`] lift it out of the heading and onto the tag.
//! Delete this module if ox-content grows the syntax itself.

/// What a heading's trailing `{…}` block asked for.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Attributes<'a> {
    pub id: Option<&'a str>,
    pub classes: Vec<&'a str>,
}

/// Split a trailing attribute block off `text`.
///
/// Returns the text without the block and what the block asked for, or
/// `None` when the text does not end in one. A block that holds nothing
/// usable (`{}`, `{ }`, or only words that are neither `#id` nor `.class`)
/// is left alone: it is more likely prose than markup.
pub(super) fn split_trailing(text: &str) -> Option<(&str, Attributes<'_>)> {
    let trimmed = text.trim_end();
    let inner_end = trimmed.strip_suffix('}')?.len();
    let open = trimmed.rfind('{')?;
    // A brace on the far side of a closing one belongs to another block.
    if trimmed[open + 1..inner_end].contains('}') {
        return None;
    }

    let mut attributes = Attributes::default();
    for token in trimmed[open + 1..inner_end].split_whitespace() {
        match token.split_at_checked(1) {
            Some(("#", id)) if !id.is_empty() => attributes.id = Some(id),
            Some((".", class)) if !class.is_empty() => attributes.classes.push(class),
            _ => return None,
        }
    }

    if attributes.id.is_none() && attributes.classes.is_empty() {
        return None;
    }
    Some((trimmed[..open].trim_end(), attributes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_lifted_off_the_text() {
        let (text, attributes) = split_trailing("Custom identifier {#custom-id}").unwrap();
        assert_eq!(text, "Custom identifier");
        assert_eq!(attributes.id, Some("custom-id"));
        assert!(attributes.classes.is_empty());
    }

    #[test]
    fn classes_come_along_in_order() {
        let (text, attributes) = split_trailing("Title {#the-id .one .two}").unwrap();
        assert_eq!(text, "Title");
        assert_eq!(attributes.id, Some("the-id"));
        assert_eq!(attributes.classes, ["one", "two"]);
    }

    #[test]
    fn a_block_of_only_classes_is_still_a_block() {
        let (text, attributes) = split_trailing("Title {.highlight}").unwrap();
        assert_eq!(text, "Title");
        assert_eq!(attributes.id, None);
        assert_eq!(attributes.classes, ["highlight"]);
    }

    #[test]
    fn prose_in_braces_stays_prose() {
        assert_eq!(split_trailing("What {this means}"), None);
        assert_eq!(split_trailing("Empty {}"), None);
        assert_eq!(split_trailing("Blank { }"), None);
        assert_eq!(split_trailing("Mixed {#id and words}"), None);
        assert_eq!(split_trailing("No block here"), None);
        assert_eq!(split_trailing("Bare markers {# .}"), None);
    }

    #[test]
    fn only_the_last_block_is_taken() {
        let (text, attributes) = split_trailing("A {#first} B {#second}").unwrap();
        assert_eq!(text, "A {#first} B");
        assert_eq!(attributes.id, Some("second"));
    }

    #[test]
    fn trailing_whitespace_does_not_hide_the_block() {
        let (text, attributes) = split_trailing("Title {#id}   ").unwrap();
        assert_eq!(text, "Title");
        assert_eq!(attributes.id, Some("id"));
    }
}
