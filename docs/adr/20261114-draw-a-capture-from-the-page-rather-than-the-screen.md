# Draw a capture from the page rather than the screen

Status: Accepted

## Context

A client that put a document in front of the reader cannot see how it came
out: whether a diagram drew, whether a table fits. It needs a picture of one
block, or of what the reader sees.

The WebView can hand back the pixels it put on screen, but each platform's
WebView does it differently, and it has pixels only for what is on screen. A
block further down would have to be scrolled to first, which moves the
reader the client is supposed to be helping.

The page already draws parts of itself into images, through `html2canvas`,
when a reader copies a code block or a formula as a picture.

## Decision

A capture is drawn by the page from its own document, with `html2canvas`,
on every platform. A block is drawn where it stands, on screen or not, after
its diagrams and formulas have been drawn; the reader is not moved.

## Alternatives considered

- Snapshot the WebView natively. The picture is exactly what is on screen,
  but only for what is on screen, and each platform needs its own code;
  a block out of view could be captured only by scrolling the reader to it.
- Render the block again outside the window, as `arto page` does. It would
  show what a fresh page draws, not what this window shows — its theme, its
  zoom and its fonts.

## Consequences

- One implementation serves every platform, and capturing never moves the
  reader.
- The picture is `html2canvas`'s drawing of the document, which can differ
  from the screen where it does not support a style the stylesheet uses.
- A capture of a block that was not yet drawn draws it first, which costs
  what scrolling to it would have.
