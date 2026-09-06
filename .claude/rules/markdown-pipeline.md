---
paths: "crates/arto-markdown/**, crates/arto/src/markdown.rs, crates/arto-page/**, samples/**, frontend/src/**"
---

# Markdown Rendering Pipeline

`crates/arto-markdown` owns Markdown → HTML for the app, `arto page` and
Quick Look. The crate docs in `crates/arto-markdown/src/lib.rs` are the
reference for the pipeline order and for the HTML contract (the attributes
and class names the frontend, the app and the CSS read). Read them before
changing output; the frontend selectors in `frontend/src/` depend on them.

Order of operations, which must not change:

1. Extract YAML frontmatter (rendered as a `<details class="frontmatter">`
   table, prepended at the end). A leading `---` block is cut off only
   when it parses as a YAML mapping; anything else is prose and stays in
   the body.
2. The engine (`src/engine/`, the only place that knows the parser and the
   HTML it writes). It parses once and renders with hooks, then rewrites
   the result into the crate's contract: Mermaid and math blocks and `$…$`
   become the `preprocessed-*` containers the frontend renders
   client-side, GitHub alerts (`> [!NOTE]`) become
   `<div class="markdown-alert …">`, the byte range on every block element
   becomes the `data-source-line` attributes, and heading ids survive only
   when a table of contents was asked for.
3. Post-process with `lol_html`: inline local images as data URLs, turn
   local Markdown links into `<span class="md-link" data-md-link="…">`.

The engine is [ox-content](https://github.com/ubugeeei-prod/ox-content),
pinned to an exact pre-release in `crates/arto-markdown/Cargo.toml`
(`ox_content_allocator` / `_ast` / `_parser` / `_renderer` move together).
Bare-URL autolinks, GitHub alerts, heading slugs and the GFM tag filter are
all its work; anything missing there is an upstream issue rather than a
local workaround.

Three modules under `src/engine/` are the exception, each carrying a doc
comment that says which upstream gap it stands in for and that it is meant
to be deleted: `line_endings.rs` (CRLF, which ox-content reads as text),
`attributes.rs` (`{#id .class}` on a heading) and `wiki.rs` (`[[Page]]`).
The last two need different seams — the attribute block reaches the
renderer inside one `Text` node, so a render hook lifts it onto the tag,
while the parser splits `[[` into separate `Text` nodes, so wiki links are
read off the rendered text in the annotation pass instead.

Rules of thumb:

- Output HTML is a contract. `samples/*.md` are snapshot-tested in
  `crates/arto-markdown/tests/samples.rs` (HTML and heading outline);
  review diffs with `cargo insta review`, and never accept a diff you did
  not intend.
- Behaviour tests go through the public API (`tests/pipeline.rs`); only
  pure functions get unit tests next to the code. Nothing outside
  `src/engine/` may name a parser type.
- Keep `RenderOptions` engine-neutral: it is also the `markdown` section of
  `config.json`.
- `arto page samples/02-blocks.md` prints the HTML without launching the
  app; use it to eyeball a change.
