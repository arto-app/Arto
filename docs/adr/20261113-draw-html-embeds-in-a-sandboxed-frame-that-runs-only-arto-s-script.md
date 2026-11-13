# Draw HTML embeds in a sandboxed frame that runs only Arto's script

Status: Accepted

## Context

Zenn draws a ` ```html-embed ` fence as the figure its HTML and CSS describe.
The markup is the document's, and in the app the document shares a WebView
with the bridge to Rust, which has no Content-Security-Policy to fall back on.
A figure's `<style>` would also restyle the page around it if it were let in.
Zenn shows a figure as tall as it is, up to a cap, and in the reader's theme.

## Decision

An HTML embed is drawn in an `<iframe srcdoc>` sandboxed without the page's
origin and with scripts allowed, under a policy of its own that runs one
script: Arto's, which reports the figure's height and repaints it when the
page names a new theme. The document's markup is never placed in the page,
and none of its scripts runs.

## Alternatives considered

- Sanitize the markup and place it in the page, its styles scoped by a
  shadow root. Everything then rests on the sanitizer being complete, and the
  document's markup sits beside the bridge to Rust.
- A frame with scripts disallowed. Nothing in the frame or outside it can
  then measure the figure or repaint it, so every figure would be drawn at a
  fixed height, mostly empty, and in the theme it was first drawn in.

## Consequences

- A page whose own policy allowlists scripts by hash — what `arto page`
  writes, and a host's page that embeds Arto's frontend — has to allowlist
  the frame's script too, since a `srcdoc` frame is held to both policies.
- The frame's script is one text shared by the frontend and the Rust side.
- The page cannot reach into a figure, so what it draws over the document —
  copy as image, the content cursor — stops at the frame's edge.
