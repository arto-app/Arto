/**
 * Pele, which draws Mermaid diagrams, for a page whose document draws one.
 *
 * It is several times the page runtime by weight, which is why it is a
 * bundle of its own: `arto-page` appends it after `page` only when the body
 * it is writing holds a diagram. It reaches the runtime through the global
 * that bundle publishes, having no module scope in common with it.
 */

import * as pele from "pele";

import { registerWithPage } from "./page-library";

registerWithPage(
  (runtime) => runtime.provideMermaid(pele),
  "Mermaid loaded without the page runtime; diagrams will stay as source",
);
