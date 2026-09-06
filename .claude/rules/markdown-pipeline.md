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
   table, prepended at the end).
2. Text pre-processing that keeps the line count: bare-URL autolinks.
3. The engine (`src/engine/`, the only place that knows the parser):
   GitHub alerts (`> [!NOTE]`) become `<div class="markdown-alert …">`,
   Mermaid and math code blocks and `$…$` become `preprocessed-*`
   containers that the frontend renders client-side, headings get their
   ids, and every block element is marked with the byte range it came
   from.
4. Source line annotation: the byte ranges become the `data-source-line`
   attributes, through the line table the engine returns.
5. Post-process with `lol_html`: inline local images as data URLs, turn
   local Markdown links into `<span class="md-link" data-md-link="…">`.

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
