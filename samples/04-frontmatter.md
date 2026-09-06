---
title: Frontmatter Sample
author: Arto Team
date: 2025-01-07
draft: false
version: 1.0
count: 42
tags:
  - markdown
  - frontmatter
  - sample
settings:
  theme: dark
  sidebar: true
  width: 280
authors:
  - name: First Author
    role: writer
  - name: Second Author
    role: reviewer
description: >
  A folded multi-line string
  that becomes one line.
empty_value:
---

# Frontmatter

The YAML block above the first heading is shown as a collapsible table instead
of as text. Previous: [Math and diagrams](./03-math-and-diagrams.md).

**Expected:** a collapsed "Frontmatter" table at the very top of the document.
Expanding it shows one row per key with values styled by type:

- **String**: plain text (`title`, `author`, `date`, `description` as one line)
- **Boolean**: highlighted (`draft`, `settings.sidebar`)
- **Number**: highlighted (`version`, `count`, `settings.width`)
- **List of strings**: bullet points (`tags`)
- **Object**: a nested table (`settings`)
- **List of objects**: nested tables (`authors`)
- **null**: shown as `null` (`empty_value`)

## Source lines after frontmatter

This paragraph is the first block after the heading.

- A list item

**Expected:** right-click "copy path with line" on this paragraph and the list
item reports their real line numbers in the file, counting the frontmatter
lines at the top.

## Body content is unaffected

> [!NOTE]
> Frontmatter must start on the first line of the file and be closed by a
> line containing only `---`. A `---` later in the document is a horizontal
> rule, like the one below.

---

```rust
fn main() {
    println!("Hello, Arto!");
}
```

**Expected:** an alert, a horizontal rule, and a code block, exactly as in the
other samples.

---

Previous: [Math and diagrams](./03-math-and-diagrams.md) · Next: [Beyond GFM](./05-beyond-gfm.md)
