/**
 * Mermaid, for a page whose document draws a diagram.
 *
 * Three quarters of the frontend's weight is here, which is why it is a
 * bundle of its own: `arto-page` appends it after `page` only when the body
 * it is writing holds a diagram. It reaches the runtime through the global
 * that bundle publishes, having no module scope in common with it.
 */

import mermaid from "mermaid";

const runtime = window.ArtoRenderer;
if (runtime) {
  runtime.provideMermaid(mermaid);
} else {
  console.error("Mermaid loaded without the page runtime; diagrams will stay as source");
}
