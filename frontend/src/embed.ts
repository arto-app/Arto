/**
 * The runtime for a document placed in someone else's page.
 *
 * Arto's window and the pages `arto page` writes own the whole document, and
 * their runtime acts like it: it takes the keyboard, scrolls the page, watches
 * the body for changes and names the theme on `<html>`. A host that puts the
 * rendered fragment in a window of its own keeps all of that for itself, and
 * wants only what the HTML leaves to script — the diagrams, the formulas and
 * the colours of the code — drawn where it says, when it says.
 *
 * So this runtime does nothing until it is handed an element, touches nothing
 * outside the elements it was handed, and is published under the same global
 * as the page runtime, so that the library bundles `arto-frontend` ships register
 * themselves with it exactly as they do with a page.
 */

import "../style/embed.css";

import * as codeCopy from "./code-copy";
import * as mathRenderer from "./math-renderer";
import * as mermaidRenderer from "./mermaid-renderer";
import { applyPictureTheme } from "./picture-theme";
import * as syntaxHighlighter from "./syntax-highlighter";
import type { Theme } from "./theme";
import * as viewportQueue from "./viewport-queue";

export { provideMermaid, provideKatex, provideHtml2Canvas, provideHljs } from "./libraries";

/** The class the stylesheet is confined to; see `build/scope-stylesheet.ts`. */
const VIEWER = "markdown-viewer";

let theme: Theme = "light";

/** The elements rendered so far, for a theme change to reach. */
const viewers = new Set<Element>();

/** The viewers still in the host's page; the rest are forgotten here. */
function liveViewers(): Element[] {
  for (const viewer of viewers) {
    if (!viewer.isConnected) {
      viewers.delete(viewer);
    }
  }
  return [...viewers];
}

/**
 * Draw what the rendered fragment inside `viewer` leaves to script.
 *
 * Call it once the fragment is in place, and again whenever the host replaces
 * it: what was drawn already is left as it is, and what is new is drawn. The
 * element is marked as the one the stylesheet is confined to and named the
 * current theme. Each block is drawn as the reader comes near it.
 */
export function render(viewer: Element): void {
  // The host's page, not Arto's window: a `.content` there is the host's.
  viewportQueue.setScroller(null);
  // Jobs for blocks the host has since thrown away would only hold the queue.
  viewportQueue.prune();

  viewers.add(viewer);
  viewer.classList.add(VIEWER);
  viewer.setAttribute("data-theme", theme);
  draw(viewer);
}

function draw(viewer: Element): void {
  applyPictureTheme(viewer, theme);
  mathRenderer.renderMath(viewer);
  syntaxHighlighter.highlightCodeBlocks(viewer);
  void mermaidRenderer.renderDiagrams(viewer);
  codeCopy.addCopyButtons(viewer);
}

/**
 * Paint every viewer rendered so far, and every one rendered after, in
 * `theme` — one of GitHub's theme names, as `config.json` spells them.
 *
 * The diagrams follow the stylesheet like the rest of the document, so
 * nothing is drawn again. A promise all the same: the script API returns
 * one, and a host may await it.
 */
export function setTheme(next: Theme): Promise<void> {
  theme = next;
  for (const viewer of liveViewers()) {
    viewer.setAttribute("data-theme", theme);
    applyPictureTheme(viewer, theme);
  }
  return Promise.resolve();
}
