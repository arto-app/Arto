# Block Syntax

Structure above the paragraph level. Previous: [Inline syntax](./01-inline.md).

## Headings

# Level 1
## Level 2
### Level 3
#### Level 4
##### Level 5
###### Level 6

Setext heading level 1
======================

Setext heading level 2
----------------------

### Heading with `code`, **bold**, and a [link](./README.md)

### 日本語の見出し

### Repeated Title

### Repeated Title

**Expected:** six sizes, the two setext headings match levels 1 and 2, inline
formatting inside headings is applied, and the table of contents lists every
heading here including the Japanese one and both "Repeated Title" entries.
Clicking either "Repeated Title" in the table of contents scrolls to the right
one. Whether clicking the Japanese heading scrolls depends on the engine: an
anchor made only of ASCII letters is empty for it.

## Paragraphs

A paragraph is one or more lines of text.
Consecutive lines belong to the same paragraph.

A blank line starts a new paragraph. Leading spaces   are collapsed,
and trailing spaces on the last line are ignored.

**Expected:** two paragraphs; the internal spacing is single spaces.

## Block quotes

> A single-line quote.

> A quote with **formatting**, `code`, and a [link](./README.md).
> The second line continues the same paragraph.
>
> A second paragraph in the same quote.

> Lazy continuation: this line starts with the marker
and this one does not, but still belongs to the quote.

> Level 1
>> Level 2
>>> Level 3

> A quote containing other blocks:
>
> - a list item
> - another one
>
> ```rust
> fn quoted() {}
> ```
>
> | a | b |
> | - | - |
> | 1 | 2 |

**Expected:** a vertical bar on the left of every quote; three nested bars for
the nested one; the list, code block, and table render inside the last quote.

## Lists

### Unordered

- Hyphen marker
- Second item
  - Nested with two spaces
  - Another nested item
    - Third level
- Back to the first level

* Asterisk marker
* Second item

+ Plus marker
+ Second item

**Expected:** three separate lists (marker changes start a new list), bullets
change style per nesting level.

### Ordered

1. First
2. Second
   1. Nested first
   2. Nested second
3. Third

3) Starts at three with a parenthesis
4) Four
5) Five

**Expected:** the second list starts at 3.

### Loose and tight

- Tight item one
- Tight item two

- Loose item one

- Loose item two

**Expected:** the second list has vertical space between its items; the first
does not.

### Items containing blocks

1. A paragraph item.

   A second paragraph inside the same item.

   ```bash
   echo "code inside a list item"
   ```

   > A quote inside a list item.

   | inside | list |
   | ------ | ---- |
   | a      | b    |

2. The next item, still numbered 2.

**Expected:** everything between the two numbers is indented to the item's
text, and the numbering continues with 2.

### Task lists

- [x] Done
- [ ] Not done
- [x] Done with **bold** and `code`
  - [ ] Nested task
  - [x] Nested done task
- [ ] ~~Struck~~ task

**Expected:** checkboxes instead of bullets, checked where marked, and the
checkboxes are not clickable.

## Horizontal rules

Three markers render the same rule:

---

***

___

**Expected:** three identical horizontal lines.

## Code blocks

### Fenced with a language

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();
    println!("Sum: {sum}");
}
```

```javascript
const greeting = (name) => {
  console.log(`Hello, ${name}!`);
};
```

```python
def fibonacci(n):
    """Generate Fibonacci sequence up to n terms."""
    a, b = 0, 1
    for _ in range(n):
        yield a
        a, b = b, a + b
```

```bash
#!/usr/bin/env bash
set -euo pipefail
cargo build --release && cargo test --all
```

```toml
[package]
name = "arto"
edition = "2021"
```

```json
{ "name": "arto", "features": ["async", "serde"], "count": 3, "ok": true }
```

```yaml
title: Arto
tags: [markdown, reader]
nested:
  key: value
```

```diff
 fn calculate_sum(numbers: &[i32]) -> i32 {
-    let mut sum = 0;
-    for num in numbers {
-        sum += num;
-    }
-    sum
+    numbers.iter().sum()
 }
```

```html
<div class="card"><a href="#">link</a></div>
```

```css
.card { color: tomato; border: 1px solid; }
```

```sql
SELECT id, name FROM users WHERE active = true ORDER BY name;
```

**Expected:** each block is highlighted for its language, the diff block colors
removed and added lines, and every block has a copy button on hover.

### Fenced without a language

```
plain text, no highlighting
<b>tags are shown literally</b>
```

**Expected:** monospaced, no colors, the tags are visible as text.

### Unknown language and info string

```made-up-language title="ignored extra info"
still rendered as a code block
```

**Expected:** a plain code block; the info string is not shown.

### Tilde fence and fence inside a fence

~~~markdown
```rust
fn inner() {}
```
~~~

````markdown
```
three backticks inside four
```
````

**Expected:** two code blocks whose content shows the inner fences literally.

### Indented code block

    Four leading spaces make a code block.
    Indentation inside is preserved:
        eight spaces here.

**Expected:** a code block identical in style to the fenced ones.

### Long lines

```text
This line is intentionally long so that the code block has to scroll horizontally rather than wrap: 0123456789 0123456789 0123456789 0123456789 0123456789 0123456789 0123456789 0123456789 0123456789 0123456789
```

**Expected:** a horizontal scrollbar inside the block; the page itself does not
scroll sideways.

## Tables

| Left aligned | Center aligned | Right aligned | Default |
| :----------- | :------------: | ------------: | ------- |
| left         |     center     |         right | default |
| a            |       b        |             c | d       |

**Expected:** the header and cells follow the alignment row.

| Formatting in cells | Result                     |
| ------------------- | -------------------------- |
| `code`              | monospaced                 |
| **bold** and _em_   | styled                     |
| [link](./README.md) | opens the index            |
| ~~struck~~          | struck through             |
| escaped pipe \|     | one visible pipe           |
| ![img](./assets/sample.svg) | an image           |
|                     | empty first cell           |

**Expected:** inline formatting works inside cells; the `\|` shows as a pipe
without splitting the cell; the empty cell keeps its column.

| Wide | table | with | many | columns | to | force | horizontal | scrolling | inside | the | table | wrapper | instead | of | the | page |
| ---- | ----- | ---- | ---- | ------- | -- | ----- | ---------- | --------- | ------ | --- | ----- | ------- | ------- | -- | --- | ---- |
| 1    | 2     | 3    | 4    | 5       | 6  | 7     | 8          | 9         | 10     | 11  | 12    | 13      | 14      | 15 | 16  | 17   |

**Expected:** the table scrolls horizontally on its own.

Header only:

| Just | a header |
| ---- | -------- |

**Expected:** a table with a header row and no body.

## Footnotes

A sentence with a footnote[^1], another with a named one[^named], and the
first one referenced again[^1].

[^1]: The first footnote.
[^named]: A footnote with several paragraphs.

    The second paragraph is indented by four spaces.

    ```rust
    fn footnotes_can_hold_code() {}
    ```

**Expected:** superscript markers in the text, the definitions collected at
the bottom of the document, each with a back-reference arrow, and the named
footnote showing two paragraphs and a code block.

## GitHub alerts

> [!NOTE]
> Useful information that users should know, even when skimming content.

> [!TIP]
> Helpful advice for doing things better or more easily.

> [!IMPORTANT]
> Key information users need to know to achieve their goal.

> [!WARNING]
> Urgent info that needs immediate user attention to avoid problems.

> [!CAUTION]
> Advises about risks or negative outcomes of certain actions.

> [!NOTE]
> An alert with several blocks:
>
> - a list item
> - another one
>
> ```rust
> fn inside_alert() {}
> ```
>
> And a closing paragraph.

> [!note]
> A lowercase marker.

> Not an alert: the marker [!NOTE] is not at the start of the quote.

**Expected:** five alerts with a colored bar and a title matching the kind, the
multi-block alert keeps its list, code, and paragraph inside the bar, and the
last quote is an ordinary quote with the bracketed text visible. The lowercase
marker currently renders as an ordinary quote with `[!note]` visible, because
only uppercase markers are recognized (tracked in #222); an engine that
compares the marker case-insensitively shows a Note instead.

## HTML blocks

<details>
<summary>Click to expand</summary>

Markdown **inside** a details block still renders.

- list item
- another

</details>

<details open>
<summary>Expanded by default</summary>

The `open` attribute keeps this one visible.

</details>

<div align="center">

Centered block via a `div`.

</div>

<table>
  <tr><th>Raw</th><th>HTML table</th></tr>
  <tr><td>a</td><td>b</td></tr>
</table>

<!-- An HTML comment block: nothing from it may appear in the output. -->

**Expected:** a collapsed and an expanded disclosure, a centered paragraph, a
table, and no trace of the comment.

---

Previous: [Inline syntax](./01-inline.md) · Next: [Math and diagrams](./03-math-and-diagrams.md)
