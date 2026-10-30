# Draw Mermaid diagrams with Pele

Status: Accepted

## Context

Mermaid.js was most of the frontend's weight. A page that `arto page` writes,
and every Quick Look preview, embeds the libraries its document needs inline,
so a document with one diagram carried megabytes of script.

Mermaid.js also writes the theme's colours into the SVG it draws. Every theme
change, and every print from a dark theme, drew every diagram again, and a
label on a fill the author chose needed a pass of its own to stay legible.

Pele reads the same Mermaid syntax, covers every diagram type Mermaid.js
ships, is a fraction of its size, draws synchronously, and names its colours
as CSS variables.

## Decision

Arto draws Mermaid diagrams with Pele instead of Mermaid.js. There is no
setting to choose between them.

## Alternatives considered

- Keep Mermaid.js. Its weight and its baked-in colours are what this decision
  removes.
- Offer both, Mermaid.js by default. Two renderers mean two sets of styles,
  tests and bundles for one feature, and a reader seldom has a reason to pick.

## Consequences

- Diagrams are laid out and styled by Pele, so they no longer look exactly as
  GitHub draws them, although they are read from the same source.
- A diagram follows a theme change through the stylesheet and is not drawn
  again.
- What Mermaid.js draws and Pele does not is no longer drawn.
