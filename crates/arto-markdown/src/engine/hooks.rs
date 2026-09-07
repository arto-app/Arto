//! Render hooks: the Mermaid and math containers, headings that carry an
//! attribute block, and wiki links.
//!
//! Everything else falls through to the built-in renderer. The
//! `preprocessed-*` containers hold the source text twice — escaped as the
//! visible fallback and in `data-original-content` for the client-side
//! renderer — and the block ones carry the node's byte span, which
//! [`super::annotate`] turns into source lines like it does for every other
//! block element.
//!
//! A heading the parser read a `{#id .class}` block off is rendered here so
//! that the id can be marked as one the document asked for by name; a wiki
//! link is rendered here so that its target resolves to a Markdown document
//! (see [`super::wiki`]).

use ox_content_ast::{Heading, Link, Node, Span};
use ox_content_renderer::{slugify_heading, HtmlRenderContext, HtmlRenderControl, HtmlRenderHooks};

/// The hook set the engine renders with.
pub(super) struct ArtoHooks<'a> {
    /// The document body, to tell a wiki link from an ordinary one: both are
    /// link nodes, and only the source says which syntax wrote them.
    source: &'a str,
    /// Set while a wiki link's label is being rendered. The built-in link
    /// renderer suppresses URL autolinking inside an anchor through renderer
    /// state no hook can reach, so the label's text is written here instead;
    /// otherwise a bare URL in the label becomes a second `<a>` nested in
    /// the one this hook opened.
    in_wiki_label: bool,
}

impl<'a> ArtoHooks<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self {
            source,
            in_wiki_label: false,
        }
    }
}

impl HtmlRenderHooks for ArtoHooks<'_> {
    fn render_node(
        &mut self,
        node: &Node<'_>,
        cx: &mut HtmlRenderContext<'_>,
    ) -> HtmlRenderControl {
        match node {
            Node::Heading(heading) if heading.id.is_some() || !heading.classes.is_empty() => {
                self.render_heading(heading, cx)
            }
            Node::Link(link) if self.is_wiki_link(link.span) => self.render_wiki_link(link, cx),
            Node::Text(text) if self.in_wiki_label => {
                cx.write_escaped(text.value);
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

impl ArtoHooks<'_> {
    /// Whether the link at `span` was written as `[[target]]`.
    ///
    /// The parser turns wiki links into ordinary link nodes, so the
    /// delimiters in the source are what is left to tell them apart. Both
    /// ends are checked: a link text that opens with a bracket starts with
    /// `[[` too, and it does not close with `]]`.
    ///
    /// One construct escapes this: a table cell holding an escaped `\|`
    /// reports the spans inside it one byte early per escape
    /// (ubugeeei-prod/ox-content#1363), so the link is not recognised and
    /// its target keeps the name it was written with instead of gaining
    /// `.md`. A wiki link inside a footnote definition misses for another
    /// reason — that content never reaches a hook at all
    /// (ubugeeei-prod/ox-content#1362), which costs the math and Mermaid
    /// containers there as well.
    fn is_wiki_link(&self, span: Span) -> bool {
        let (start, end) = (span.start as usize, span.end as usize);
        let Some(source) = self.source.get(start..end) else {
            return false;
        };
        source.starts_with("[[") && source.ends_with("]]")
    }

    /// Render `[[target]]` as an anchor on the document the target names.
    ///
    /// The href is written as a path, not percent-encoded like an ordinary
    /// link's: the app opens it as a file name, and a wiki target commonly
    /// holds spaces.
    fn render_wiki_link(
        &mut self,
        link: &Link<'_>,
        cx: &mut HtmlRenderContext<'_>,
    ) -> HtmlRenderControl {
        cx.write("<a href=\"");
        cx.write_attribute_escaped(&super::wiki::href(link.url));
        cx.write("\">");
        let enclosing = std::mem::replace(&mut self.in_wiki_label, true);
        cx.render_nodes(&link.children, self);
        self.in_wiki_label = enclosing;
        cx.write("</a>");
        HtmlRenderControl::Handled
    }

    /// Render a heading whose `{#id .class}` block the parser lifted onto the
    /// node; every other heading is left to the built-in renderer.
    fn render_heading(
        &mut self,
        heading: &Heading<'_>,
        cx: &mut HtmlRenderContext<'_>,
    ) -> HtmlRenderControl {
        let depth = heading.depth.clamp(1, 6);
        cx.write("<h");
        cx.write_display(depth);
        cx.write(" data-source-span=\"");
        cx.write_display(heading.span.start);
        cx.write("-");
        cx.write_display(heading.span.end);
        // A heading that named only classes still needs an id, or the table
        // of contents has nothing to scroll to. The renderer's own id is not
        // reachable from here, so the slug is derived from the heading text —
        // which is what the renderer would have written itself.
        cx.write("\" id=\"");
        match heading.id {
            Some(id) => cx.write_attribute_escaped(id),
            None => cx.write_attribute_escaped(&slugify_heading(&super::outline::heading_text(
                &heading.children,
            ))),
        }
        cx.write("\"");
        if !heading.classes.is_empty() {
            cx.write(" class=\"");
            for (index, class) in heading.classes.iter().enumerate() {
                if index > 0 {
                    cx.write(" ");
                }
                cx.write_attribute_escaped(class);
            }
            cx.write("\"");
        }
        // Tells the annotation pass this id was authored, so it survives a
        // render that drops the generated ones. A heading that named only
        // classes did not ask for an id, so its slug is dropped like any
        // other generated one.
        if heading.id.is_some() {
            cx.write(" data-arto-authored-id");
        }
        cx.write(">");

        cx.render_nodes(&heading.children, self);

        cx.write("</h");
        cx.write_display(depth);
        cx.write(">\n");
        HtmlRenderControl::Handled
    }
}

/// Write one block container: `<tag data-source-span class data-original-content>`.
///
/// The content is what the client-side renderer is handed, so it is reduced
/// to `\n`: a math block keeps the `\r` of a CRLF file, which the same
/// document in LF does not have.
fn write_container(
    cx: &mut HtmlRenderContext<'_>,
    tag: &str,
    class: &str,
    span: Span,
    content: &str,
) {
    let content = &crate::line_endings::to_lf(content);
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
