//! Turn the renderer's HTML into the crate's HTML contract.
//!
//! Two things separate what ox-content writes from what the frontend reads,
//! and both are attribute-level edits over the finished document, so one
//! lol_html pass does them together:
//!
//! * **Source ranges.** The renderer marks every block element with
//!   `data-source-span="S-E"`, byte offsets into the body; the contract is
//!   `data-source-range="L:C-L:C"` in lines and columns of the whole file, in the
//!   same place. A code block's `<code>` gets the range of its content,
//!   which the renderer does not mark at all.
//! * **Callouts.** `> [!NOTE]` is rendered as `<blockquote class="ox-callout
//!   ox-callout--note">` with a plain title; GitHub — and therefore
//!   the frontend stylesheet, which styles the rendered page — uses
//!   `<div class="markdown-alert markdown-alert-note">` with the icon
//!   placeholder the frontend fills in.
//!
//! Heading ids are collected here as well: the renderer derives them, so
//! reading them back off the rendered headings is what keeps the table of
//! contents and the anchors in agreement. Headings inside the trailing
//! `<section class="footnotes">` are left out, because the renderer moved
//! them there out of document order and the outline lists nothing from a
//! footnote definition.

use super::lines::{LineTable, SourceRange};
use lol_html::html_content::ContentType;
use lol_html::{element, EndTagHandler, HtmlRewriter, Settings};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

/// Annotated HTML plus the `id` of each rendered heading, in document order.
pub(super) struct Annotated {
    pub html: String,
    pub heading_ids: Vec<String>,
}

#[derive(Default)]
struct State {
    /// The kind of the callout that is currently open, for the title that
    /// follows the opening tag.
    callout_kind: Option<String>,
    /// Set while a callout is waiting for its body: the first paragraph's
    /// span starts at the `[!NOTE]` marker, but its range starts where the
    /// body text does. Cleared when the callout closes, so a
    /// callout that has no body paragraph cannot shift the paragraph that
    /// follows it.
    callout_body_pending: bool,
    /// The content range of the code block that is open, for the `<code>`
    /// that follows its `<pre>`.
    code_range: Option<SourceRange>,
    /// Set inside the trailing footnotes section, whose headings are not
    /// part of the outline.
    in_footnotes: bool,
    heading_ids: Vec<String>,
}

/// Parse `S-E` into byte offsets.
fn parse_span(value: &str) -> Option<(usize, usize)> {
    let (start, end) = value.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?))
}

/// Offset of the first body character after a `[!KIND]` callout marker.
///
/// The marker opens the paragraph and closes on the same line, so anything
/// else — the body of a callout whose marker line stood alone — keeps the
/// offset it came with. Searching further would find the `]` of a link on a
/// later line of the same paragraph and report that line instead.
fn callout_body_start(body: &str, start: usize) -> usize {
    let start = start.min(body.len());
    let rest = &body[start..];
    if !rest.starts_with("[!") {
        return start;
    }
    let marker_line = rest.split('\n').next().unwrap_or(rest);
    let Some(marker_end) = marker_line.find(']') else {
        return start;
    };
    let after = &rest[marker_end + 1..];
    let same_line = after.len() - after.trim_start_matches([' ', '\t']).len();
    let body = &after[same_line..];
    // A body on a later line starts after the quote markers and blank lines
    // in between; one on the marker's own line starts where it stands, even
    // on a `>` of its own.
    let skipped = if body.starts_with(['\r', '\n']) {
        body.len()
            - body
                .trim_start_matches(|c: char| c.is_whitespace() || c == '>')
                .len()
    } else {
        0
    };
    start + marker_end + 1 + same_line + skipped
}

/// The uppercase name the alert title shows for a callout class suffix.
fn alert_name(kind: &str) -> String {
    kind.to_uppercase()
}

/// Marker [`super::hooks`] puts on a heading whose id the document wrote.
const AUTHORED_ID: &str = "data-arto-authored-id";

/// Rewrite the engine's HTML into the crate's contract.
///
/// Heading ids are dropped unless `keep_heading_ids` is set; either way they
/// are reported in [`Annotated::heading_ids`].
///
/// `code_contents` maps the start of each code block's span to the bytes of
/// its content (see [`super::code`]).
pub(super) fn annotate(
    html: &str,
    lines: &LineTable<'_>,
    code_contents: &HashMap<usize, Range<usize>>,
    keep_heading_ids: bool,
) -> Annotated {
    // `Rc` because the end-tag handlers that close a scope have to own their
    // share of the state: lol_html requires them to be `'static`.
    let state = Rc::new(RefCell::new(State::default()));
    let mut output = Vec::new();

    // The span handler is registered first so that the class and `dir` the
    // callout handler adds afterwards land around the range in the order
    // the contract documents.
    let settings = Settings::new()
        // Raw HTML in the document may carry a `data-source-range` of its own; the
        // attribute is the pipeline's, so only the ranges written below stay.
        .append_element_content_handler(element!("[data-source-range]", |el| {
            el.remove_attribute("data-source-range");
            Ok(())
        }))
        .append_element_content_handler(element!("[data-source-span]", |el| {
            let attrs: Vec<(String, String)> = el
                .attributes()
                .iter()
                .map(|attr| (attr.name(), attr.value()))
                .collect();
            let span = attrs
                .iter()
                .find(|(name, _)| name == "data-source-span")
                .and_then(|(_, value)| parse_span(value));
            let class = attrs
                .iter()
                .find(|(name, _)| name == "class")
                .map(|(_, value)| value.as_str())
                .unwrap_or_default();

            let tag = el.tag_name();
            let is_heading = matches!(tag.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
            // An id the document asked for by name is content, so it stays
            // even when the generated ones are dropped.
            let authored_id = attrs.iter().any(|(name, _)| name == AUTHORED_ID);
            let body_pending = std::mem::take(&mut state.borrow_mut().callout_body_pending);

            let range = span.and_then(|(start, end)| {
                // The body of a callout starts after the `[!KIND]` marker
                // the renderer stripped from the text.
                let start = if tag == "p" && body_pending {
                    callout_body_start(lines.body(), start)
                } else {
                    start
                };
                lines.range(start, end)
            });
            if tag == "pre" && !class.contains("preprocessed-") {
                state.borrow_mut().code_range = span
                    .and_then(|(start, _)| code_contents.get(&start))
                    .and_then(|content| lines.range_from(content.start, content.end));
            }

            // The renderer writes a heading's id ahead of the span; the
            // contract puts the source range first, so the id moves behind
            // it (and is dropped without a table of contents, though it is
            // always reported).
            let heading_id = is_heading.then(|| {
                let id = attrs
                    .iter()
                    .find(|(name, _)| name == "id")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                let mut state = state.borrow_mut();
                if !state.in_footnotes {
                    state.heading_ids.push(id.clone());
                }
                id
            });

            // Rebuild the attribute list so the range takes the span's
            // position instead of being appended at the end.
            for (name, _) in &attrs {
                el.remove_attribute(name);
            }
            for (name, value) in &attrs {
                match name.as_str() {
                    "data-source-span" => {
                        if let Some(range) = range {
                            el.set_attribute("data-source-range", &range.to_string())?;
                        }
                        if keep_heading_ids || authored_id {
                            if let Some(id) = &heading_id {
                                el.set_attribute("id", id)?;
                            }
                        }
                    }
                    "id" if is_heading => {}
                    AUTHORED_ID => {}
                    _ => el.set_attribute(name, value)?,
                }
            }
            Ok(())
        }))
        // A raw `<pre><code>` in the document takes nothing: every `<pre>`
        // the renderer wrote set the range, and its `<code>` consumed it.
        .append_element_content_handler(element!("pre > code", |el| {
            let Some(range) = state.borrow_mut().code_range.take() else {
                return Ok(());
            };
            let attrs: Vec<(String, String)> = el
                .attributes()
                .iter()
                .map(|attr| (attr.name(), attr.value()))
                .collect();
            for (name, _) in &attrs {
                el.remove_attribute(name);
            }
            el.set_attribute("data-source-range", &range.to_string())?;
            for (name, value) in &attrs {
                el.set_attribute(name, value)?;
            }
            Ok(())
        }))
        // The renderer appends the footnote bodies here, out of document
        // order, so their headings are not part of the outline.
        .append_element_content_handler(element!("section.footnotes", |el| {
            state.borrow_mut().in_footnotes = true;
            let closing = Rc::clone(&state);
            let close: EndTagHandler<'static> = Box::new(move |_| {
                closing.borrow_mut().in_footnotes = false;
                Ok(())
            });
            let _ = el.on_end_tag(close);
            Ok(())
        }))
        .append_element_content_handler(element!("blockquote.ox-callout", |el| {
            let Some(kind) = el.get_attribute("class").and_then(|class| {
                class
                    .split(' ')
                    .find_map(|name| name.strip_prefix("ox-callout--"))
                    .map(str::to_string)
            }) else {
                return Ok(());
            };
            el.set_tag_name("div")?;
            el.set_attribute("class", &format!("markdown-alert markdown-alert-{kind}"))?;
            el.set_attribute("dir", "auto")?;
            // A callout whose only line is the marker has no body paragraph
            // to take the flag, so it is dropped when the callout closes
            // rather than shifting the next paragraph in the document.
            let closing = Rc::clone(&state);
            let close: EndTagHandler<'static> = Box::new(move |_| {
                closing.borrow_mut().callout_body_pending = false;
                Ok(())
            });
            let _ = el.on_end_tag(close);
            let mut state = state.borrow_mut();
            state.callout_kind = Some(kind);
            state.callout_body_pending = true;
            Ok(())
        }))
        // Nothing styles definition lists by class, and a class named after
        // the engine would outlive it, so the renderer's marker comes off.
        .append_element_content_handler(element!("dl.ox-definition-list", |el| {
            el.remove_attribute("class");
            Ok(())
        }))
        .append_element_content_handler(element!("p.ox-callout-title", |el| {
            let kind = state.borrow().callout_kind.clone().unwrap_or_default();
            el.set_attribute("class", "markdown-alert-title")?;
            el.set_attribute("dir", "auto")?;
            el.set_inner_content(
                &format!(
                    r#"<span class="alert-icon" data-alert-type="{kind}"></span>{}"#,
                    alert_name(&kind)
                ),
                ContentType::Html,
            );
            Ok(())
        }));

    let mut rewriter = HtmlRewriter::new(settings, |chunk: &[u8]| {
        output.extend_from_slice(chunk);
    });
    let written = rewriter.write(html.as_bytes());
    let ended = rewriter.end();

    if let Err(error) = written.and(ended) {
        // A rewrite that stopped part way through leaves `output` holding
        // the document up to that point, and the ids of only the headings
        // it reached. Showing the whole document with the engine's own
        // attributes still on it loses the source lines and the alert
        // styling, but it loses no content — and an empty outline is better
        // than one whose entries point at the wrong headings.
        tracing::debug!(%error, "annotation failed; rendering the unannotated HTML");
        return Annotated {
            html: html.to_string(),
            heading_ids: Vec::new(),
        };
    }

    let heading_ids = std::mem::take(&mut state.borrow_mut().heading_ids);
    Annotated {
        html: String::from_utf8(output).unwrap_or_else(|_| html.to_string()),
        heading_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Annotate `html` rendered from `body`, whose code blocks have the
    /// content ranges `code`.
    fn run(html: &str, body: &str, code: &[(usize, Range<usize>)]) -> Annotated {
        let code: HashMap<_, _> = code.iter().cloned().collect();
        annotate(html, &LineTable::new(body, 0), &code, false)
    }

    #[test]
    fn a_span_becomes_a_range_in_its_place() {
        let annotated = run(
            r#"<p data-source-span="5-7" class="x">hi</p>"#,
            "# T\n\nhi\n",
            &[],
        );
        assert_eq!(
            annotated.html,
            r#"<p data-source-range="3:1-3:2" class="x">hi</p>"#
        );
    }

    #[test]
    fn a_code_block_hands_its_content_to_the_code_element() {
        let html = r#"<pre data-source-span="5-18"><code class="language-rust">x
</code></pre>"#;
        let annotated = run(html, "# T\n\n```rust\nx\n```\n", &[(5, 13..14)]);
        assert!(
            annotated.html.starts_with(
                r#"<pre data-source-range="3:1-5:3"><code data-source-range="4:1-4:1" class="language-rust">"#
            ),
            "{}",
            annotated.html
        );
    }

    #[test]
    fn a_code_block_without_content_leaves_its_code_element_bare() {
        let html = r#"<pre data-source-span="0-8"><code></code></pre>"#;
        let annotated = run(html, "```\n```\n", &[]);
        assert_eq!(
            annotated.html,
            r#"<pre data-source-range="1:1-2:3"><code></code></pre>"#
        );
    }

    #[test]
    fn a_code_element_the_renderer_did_not_write_takes_no_range() {
        let html = r#"<pre><code>x</code></pre>"#;
        assert_eq!(run(html, "x", &[]).html, html);
    }

    #[test]
    fn a_range_the_document_wrote_itself_is_dropped() {
        let html = concat!(
            r#"<div data-source-range="0:0-0:0">x</div>"#,
            r#"<p data-source-range="9:9-9:9" data-source-span="0-1">y</p>"#,
        );
        assert_eq!(
            run(html, "y", &[]).html,
            r#"<div>x</div><p data-source-range="1:1-1:1">y</p>"#
        );
    }

    #[test]
    fn table_cells_name_their_content_without_padding() {
        let html = concat!(
            r#"<table data-source-span="0-18"><tr data-source-span="0-5">"#,
            r#"<th data-source-span="1-4">x</th></tr></table>"#,
        );
        assert_eq!(
            run(html, "| x |\n| - |\n| 1 |\n", &[]).html,
            concat!(
                r#"<table data-source-range="1:1-3:5"><tr data-source-range="1:1-1:5">"#,
                r#"<th data-source-range="1:3-1:3">x</th></tr></table>"#,
            )
        );
    }

    #[test]
    fn containers_keep_their_content_attribute_untouched() {
        let html = concat!(
            r#"<pre data-source-span="0-20" class="preprocessed-mermaid" "#,
            r#"data-original-content="A--&gt;B">A--&gt;B</pre>"#,
        );
        assert_eq!(
            run(html, "```mermaid\nA-->B\n```\n", &[(0, 11..16)]).html,
            concat!(
                r#"<pre data-source-range="1:1-3:3" class="preprocessed-mermaid" "#,
                r#"data-original-content="A--&gt;B">A--&gt;B</pre>"#,
            )
        );
    }

    #[test]
    fn heading_ids_are_collected_and_optionally_kept() {
        let html = r#"<h1 id="title" data-source-span="0-8">Title</h1>"#;
        let lines = LineTable::new("# Title\n", 0);

        let kept = annotate(html, &lines, &HashMap::new(), true);
        assert_eq!(kept.heading_ids, vec!["title".to_string()]);
        assert_eq!(
            kept.html,
            r#"<h1 data-source-range="1:1-1:7" id="title">Title</h1>"#
        );

        let dropped = annotate(html, &lines, &HashMap::new(), false);
        assert_eq!(dropped.heading_ids, vec!["title".to_string()]);
        assert_eq!(
            dropped.html,
            r#"<h1 data-source-range="1:1-1:7">Title</h1>"#
        );
    }

    #[test]
    fn callouts_take_github_markup_and_the_body_range() {
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--note" data-source-span="0-17">"#,
            r#"<p class="ox-callout-title">Note</p>"#,
            r#"<p data-source-span="2-16">body</p></blockquote>"#,
        );
        assert_eq!(
            run(html, "> [!NOTE]\n> body\n", &[]).html,
            concat!(
                r#"<div class="markdown-alert markdown-alert-note" data-source-range="1:1-2:6" dir="auto">"#,
                r#"<p class="markdown-alert-title" dir="auto">"#,
                r#"<span class="alert-icon" data-alert-type="note"></span>NOTE</p>"#,
                r#"<p data-source-range="2:3-2:6">body</p></div>"#,
            )
        );
    }

    #[test]
    fn a_callout_body_on_the_marker_line_keeps_a_leading_bracket() {
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--note" data-source-span="0-14">"#,
            r#"<p class="ox-callout-title">Note</p>"#,
            r#"<p data-source-span="2-13">&gt; 0</p></blockquote>"#,
        );
        assert!(run(html, "> [!NOTE] > 0\n", &[])
            .html
            .contains(r#"<p data-source-range="1:11-1:13">"#),);
    }

    #[test]
    fn a_callout_without_a_body_paragraph_does_not_shift_a_later_one() {
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--tip" data-source-span="0-18">"#,
            r#"<p class="ox-callout-title">Tip</p>"#,
            r#"<ul data-source-span="11-18"><li data-source-span="11-18">"#,
            r#"<p data-source-span="13-18">item</p></li></ul></blockquote>"#,
        );
        let annotated = run(html, "> [!TIP]\n> - item\n", &[]);
        assert!(
            annotated
                .html
                .contains(r#"<p data-source-range="2:5-2:8">item</p>"#),
            "{}",
            annotated.html
        );
    }

    #[test]
    fn a_callout_body_paragraph_of_its_own_keeps_its_start() {
        // The marker stands alone, so the body paragraph starts where its
        // span says; a `]` further down the paragraph is body text.
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--note" data-source-span="0-41">"#,
            r#"<p class="ox-callout-title">Note</p>"#,
            r#"<p data-source-span="14-40">first line
second ] line</p></blockquote>"#,
        );
        let annotated = run(html, "> [!NOTE]\n>\n> first line\n> second ] line\n", &[]);
        assert!(
            annotated
                .html
                .contains(r#"<p data-source-range="3:3-4:15">"#),
            "{}",
            annotated.html
        );
    }

    #[test]
    fn every_span_becomes_a_range_and_a_malformed_one_is_dropped() {
        let html = r#"<section data-source-span="0-1">x</section><p data-source-span="nope">y</p>"#;
        assert_eq!(
            run(html, "x", &[]).html,
            r#"<section data-source-range="1:1-1:1">x</section><p>y</p>"#
        );
    }
}
