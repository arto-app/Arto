# Inline Syntax

Everything that lives inside a paragraph. See the [index](./README.md) for how
to read the **Expected:** lines.

## Emphasis

*asterisk italic* and _underscore italic_, **asterisk bold** and
__underscore bold__, ***bold italic*** and ___also bold italic___.

**Expected:** two italic, two bold, two bold-italic runs, no visible `*` or `_`.

Nested: **bold with _italic_ inside** and _italic with **bold** inside_.

**Expected:** the inner run takes both styles.

Intraword underscores stay literal: `snake_case_name` written without code is
snake_case_name, and file_name_with_parts keeps its underscores.

**Expected:** no italics in the two identifiers above.

Intraword asterisks do emphasize: un*frigging*believable.

**Expected:** "frigging" is italic.

Emphasis next to punctuation: **"quoted"** and **(parenthesized)** and **bold**.

**Expected:** all three bold.

Emphasis against CJK punctuation: これは**「重要」**です。

**Expected:** depends on the engine. CommonMark's flanking rules leave the
asterisks visible here because `「` and `」` are punctuation. An engine with CJK
emphasis support renders 「重要」 in bold.
**GitHub:** leaves the asterisks visible.

## Strikethrough

~~two tildes~~ is GitHub strikethrough. Combined: **bold ~~and struck~~** and
~~struck with `code` inside~~.

**Expected:** three struck-through runs, the code span keeps its background.

Single tildes: ~one tilde~.

**Expected:** depends on the engine. GitHub renders single-tilde strikethrough.
See the Subscript section of [Beyond GFM](./05-beyond-gfm.md) for what Arto's
engine does with it.

## Inline code

Use `inline code` for identifiers such as `std::io::stdin()`. Backticks inside
code need a longer fence: `` `backticks` `` and ``` `` double `` ```.

**Expected:** every code span has a background; the inner backticks are visible
inside the last two spans.

Code spans keep Markdown literal: `**not bold**`, `[not a link](x)`, `$not math$`.

**Expected:** all three shown verbatim with their punctuation.

## Links

- Inline: [Rust](https://www.rust-lang.org/)
- With title: [GitHub](https://github.com "GitHub Homepage") (hover shows the title)
- Reference style: [CommonMark spec][spec] and [collapsed][] and [shortcut]
- Angle-bracket destination: [with a space in the URL](<https://example.com/a b>)
- Autolink in angle brackets: <https://commonmark.org/>
- Bare URL: https://spec.commonmark.org/0.31.2/#links
- Bare URL followed by punctuation: see https://example.com/page. Trailing dot excluded.
- Bare URL with parentheses: https://en.wikipedia.org/wiki/Rust_(programming_language)
- `www.` without scheme: www.example.com
- Email: contact@example.com

[spec]: https://spec.commonmark.org/
[collapsed]: https://spec.commonmark.org/0.31.2/#links
[shortcut]: https://spec.commonmark.org/0.31.2/#reference-link

**Expected:** every item except the last two is a link. The trailing dot after
`page` and the closing parenthesis of the Wikipedia URL belong to the text and
the URL respectively.
**GitHub:** also links `www.example.com` and the email address; Arto's
behavior there depends on the engine.

Links to other documents:

- [Block syntax](./02-blocks.md) — opens in Arto
- [A file that does not exist](./does-not-exist.md) — opens nothing
- [A non-Markdown file](./assets/sample.svg) — styled as unavailable
- [A section anchor](./02-blocks.md#tables) — styled as unavailable

**Expected:** the first one navigates inside Arto; the missing file looks like
a normal document link but clicking it does nothing; the last two are dimmed.
Anchors on document links are not followed.

## Images

Remote image:

![Rust logo](https://www.rust-lang.org/static/images/rust-logo-blk.svg)

Local image, relative path:

![Local sample](./assets/sample.svg)

Local image with a title and reference-style syntax:

![Local sample again][sample]

[sample]: ./assets/sample.svg "Sample title"

Missing local image:

![This file does not exist](./assets/missing.png)

Image inside a link: [![Local sample](./assets/sample.svg)](./README.md)

**Expected:** the first three images render (the local one is inlined as a data
URL, so it also shows offline); the missing one shows its alt text; the last
one is clickable and opens the index.

## Line breaks

Two trailing spaces  
force a hard break. So does a backslash\
at the end of the line.
A plain newline is a soft break and stays on the same line.

**Expected:** the first three lines are three visual lines; the fourth
continues the third on the same line.

## Escapes and entities

Escaped punctuation: \*not italic\*, \_not italic\_, \`not code\`, \# not a
heading, \[not a link\](x), 1\. not a list.

**Expected:** the characters are visible and nothing is formatted.

Entities: &copy; &amp; &lt;tag&gt; &quot;quoted&quot; &nbsp;non-breaking &mdash;
numeric &#8212; and hex &#x1F600;.

**Expected:** ©, &, <tag>, "quoted", a dash, another dash, and an emoji.

Backslash before a non-punctuation character stays: C:\Users\name and 3\4.

## Raw inline HTML

Tags Markdown has no syntax for: H<sub>2</sub>O, x<sup>2</sup>, press
<kbd>Cmd</kbd> + <kbd>F</kbd>, <mark>highlighted</mark>, <abbr title="GitHub
Flavored Markdown">GFM</abbr>, <ins>inserted</ins>, <del>deleted</del>,
<small>small print</small>, and <span style="color: tomato">inline style</span>.

**Expected:** subscript, superscript, key caps, highlight, dotted abbreviation,
underline, strikethrough, smaller text, and a colored word.

Disallowed raw HTML: <script>alert("x")</script>
<style>.tagfilter-probe { color: red }</style>
<span class="tagfilter-probe">this sentence turns red if style tags are applied</span>.

**Expected:** no dialog appears. Whether the sentence turns red depends on
the engine: GitHub's tag filter escapes `<script>` and `<style>` so they show
as text; an engine that passes raw HTML through applies the style.

An HTML comment follows this sentence and must be invisible. <!-- hidden -->

## Emoji and Unicode

Emoji: 🎉 🚀 ✨ 👩‍💻 🇯🇵. Shortcodes are **not** expanded: :tada: :rocket:

**Expected:** the first row shows emoji including the multi-codepoint ones; the
shortcodes stay as text.
**GitHub:** expands the shortcodes.

Mixed scripts in one paragraph: English, 日本語、한국어, العربية, עברית, and
combining marks: é (e + ◌́), ﬁ ligature.

**Expected:** every script displays with its own font; right-to-left runs are
shaped correctly inside the left-to-right paragraph.

---

Next: [Block syntax](./02-blocks.md)
