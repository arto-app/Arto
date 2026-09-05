# Arto Rendering Samples

These documents exercise every Markdown construct Arto renders. Open this
directory in Arto (`arto example/`) and walk through the files in order. Each
section states what you should see, so a wrong rendering stands out without
comparing against another viewer.

The files build on each other from small to large constructs:

1. [Inline syntax](./01-inline.md) — emphasis, code, links, images, breaks,
   escapes, raw inline HTML
2. [Block syntax](./02-blocks.md) — headings, lists, quotes, code blocks,
   tables, footnotes, alerts, HTML blocks
3. [Math and diagrams](./03-math-and-diagrams.md) — KaTeX and Mermaid,
   including the edge cases where math meets Markdown punctuation
4. [Frontmatter](./04-frontmatter.md) — the YAML metadata table
5. [Beyond GitHub Flavored Markdown](./05-beyond-gfm.md) — syntax outside
   the CommonMark and GFM specifications, which GitHub, plain GFM renderers,
   and Arto's engine each treat differently; use it to see what an engine
   adds or drops

## How to read the samples

- A line starting with **Expected:** describes the correct rendering of the
  block right above it.
- A line starting with **GitHub:** notes where GitHub's own rendering differs
  from what Arto shows, so a difference is a known one rather than a bug.
- Source text that must stay visible verbatim is repeated in a code span next
  to the example.

## Interactive checks

Rendering is only half of it. With any of these files open:

- **Table of contents**: the right sidebar lists every heading, including the
  Japanese heading and the two headings that share a title in the Headings
  section of [Block syntax](./02-blocks.md). Clicking each one scrolls to it.
- **Copy path with line**: right-click a paragraph, a nested list item, a
  paragraph inside a quote, a table, and a code block. The copied line numbers
  must match the source file for each of them.
- **Copy selection as Markdown**: select rendered text that spans bold or a
  link and copy it as source. The `**` and `[…](…)` markers must come along.
- **Document links**: the links between these files open in the same tab and
  the back button returns. The Links section of
  [Inline syntax](./01-inline.md) also has a link to a missing file, which
  looks like a normal document link and does nothing when clicked, and links
  to a non-Markdown file and to a section anchor, which are styled as
  unavailable.
- **Auto reload**: edit any of these files while it is open; the view updates
  without losing the scroll position.
