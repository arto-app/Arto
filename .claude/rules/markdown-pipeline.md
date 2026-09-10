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

The engine is [ox-content](https://github.com/ubugeeei-prod/ox-content), four
crates in `crates/arto-markdown/Cargo.toml` (`ox_content_allocator` / `_ast` /
`_parser` / `_renderer`) that are released together. Bare-URL autolinks,
GitHub alerts, heading slugs and the GFM tag filter are all its work; anything
missing there is an upstream issue rather than a local workaround. A version
change shows up as a snapshot diff, so review it rather than accepting it.

`RenderOptions` is what the reader chooses, and `src/engine.rs` is the only
place it meets ox-content: `parser_options` and `renderer_options` build both
option sets from it. Two rules bound what may go in it. The GFM baseline —
tables, task lists, strikethrough, footnotes — stays fixed, because a document
written for GitHub contains those and showing them as literal pipes would be a
broken reader rather than a configured one; so do the parts of the renderer
the HTML contract rests on (`source_spans`, `semantic_footnotes`). And every
option reaches the selection source map as well as the rendering
(`extract_source_selection` takes the options for that reason): the map is
built by parsing the source again, so reading it any other way than the
document on screen was read makes a selection count into the wrong bytes.

Heading attributes (`{#id .class}`) and wiki links (`[[Page]]`) are parser
options, so what is left of them here is Arto's own half. `engine/wiki.rs` turns a target into
an href (`.md` for one without an extension, the target as written otherwise),
which the render hook writes unencoded because the app opens it as a file
name; the hook also renders a heading the parser read an attribute block off,
to mark an id the document asked for by name so that it survives a render
without a table of contents. `src/line_endings.rs` is not about the parser at
all: it serves the two readers of the source that are not the parser.
`normalize` turns a lone `\r` into `\n` byte for byte — no offset moves — for
the line table, and
`to_lf` drops the `\r` of a `\r\n` as well for the selection source map, which
indexes text of its own.

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
