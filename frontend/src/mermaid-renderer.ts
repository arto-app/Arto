import type { MathMarkup, RenderOptions } from "pele";

import { katexLibrary, mermaidLibrary } from "./libraries";
import { inertSvg } from "./preview-diagram";
import { whenNearViewport } from "./viewport-queue";
import { restoreCopyButton } from "./code-copy";

export function renderDiagrams(container: Element): Promise<void> {
  if (!mermaidLibrary()) {
    return Promise.resolve();
  }
  const mermaidBlocks = collectMermaidBlocks(container);

  if (mermaidBlocks.length === 0) {
    return Promise.resolve();
  }

  console.debug(`Queueing ${mermaidBlocks.length} mermaid diagrams`);

  // Laying a diagram out is cheap, but putting it on the page is not: each
  // one changes the height of everything below it.
  for (const block of mermaidBlocks) {
    whenNearViewport(block, () => {
      renderDiagram(block);
      // Drawing the diagram overwrites the block, button and all.
      restoreCopyButton(block as HTMLPreElement);
    });
  }
  return Promise.resolve();
}

/**
 * Names the next diagram, whose ids Pele prefixes with it. A counter is unique
 * everywhere Arto draws: `crypto.randomUUID` is exposed in a secure context
 * alone, which neither the app's WebView on Windows nor Quick Look's is.
 */
let nextDiagramNumber = 1;

/**
 * How `source` is drawn where `element` is: in its font, so the boxes Pele
 * measures are the boxes the text is set in.
 *
 * The colours are not among them. The SVG names them as CSS variables, which
 * the stylesheet maps onto the theme's tokens, so a diagram follows a theme
 * change without being drawn again.
 */
export function diagramOptions(element: Element): RenderOptions {
  const style = getComputedStyle(element);
  const fontSize = Number.parseFloat(style.fontSize);
  return {
    ...(style.fontFamily ? { fontFamily: style.fontFamily } : {}),
    ...(fontSize > 0 ? { fontSize } : {}),
    idPrefix: `mermaid-${nextDiagramNumber++}`,
    // A narrow window would otherwise turn a left-to-right flowchart on its
    // side; the document is shown as its author wrote it.
    autoDirection: false,
    math: typesetMath,
  };
}

/**
 * `tex` set by KaTeX as MathML, measured on the page at `size`, for a label
 * that holds a formula; `null` leaves the label as its source.
 *
 * MathML rather than KaTeX's HTML: Safari places HTML inside an SVG's
 * `foreignObject` wrongly (WebKit bug 23113), and Arto's macOS views are
 * Safari's engine.
 */
function typesetMath(tex: string, size: number): MathMarkup | null {
  const katex = katexLibrary();
  if (!katex) {
    return null;
  }
  let markup: string;
  try {
    markup = katex.renderToString(tex, { displayMode: true, output: "mathml" });
  } catch {
    return null;
  }
  const probe = document.createElement("div");
  probe.style.cssText =
    "position: absolute; left: -9999px; visibility: hidden; display: inline-block; white-space: nowrap;";
  probe.style.fontSize = `${size}px`;
  probe.innerHTML = markup;
  document.body.append(probe);
  try {
    const expression = probe.querySelector("math");
    if (!expression) {
      return null;
    }
    const { width, height } = expression.getBoundingClientRect();
    return { html: expression.outerHTML, width, height };
  } finally {
    probe.remove();
  }
}

function renderDiagram(element: HTMLElement): void {
  const pele = mermaidLibrary();
  if (!pele) {
    return;
  }

  // Skip if already rendered (has SVG child or marked as rendered)
  if (element.dataset.rendered === "true" || element.querySelector("svg")) {
    return;
  }

  // The source is embedded as a data attribute when the Markdown is rendered
  // in Rust.
  const mermaidSource = element.dataset.originalContent || element.textContent || "";
  if (!mermaidSource) {
    element.dataset.rendered = "true"; // Mark as processed to skip in future
    return;
  }

  try {
    const { svg } = pele.render(mermaidSource, diagramOptions(element));
    element.innerHTML = svg;
    element.dataset.rendered = "true";

    // Make diagram clickable to open viewer. Hover styling (cursor, opacity,
    // outline) is handled by CSS via pre.preprocessed-mermaid:hover in
    // mermaid-window.css
    element.querySelector("svg")?.addEventListener("click", () => {
      // The window is the app's; a standalone page installs no handler and
      // the click does nothing.
      if (typeof window.handleMermaidWindowOpen === "function") {
        window.handleMermaidWindowOpen(mermaidSource);
      }
    });
  } catch (error) {
    console.warn("Failed to render mermaid diagram:", error);
    showError(element, error);
    element.dataset.rendered = "true"; // Mark as processed even on error
  }
}

/**
 * Why `element`'s diagram could not be drawn, in its place. Written as text:
 * the message quotes the source, which is the document's.
 */
function showError(element: HTMLElement, error: unknown): void {
  const box = document.createElement("div");
  box.style.cssText = "color: red; padding: 1rem; border: 1px solid red; border-radius: 4px;";
  const title = document.createElement("strong");
  title.textContent = "Mermaid Error:";
  const message = document.createElement("pre");
  message.style.cssText = "margin-top: 0.5rem; white-space: pre-wrap;";
  message.textContent = error instanceof Error ? error.message : String(error);
  box.append(title, message);
  element.replaceChildren(box);
}

function collectMermaidBlocks(container: Element): HTMLElement[] {
  const blocks = new Map<HTMLElement, string>();

  const preprocessed = container.querySelectorAll("pre.preprocessed-mermaid:not([data-rendered])");
  preprocessed.forEach((block) => {
    const element = block as HTMLElement;
    blocks.set(element, element.dataset.originalContent || element.textContent || "");
  });

  const codeBlocks = container.querySelectorAll("pre code.language-mermaid:not([data-rendered])");
  codeBlocks.forEach((code) => {
    const pre = code.closest("pre");
    if (!pre) {
      return;
    }
    const element = pre as HTMLElement;
    if (blocks.has(element)) {
      return;
    }
    const source = code.textContent || "";
    element.classList.add("preprocessed-mermaid");
    element.dataset.originalContent = source;
    blocks.set(element, source);
  });

  return Array.from(blocks.keys());
}

/**
 * How many diagrams one preview draws. A preview comes up when the pointer
 * merely rests on a link, and its diagrams are drawn at once rather than as
 * the reader nears them; the rest are shown as their source.
 */
export const MAX_PREVIEW_DIAGRAMS = 3;

/**
 * Draw the diagrams in `container`, which is not the page, with nothing in
 * them left to run (see `preview-diagram.ts`); returns whether any was drawn.
 *
 * All at once, since the viewport queue watches the page's scroller and
 * would never see them come near, and without the click that opens the
 * Mermaid window, which is the page's. A diagram that fails to draw is shown
 * as its source.
 */
export function renderDiagramsElsewhere(container: Element): boolean {
  const pele = mermaidLibrary();
  let drawn = false;
  let tried = 0;
  for (const block of collectMermaidBlocks(container)) {
    const source = block.dataset.originalContent || block.textContent || "";
    if (!pele || !source || tried >= MAX_PREVIEW_DIAGRAMS) {
      showAsSource(block, source);
      continue;
    }
    tried += 1;
    let svg: SVGSVGElement | null = null;
    try {
      svg = inertSvg(pele.render(source, diagramOptions(block)).svg);
    } catch (error) {
      console.warn("Failed to draw a mermaid diagram outside the page:", error);
    }
    if (!svg) {
      showAsSource(block, source);
      continue;
    }
    block.replaceChildren(svg);
    block.dataset.rendered = "true";
    drawn = true;
  }
  return drawn;
}

/** `block` as the plain code block its source is. */
function showAsSource(block: HTMLElement, source: string): void {
  block.classList.remove("preprocessed-mermaid");
  delete block.dataset.originalContent;
  block.textContent = source;
}
