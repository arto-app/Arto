//! Render hooks: the Mermaid and math containers, and heading attributes.
//!
//! Everything else falls through to the built-in renderer. The
//! `preprocessed-*` containers hold the source text twice — escaped as the
//! visible fallback and in `data-original-content` for the client-side
//! renderer — and the block ones carry the node's byte span, which
//! [`super::annotate`] turns into source lines like it does for every other
//! block element.
//!
//! A heading that ends in a `{#id .class}` block is rendered here too, so
//! that the block becomes the tag's attributes instead of showing as text.
//! See [`super::attributes`] for why that syntax reaches the renderer at all.

use ox_content_ast::{Heading, Node, Span};
use ox_content_renderer::{slugify_heading, HtmlRenderContext, HtmlRenderControl, HtmlRenderHooks};

/// The hook set the engine renders with.
#[derive(Default)]
pub(super) struct ArtoHooks {
    /// The `Text` node a heading's attribute block was cut from, and the
    /// length that is left of it. Set while that heading's children render.
    strip: Option<(Span, usize)>,
}

impl HtmlRenderHooks for ArtoHooks {
    fn render_node(
        &mut self,
        node: &Node<'_>,
        cx: &mut HtmlRenderContext<'_>,
    ) -> HtmlRenderControl {
        match node {
            Node::Heading(heading) => self.render_heading(heading, cx),
            // The tail of a heading whose attribute block was lifted onto
            // the tag; everything after `len` is that block.
            Node::Text(text) if self.strip.is_some_and(|(span, _)| span == text.span) => {
                let (_, len) = self.strip.take().unwrap_or_default();
                cx.write_escaped(text.value.get(..len).unwrap_or(text.value));
                HtmlRenderControl::Handled
            }
            Node::CodeBlock(code_block) => {
                // Only the two languages the frontend renders itself are
                // taken over; every other fence stays a `<pre><code>`.
                let class = match code_block.lang {
                    Some("mermaid") => "preprocessed-mermaid",
                    Some("math") => "preprocessed-math",
                    _ => return HtmlRenderControl::Default,
                };
                write_container(cx, "pre", class, code_block.span, code_block.value);
                HtmlRenderControl::Handled
            }
            Node::MathBlock(math) => {
                write_container(
                    cx,
                    "div",
                    "preprocessed-math-display",
                    math.span,
                    math.value,
                );
                HtmlRenderControl::Handled
            }
            Node::InlineMath(math) => {
                // Inline containers carry no source line: the frontend
                // resolves lines from the block that encloses them.
                cx.write("<span class=\"preprocessed-math-inline\" data-original-content=\"");
                cx.write_attribute_escaped(math.value);
                cx.write("\">");
                cx.write_escaped(math.value);
                cx.write("</span>");
                HtmlRenderControl::Handled
            }
            _ => HtmlRenderControl::Default,
        }
    }
}

impl ArtoHooks {
    /// Render a heading that ends in an attribute block; leave every other
    /// heading to the built-in renderer.
    fn render_heading(
        &mut self,
        heading: &Heading<'_>,
        cx: &mut HtmlRenderContext<'_>,
    ) -> HtmlRenderControl {
        let Some(Node::Text(last)) = heading.children.last() else {
            return HtmlRenderControl::Default;
        };
        let Some((kept, attributes)) = super::attributes::split_trailing(last.value) else {
            return HtmlRenderControl::Default;
        };

        let depth = heading.depth.clamp(1, 6);
        cx.write("<h");
        cx.write_display(depth);
        cx.write(" data-source-span=\"");
        cx.write_display(heading.span.start);
        cx.write("-");
        cx.write_display(heading.span.end);
        // A heading without an explicit id still needs one, or the table of
        // contents has nothing to scroll to. The renderer's own id is not
        // reachable from here, so the slug is derived from the text the
        // block was cut from — which is what the renderer would have used
        // had the block not been there.
        cx.write("\" id=\"");
        match attributes.id {
            Some(id) => cx.write_attribute_escaped(id),
            None => cx.write_attribute_escaped(&slugify_heading(&super::outline::heading_text(
                &heading.children,
            ))),
        }
        cx.write("\"");
        if !attributes.classes.is_empty() {
            cx.write(" class=\"");
            for (index, class) in attributes.classes.iter().enumerate() {
                if index > 0 {
                    cx.write(" ");
                }
                cx.write_attribute_escaped(class);
            }
            cx.write("\"");
        }
        // Tells the annotation pass this id was authored, so it survives a
        // render that drops the generated ones.
        cx.write(" data-arto-authored-id>");

        self.strip = Some((last.span, kept.len()));
        cx.render_nodes(&heading.children, self);
        self.strip = None;

        cx.write("</h");
        cx.write_display(depth);
        cx.write(">\n");
        HtmlRenderControl::Handled
    }
}

/// Write one block container: `<tag data-source-span class data-original-content>`.
fn write_container(
    cx: &mut HtmlRenderContext<'_>,
    tag: &str,
    class: &str,
    span: Span,
    content: &str,
) {
    cx.write("<");
    cx.write(tag);
    cx.write(" data-source-span=\"");
    cx.write_display(span.start);
    cx.write("-");
    cx.write_display(span.end);
    cx.write("\" class=\"");
    cx.write(class);
    cx.write("\" data-original-content=\"");
    cx.write_attribute_escaped(content);
    cx.write("\">");
    cx.write_escaped(content);
    cx.write("</");
    cx.write(tag);
    cx.write(">\n");
}
