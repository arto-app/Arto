# Beyond GitHub Flavored Markdown

Nothing on this page is part of CommonMark or the GFM specification, and
viewers disagree on it: GitHub renders some of it (emoji shortcodes, bare
autolinks, math in table cells), a plain GFM renderer renders none of it, and
Arto renders whatever subset its Markdown engine supports. That makes this
file the place where an engine change becomes visible: every section shows
the source in a code span, the rendered form right below it, and what the
other renderers show instead. Previous: [Frontmatter](./04-frontmatter.md).

Use it in two ways. After changing the engine, walk through it once and note
which sections switched. When writing documents meant to look the same on
GitHub and in Arto, avoid everything on this page except the constructs both
render.

## Smart punctuation

Source: `"double quotes" 'single quotes' -- en dash --- em dash ... ellipsis`

"double quotes" 'single quotes' -- en dash --- em dash ... ellipsis

**Engine with smart punctuation:** curly quotes “ ” ‘ ’, an en dash –, an em
dash —, and a single ellipsis character ….
**Plain GFM:** straight quotes and the literal `--`, `---`, `...`.

Apostrophes: it's, don't, the '90s.

## Superscript

Source: `x^2^ + y^2^ = r^2^` and `E = mc^2^`

x^2^ + y^2^ = r^2^ and E = mc^2^

**Engine with superscript:** the 2s are raised.
**Plain GFM:** the carets stay visible. Compare with the raw HTML form
x<sup>2</sup>, which every engine raises.

## Subscript and single tildes

Source: `H~2~O` and `CO~2~`

H~2~O and CO~2~

**Engine with subscript:** the numbers are lowered.
**GitHub:** treats single tildes as strikethrough, so it shows H~~2~~O with the
digit struck through.
**Plain CommonMark:** the tildes stay visible. Compare with the raw HTML form
H<sub>2</sub>O, which every engine lowers.

## Definition lists

Source:

```markdown
Markdown
: A lightweight markup language with plain text formatting syntax.

CommonMark
: A strongly defined, highly compatible specification of Markdown.
: A second definition for the same term.
```

Markdown
: A lightweight markup language with plain text formatting syntax.

CommonMark
: A strongly defined, highly compatible specification of Markdown.
: A second definition for the same term.

**Engine with definition lists:** each term on its own line with the
definitions indented below it.
**Plain GFM:** paragraphs beginning with a colon. Compare with the raw HTML
form, which every engine renders:

<dl>
<dt>Markdown</dt>
<dd>A lightweight markup language with plain text formatting syntax.</dd>
<dt>CommonMark</dt>
<dd>A strongly defined, highly compatible specification of Markdown.</dd>
</dl>

## Wiki links

Source: `[[README]]` and `[[README|Back to the index]]`

[[README]] and [[README|Back to the index]]

**Engine with wiki links:** two links; the second shows the text after the bar.
**Plain GFM:** the double brackets stay visible.

## Heading attributes

Source: `### Custom identifier {#custom-heading-id .highlight}`

### Custom identifier {#custom-heading-id .highlight}

**Engine with heading attributes:** the braces disappear from the heading and
the table of contents. Whether the anchor becomes `custom-heading-id` is up to
the viewer; Arto derives anchors from the heading text, so
[this link](#custom-heading-id) scrolls nowhere while
[this one](#custom-identifier) does.
**Plain GFM:** the braces are part of the heading text.

## Math in table cells

Source: a table whose cells contain `$E = mc^2$` and `$\sum_{i=1}^{n} i$`

| Formula            | Meaning     |
| ------------------ | ----------- |
| $E = mc^2$         | mass-energy |
| $\sum_{i=1}^{n} i$ | a sum       |

**Engine with math in cells:** two typeset formulas.
**Otherwise:** the dollar signs stay visible inside the cells while the same
formulas in the Inline math section of
[Math and diagrams](./03-math-and-diagrams.md) are typeset. GitHub typesets
both.

## Emoji shortcodes

Source: `:tada: :rocket: :white_check_mark:`

Shortcodes: :tada: :rocket: :white_check_mark:

**GitHub:** three emoji.
**Arto:** the shortcodes stay as text; use the emoji characters 🎉 🚀 ✅
directly.

## Autolinks without a scheme

Source: `www.example.com` and `contact@example.com`

www.example.com and contact@example.com

**GitHub and engines with GFM autolink literals:** both become links.
**Otherwise:** plain text, while https://example.com on the same page is a
link.

---

Previous: [Frontmatter](./04-frontmatter.md) · Back to the [index](./README.md)
