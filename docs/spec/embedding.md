# Embedding

An application can show Markdown the way Arto does in a window of its own:
`arto-markdown` renders the HTML, and Arto's frontend, published as
`arto-frontend`, draws what the HTML leaves to script — Mermaid diagrams,
math and the colours of code. The host keeps its window: its keys, its scrolling, its
links and its theme stay its own.

## What a host is given

`arto-frontend` carries, as Rust strings:

- `STYLESHEET`, the styles of a rendered document;
- `RUNTIME`, the script that draws it, published as `window.ArtoRenderer`;
- the library bundles: Pele, which draws Mermaid diagrams, KaTeX, and
  highlight.js in a common and a complete build;
- `Libraries::for_fragment`, which names the bundles a rendered fragment
  needs, and `Libraries::scripts`, which yields them in the order to load
  them.

The library bundles are the ones `arto page` and Quick Look embed, and the
crate is versioned with `arto-markdown`. A document that draws no diagram needs no Pele, one that
sets no formula needs no KaTeX, and the complete highlight.js is needed only
for a language outside the common build.

## Placing a document

The fragment `arto-markdown` renders goes inside a viewer element:

```html
<div class="markdown-viewer">
  <article class="markdown-body"><!-- the rendered fragment --></article>
</div>
```

The stylesheet is loaded, then the runtime, then the library bundles the
fragment needs.

## The stylesheet stays in the viewer

Every rule of `STYLESHEET` applies inside an element of class
`markdown-viewer` and nowhere else. The design tokens are declared on that
element and the theme is named on it, so a `data-theme` or a custom property
the host declares on its own root is left alone, and nothing outside the
viewer is restyled. The host.s own rules still reach into the viewer, as they
reach any element of its page.

The viewer is painted as Arto paints the page around a document: its padding,
its background and the reading measure. A host overrides those on the viewer
like any other style.

## The script API

`window.ArtoRenderer.render(viewer)` draws what the fragment inside `viewer`
leaves to script. It names the current theme on `viewer` and gives it the
class `markdown-viewer` if it does not have it. Each diagram, formula and code
block is drawn as it comes near the visible part of the page, so a long
document costs what is on screen.

Calling `render` again on the same viewer after the host replaced its content
— a file changed on disk — draws the new content; what was drawn already is
left as it is. A viewer taken out of the page is forgotten.

`window.ArtoRenderer.setTheme(theme)` repaints every viewer rendered so far,
and every one rendered after, in one of GitHub's themes, named as
`config.json` names them (`light`, `dark_dimmed`, …). Diagrams take the new
colours from the stylesheet like the rest of the document, so nothing is drawn
again and the promise it returns settles at once. Until it is called, a viewer
is painted `light`.

The runtime does nothing until `render` is called and touches nothing outside
the viewers it was handed: no keyboard handler, no scrolling, no observer of
the host's document.

## What the host keeps

Links, images and source positions stay as `arto-markdown` writes them: a
document link is a `span.md-link` with its `data-md-link`, a block carries its
`data-source-range`, and images resolved with `ImageResolution::Deferred` are
for the host to serve. Clicking a diagram, a formula or an image opens nothing.

## Compatibility

The markup above, the stylesheet's confinement and the script API are one
interface with the HTML `arto-markdown` writes. A change to any of them is a
breaking change.
