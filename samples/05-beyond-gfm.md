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
A target without a file extension names a Markdown document, so both open
`README.md` inside Arto.
**Plain GFM:** the double brackets stay visible.

## Heading attributes

Source: `### Custom identifier {#custom-heading-id .highlight}`

### Custom identifier {#custom-heading-id .highlight}

**Engine with heading attributes:** the braces disappear from the heading and
the table of contents, and the anchor is the one the block names, so
[this link](#custom-heading-id) scrolls here while
[this one](#custom-identifier) does not.
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

## HTML embeds

Source: a fenced block whose language is `html-embed`, holding HTML and a
`<style>`, as Zenn writes it.

```html-embed
<div class="flow">
  <div class="box">Input</div><div class="arrow">→</div>
  <div class="box">Process</div><div class="arrow">→</div>
  <div class="box">Output</div>
</div>
<p class="note">A <a href="https://example.com">link</a> stays text.</p>
<button onclick="alert(1)">Never shown</button>
<script>document.body.textContent = "script ran";</script>
<style>
  .flow { display: flex; align-items: center; gap: 8px; justify-content: center; padding: 16px; }
  .box { padding: 8px 16px; border: 2px solid #3ea8ff; border-radius: 8px;
         background: light-dark(#eaf6ff, #1a2f3f); }
  .note { text-align: center; margin: 0 0 16px; }
</style>
<style data-theme="dark">.arrow { color: #9fd3ff; }</style>
```

**Zenn:** the three boxes in a row, in a bordered frame labelled as
user-created content, the box fill and the arrows following the reader's
theme; the link is plain text and neither the button nor the script's text
appears.
**Arto:** the same, in a frame as tall as the figure.
**GitHub and plain GFM renderers:** the source as an HTML code block.

---

Previous: [Frontmatter](./04-frontmatter.md) · Back to the [index](./README.md)
