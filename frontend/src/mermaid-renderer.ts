import { currentTheme, type Theme } from "./theme";
import { buildMermaidThemeConfig } from "./mermaid-theme";
import { fixTextContrast } from "./mermaid-contrast";
import { type MermaidLibrary, mermaidLibrary } from "./libraries";
import { inertSvg } from "./preview-diagram";
import { whenNearViewport } from "./viewport-queue";
import { restoreCopyButton } from "./code-copy";

export function init(): void {
  // The document already names its theme, and the colours are read out of it,
  // so a literal here would only disagree with the tokens actually resolved.
  setTheme(currentTheme());
}

export function setTheme(theme: Theme): void {
  const mermaid = mermaidLibrary();
  if (!mermaid) {
    return;
  }
  const config = buildMermaidThemeConfig(theme);
  mermaid.initialize({
    startOnLoad: false,
    ...config,
    securityLevel: "loose",
    fontFamily: "inherit",
  });
}

export async function renderDiagrams(container: Element): Promise<void> {
  if (!mermaidLibrary()) {
    return;
  }
  const mermaidBlocks = collectMermaidBlocks(container);

  if (mermaidBlocks.length === 0) {
    return;
  }

  console.debug(`Queueing ${mermaidBlocks.length} mermaid diagrams`);

  // A diagram is the most expensive thing on the page — tens of milliseconds
  // each — so none is drawn until the reader is near it.
  for (const block of mermaidBlocks) {
    whenNearViewport(block, async () => {
      try {
        await renderDiagram(block);
      } catch (error) {
        console.error("Failed to render mermaid diagram:", error);
        // Don't let one failure stop others
      }
      // Drawing the diagram overwrites the block, button and all.
      restoreCopyButton(block as HTMLPreElement);
    });
  }
}

/**
 * Names the next diagram. Mermaid only asks for an id that is unique within
 * the document, and a counter is that everywhere Arto draws:
 * `crypto.randomUUID` is exposed in a secure context alone, which neither the
 * app's WebView on Windows nor Quick Look's is.
 */
let nextDiagramNumber = 1;

async function renderDiagram(element: HTMLElement): Promise<void> {
  const mermaid = mermaidLibrary();
  if (!mermaid) {
    return;
  }

  // Skip if already rendered (has SVG child or marked as rendered)
  if (element.dataset.rendered === "true" || element.querySelector("svg")) {
    return;
  }

  // Get the mermaid source code from the element data attribute
  // This data attribute is embedded during markdown parsing phase
  // in Rust code.
  const mermaidSource = element.dataset.originalContent || element.textContent || "";
  if (!mermaidSource) {
    element.dataset.rendered = "true"; // Mark as processed to skip in future
    return;
  }

  try {
    const id = `mermaid-${nextDiagramNumber++}`;

    // Render the diagram inside the target element so Mermaid measures
    // text in the same CSS context where the SVG will be displayed.
    // Without this, Mermaid measures text in a temporary container on
    // document.body (outside .markdown-body), causing size mismatches
    // between node boxes and their text content.
    const { svg } = await mermaid.render(id, mermaidSource, element);

    // Replace the text content with the rendered SVG
    element.innerHTML = svg;
    element.dataset.rendered = "true";

    // Make diagram clickable to open viewer
    const svgElement = element.querySelector("svg");
    if (svgElement) {
      // Fix text contrast for nodes with custom fill colors
      fixTextContrast(svgElement as SVGSVGElement);
      // Hover styling (cursor, opacity, outline) is handled by CSS via
      // pre.preprocessed-mermaid:hover in mermaid-window.css
      svgElement.addEventListener("click", () => {
        // The window is the app's; a standalone page installs no handler and
        // the click does nothing.
        if (typeof window.handleMermaidWindowOpen === "function") {
          window.handleMermaidWindowOpen(mermaidSource);
        }
      });
    }

    console.debug(`Rendered mermaid diagram: ${id}`);
  } catch (error) {
    console.error("Failed to render mermaid diagram:", error);
    // Show error in the diagram
    element.innerHTML = `<div style="color: red; padding: 1rem; border: 1px solid red; border-radius: 4px;">
      <strong>Mermaid Error:</strong><br/>
      <pre style="margin-top: 0.5rem; white-space: pre-wrap;">${error}</pre>
    </div>`;
    element.dataset.rendered = "true"; // Mark as processed even on error
  }
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
 * How many diagrams one preview draws. Each takes tens of milliseconds and is
 * drawn at once, not as the reader nears it, and a preview comes up when the
 * pointer merely rests on a link; the rest are shown as their source.
 */
export const MAX_PREVIEW_DIAGRAMS = 3;

/**
 * Draw the diagrams in `container`, which is not the page, with nothing in
 * them left to run (see `preview-diagram.ts`); returns whether any was drawn.
 *
 * All at once, since the viewport queue watches the page's scroller and
 * would never see them come near, and without the click that opens the
 * Mermaid window, which is the page's. A diagram that fails to draw is shown
 * as its source. `isCurrent` is asked before each
 * write: drawing takes a while, and a container no longer shown by then is
 * left as it is.
 */
export async function renderDiagramsElsewhere(
  container: Element,
  isCurrent: () => boolean,
): Promise<boolean> {
  const mermaid = mermaidLibrary();
  let drawn = false;
  let tried = 0;
  for (const block of collectMermaidBlocks(container)) {
    if (!isCurrent()) {
      return drawn;
    }
    const source = block.dataset.originalContent || block.textContent || "";
    if (!mermaid || !source || tried >= MAX_PREVIEW_DIAGRAMS) {
      showAsSource(block, source);
      continue;
    }
    tried += 1;
    const svg = await drawApart(mermaid, block, source);
    if (!isCurrent()) {
      return drawn;
    }
    if (!svg) {
      showAsSource(block, source);
      continue;
    }
    block.replaceChildren(svg);
    block.dataset.rendered = "true";
    fixTextContrast(svg);
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

/**
 * `source` drawn beside `block`, with nothing in it left to run, or
 * `null` when it could not be drawn.
 *
 * Mermaid measures text in the element it is handed, so that element sits
 * where `block` is and is set in its size, but hidden and apart from it:
 * `block` keeps its source until the drawing is known to be wanted.
 */
async function drawApart(
  mermaid: MermaidLibrary,
  block: HTMLElement,
  source: string,
): Promise<SVGSVGElement | null> {
  const scratch = document.createElement("div");
  scratch.style.cssText = "position: absolute; left: 0; top: 0; width: 100%; visibility: hidden;";
  scratch.style.fontSize = getComputedStyle(block).fontSize;
  block.after(scratch);
  try {
    const { svg } = await mermaid.render(`mermaid-${nextDiagramNumber++}`, source, scratch);
    return inertSvg(svg);
  } catch (error) {
    console.warn("Failed to draw a mermaid diagram outside the page:", error);
    return null;
  } finally {
    scratch.remove();
  }
}
