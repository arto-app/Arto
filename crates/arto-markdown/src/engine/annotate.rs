//! Turn the renderer's HTML into the crate's HTML contract.
//!
//! Two things separate what ox-content writes from what the frontend reads,
//! and both are attribute-level edits over the finished document, so one
//! lol_html pass does them together:
//!
//! * **Source positions.** The renderer marks every block element with
//!   `data-source-span="S-E"`, byte offsets into the body. The contract is
//!   line based and differs per element: paragraphs, headings, quotes, rules
//!   and table rows carry the start line; lists, items, tables and the
//!   Mermaid and math containers also carry the end line; code blocks
//!   additionally carry the line their content starts on. Elements the
//!   frontend does not track just lose the attribute.
//! * **Callouts.** `> [!NOTE]` is rendered as `<blockquote class="ox-callout
//!   ox-callout--note">` with a plain title; GitHub — and therefore
//!   `github-markdown-css`, which styles the rendered page — uses
//!   `<div class="markdown-alert markdown-alert-note">` with the icon
//!   placeholder the frontend fills in.
//!
//! Heading ids are collected here as well: the renderer derives them, so
//! reading them back off the rendered headings is what keeps the table of
//! contents and the anchors in agreement. Headings inside the trailing
//! `<section class="footnotes">` are left out, because the renderer moved
//! them there out of document order and the outline lists nothing from a
//! footnote definition.

use super::lines::LineTable;
use lol_html::html_content::ContentType;
use lol_html::{element, text, EndTagHandler, HtmlRewriter, Settings};
use std::cell::RefCell;
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
    /// span starts at the `[!NOTE]` marker, but the frontend wants the line
    /// the body text starts on. Cleared when the callout closes, so a
    /// callout that has no body paragraph cannot shift the paragraph that
    /// follows it.
    callout_body_pending: bool,
    /// Set inside the trailing footnotes section, whose headings are not
    /// part of the outline.
    in_footnotes: bool,
    heading_ids: Vec<String>,
    /// How many elements deep the text being read is quoted verbatim.
    verbatim_depth: usize,
    /// The text node being collected, which lol_html may hand over in
    /// several chunks; a wiki link can straddle the boundary.
    text: String,
}

/// Parse `S-E` into byte offsets.
fn parse_span(value: &str) -> Option<(usize, usize)> {
    let (start, end) = value.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?))
}

/// Whether a code block's source starts with a fence rather than
/// indentation. The source may start inside a container, so quote markers
/// and indentation are skipped first.
fn is_fenced(source: &str) -> bool {
    let trimmed = source.trim_start_matches([' ', '\t', '>']);
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
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
    start + marker_end + 1 + (after.len() - after.trim_start().len())
}

/// The line attributes an element with the given tag and class carries.
fn line_attributes(
    tag: &str,
    class: &str,
    start: usize,
    end: usize,
    start_line: usize,
    lines: &LineTable<'_>,
) -> Vec<(&'static str, usize)> {
    let end_line = lines.line_at_end(start, end);
    match tag {
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "hr" | "tr" => {
            vec![("data-source-line", start_line)]
        }
        "ul" | "ol" | "li" | "table" => vec![
            ("data-source-line", start_line),
            ("data-source-line-end", end_line),
        ],
        "pre" | "div" if class.contains("preprocessed-") => vec![
            ("data-source-line", start_line),
            ("data-source-line-end", end_line),
        ],
        "pre" => {
            // Fenced blocks: content starts on the line after the fence.
            // Indented blocks: content starts on the same line.
            let content_start = if is_fenced(lines.slice(start, end)) {
                start_line + 1
            } else {
                start_line
            };
            vec![
                ("data-source-line", start_line),
                ("data-source-line-end", end_line),
                ("data-source-line-start", content_start),
            ]
        }
        _ => Vec::new(),
    }
}

/// The uppercase name the alert title shows for a callout class suffix.
fn alert_name(kind: &str) -> String {
    kind.to_uppercase()
}

/// Marker [`super::hooks`] puts on a heading whose id the document wrote.
const AUTHORED_ID: &str = "data-arto-authored-id";

/// Elements whose text is quoted verbatim, or is already a link, so that
/// `[[…]]` inside them is not markup.
const VERBATIM_SELECTOR: &str =
    "code, pre, a, script, style, .preprocessed-math-display, .preprocessed-math-inline";

/// Rewrite the engine's HTML into the crate's contract.
///
/// Heading ids are dropped unless `keep_heading_ids` is set; either way they
/// are reported in [`Annotated::heading_ids`].
pub(super) fn annotate(html: &str, lines: &LineTable<'_>, keep_heading_ids: bool) -> Annotated {
    // `Rc` because the end-tag handlers that close a scope have to own their
    // share of the state: lol_html requires them to be `'static`.
    let state = Rc::new(RefCell::new(State::default()));
    let mut output = Vec::new();

    // The span handler is registered first so that it sees a callout while
    // it still is a `<blockquote>` — it carries the start line only, like
    // every other quote — and so that the class and `dir` the callout
    // handler adds afterwards land around the line attribute in the order
    // the contract documents.
    let mut settings = Settings::new()
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

            let line_attrs = span
                .map(|(start, end)| {
                    // The body of a callout starts after the `[!KIND]`
                    // marker the renderer stripped from the text.
                    let start_line = if tag == "p" && body_pending {
                        lines.line_at(callout_body_start(lines.body(), start))
                    } else {
                        lines.line_at(start)
                    };
                    line_attributes(&tag, class, start, end, start_line, lines)
                })
                .unwrap_or_default();

            // The renderer writes a heading's id ahead of the span; the
            // contract puts the source line first, so the id moves behind
            // the line attributes (and is dropped without a table of
            // contents, though it is always reported).
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

            // Rebuild the attribute list so the line attributes take the
            // span's position instead of being appended at the end.
            for (name, _) in &attrs {
                el.remove_attribute(name);
            }
            for (name, value) in &attrs {
                match name.as_str() {
                    "data-source-span" => {
                        for (line_name, line) in &line_attrs {
                            el.set_attribute(line_name, &line.to_string())?;
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

    // Wiki links are read off the rendered text, where the run is contiguous
    // again; see [`super::wiki`]. Documents without a `[[` skip the text
    // handlers entirely, which is nearly all of them.
    if html.contains("[[") {
        settings = settings
            .append_element_content_handler(element!(VERBATIM_SELECTOR, {
                let state = Rc::clone(&state);
                move |el| {
                    state.borrow_mut().verbatim_depth += 1;
                    let closing = Rc::clone(&state);
                    let close: EndTagHandler<'static> = Box::new(move |_| {
                        let mut state = closing.borrow_mut();
                        state.verbatim_depth = state.verbatim_depth.saturating_sub(1);
                        Ok(())
                    });
                    let _ = el.on_end_tag(close);
                    Ok(())
                }
            }))
            .append_element_content_handler(text!("*", {
                let state = Rc::clone(&state);
                move |chunk| {
                    if state.borrow().verbatim_depth > 0 {
                        return Ok(());
                    }
                    state.borrow_mut().text.push_str(chunk.as_str());
                    if !chunk.last_in_text_node() {
                        // Hold the piece back; the whole node is written
                        // when its last chunk arrives.
                        chunk.remove();
                        return Ok(());
                    }
                    let text = std::mem::take(&mut state.borrow_mut().text);
                    // The text is already escaped, so it goes back as HTML
                    // whether or not a link was found in it.
                    let rewritten = super::wiki::rewrite(&text);
                    chunk.replace(rewritten.as_deref().unwrap_or(&text), ContentType::Html);
                    Ok(())
                }
            }));
    }

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

    fn table(body: &str) -> LineTable<'_> {
        LineTable::new(body, 0)
    }

    #[test]
    fn paragraphs_get_their_start_line() {
        let html = r#"<p data-source-span="5-7">hi</p>"#;
        let annotated = annotate(html, &table("# T\n\nhi\n"), false);
        assert_eq!(annotated.html, r#"<p data-source-line="3">hi</p>"#);
    }

    #[test]
    fn code_blocks_get_all_three_attributes() {
        let html = r#"<pre data-source-span="5-18"><code class="language-rust">x
</code></pre>"#;
        let annotated = annotate(html, &table("# T\n\n```rust\nx\n```\n"), false);
        assert!(
            annotated.html.starts_with(
                r#"<pre data-source-line="3" data-source-line-end="5" data-source-line-start="4">"#
            ),
            "{}",
            annotated.html
        );
    }

    #[test]
    fn indented_and_quoted_code_blocks_are_told_apart() {
        let indented = annotate(
            r#"<pre data-source-span="0-6"><code>x
</code></pre>"#,
            &table("    x\n"),
            false,
        );
        assert!(
            indented.html.contains(r#"data-source-line-start="1""#),
            "{}",
            indented.html
        );

        let quoted = annotate(
            r#"<pre data-source-span="0-16"><code>x
</code></pre>"#,
            &table("> ```\n> x\n> ```\n"),
            false,
        );
        assert!(
            quoted.html.contains(r#"data-source-line-start="2""#),
            "{}",
            quoted.html
        );
    }

    #[test]
    fn lists_and_tables_carry_end_lines_and_cells_lose_the_span() {
        let html = concat!(
            r#"<ul data-source-span="0-8">"#,
            r#"<li data-source-span="0-4">a</li></ul>"#,
            r#"<table data-source-span="9-27"><tr data-source-span="9-14">"#,
            r#"<td data-source-span="11-12">x</td></tr></table>"#,
        );
        let annotated = annotate(html, &table("- a\n- b\n\n| x |\n| - |\n| 1 |\n"), false);
        assert!(annotated
            .html
            .contains(r#"<ul data-source-line="1" data-source-line-end="2">"#));
        assert!(annotated
            .html
            .contains(r#"<li data-source-line="1" data-source-line-end="1">"#));
        assert!(annotated
            .html
            .contains(r#"<table data-source-line="4" data-source-line-end="6">"#));
        assert!(annotated.html.contains(r#"<tr data-source-line="4">"#));
        assert!(annotated.html.contains("<td>x</td>"), "{}", annotated.html);
        assert!(!annotated.html.contains("data-source-span"));
    }

    #[test]
    fn containers_keep_their_content_attribute_untouched() {
        let html = concat!(
            r#"<pre data-source-span="0-20" class="preprocessed-mermaid" "#,
            r#"data-original-content="A--&gt;B">A--&gt;B</pre>"#,
        );
        let annotated = annotate(html, &table("```mermaid\nA-->B\n```\n"), false);
        assert_eq!(
            annotated.html,
            concat!(
                r#"<pre data-source-line="1" data-source-line-end="3" class="preprocessed-mermaid" "#,
                r#"data-original-content="A--&gt;B">A--&gt;B</pre>"#,
            )
        );
    }

    #[test]
    fn heading_ids_are_collected_and_optionally_kept() {
        let html = r#"<h1 id="title" data-source-span="0-8">Title</h1>"#;

        let kept = annotate(html, &table("# Title\n"), true);
        assert_eq!(kept.heading_ids, vec!["title".to_string()]);
        assert_eq!(
            kept.html,
            r#"<h1 data-source-line="1" id="title">Title</h1>"#
        );

        let dropped = annotate(html, &table("# Title\n"), false);
        assert_eq!(dropped.heading_ids, vec!["title".to_string()]);
        assert_eq!(dropped.html, r#"<h1 data-source-line="1">Title</h1>"#);
    }

    #[test]
    fn callouts_take_github_markup_and_the_body_line() {
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--note" data-source-span="0-17">"#,
            r#"<p class="ox-callout-title">Note</p>"#,
            r#"<p data-source-span="2-16">body</p></blockquote>"#,
        );
        let annotated = annotate(html, &table("> [!NOTE]\n> body\n"), false);
        assert_eq!(
            annotated.html,
            concat!(
                r#"<div class="markdown-alert markdown-alert-note" data-source-line="1" dir="auto">"#,
                r#"<p class="markdown-alert-title" dir="auto">"#,
                r#"<span class="alert-icon" data-alert-type="note"></span>NOTE</p>"#,
                r#"<p data-source-line="2">body</p></div>"#,
            )
        );
    }

    #[test]
    fn a_callout_without_a_body_paragraph_does_not_shift_a_later_one() {
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--tip" data-source-span="0-19">"#,
            r#"<p class="ox-callout-title">Tip</p>"#,
            r#"<ul data-source-span="10-19"><li data-source-span="10-19">"#,
            r#"<p data-source-span="12-19">item</p></li></ul></blockquote>"#,
        );
        let annotated = annotate(html, &table("> [!TIP]\n> - item\n"), false);
        assert!(
            annotated
                .html
                .contains(r#"<p data-source-line="2">item</p>"#),
            "{}",
            annotated.html
        );
    }

    #[test]
    fn a_callout_body_paragraph_of_its_own_keeps_its_line() {
        // The marker stands alone, so the body paragraph starts where its
        // span says; a `]` further down the paragraph is body text.
        let html = concat!(
            r#"<blockquote class="ox-callout ox-callout--note" data-source-span="0-41">"#,
            r#"<p class="ox-callout-title">Note</p>"#,
            r#"<p data-source-span="14-40">first line
second ] line</p></blockquote>"#,
        );
        let annotated = annotate(
            html,
            &table("> [!NOTE]\n>\n> first line\n> second ] line\n"),
            false,
        );
        assert!(
            annotated.html.contains(r#"<p data-source-line="3">"#),
            "{}",
            annotated.html
        );
    }

    #[test]
    fn unknown_elements_and_malformed_spans_lose_the_attribute() {
        let html = r#"<section data-source-span="0-1">x</section><p data-source-span="nope">y</p>"#;
        let annotated = annotate(html, &table("x"), false);
        assert_eq!(annotated.html, "<section>x</section><p>y</p>");
    }
}
