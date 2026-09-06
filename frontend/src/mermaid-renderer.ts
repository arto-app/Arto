import mermaid from "mermaid";
import type { Theme } from "./theme";
import { buildMermaidThemeConfig } from "./mermaid-theme";
import { fixTextContrast } from "./mermaid-contrast";
import { openMermaidWindow } from "./mermaid-window-controller";

export function init(): void {
  const config = buildMermaidThemeConfig("light");
  mermaid.initialize({
    startOnLoad: false,
    ...config,
    securityLevel: "loose",
    fontFamily: "inherit",
  });
}

export function setTheme(theme: Theme): void {
  const config = buildMermaidThemeConfig(theme);
  mermaid.initialize({
    startOnLoad: false,
    ...config,
    securityLevel: "loose",
    fontFamily: "inherit",
  });
}

export async function renderDiagrams(container: Element): Promise<void> {
  const mermaidBlocks = collectMermaidBlocks(container);

  if (mermaidBlocks.length === 0) {
    return;
  }

  console.debug(`Rendering ${mermaidBlocks.length} mermaid diagrams in parallel`);

  // Render all diagrams in parallel for better performance
  const renderPromises = mermaidBlocks.map((block) =>
    renderDiagram(block as HTMLElement).catch((error) => {
      console.error("Failed to render mermaid diagram:", error);
      // Don't let one failure stop others
    }),
  );

  await Promise.all(renderPromises);
  console.debug("All mermaid diagrams rendered");
}

async function renderDiagram(element: HTMLElement): Promise<void> {
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
    // Generate a unique ID for this diagram
    const id = `mermaid-${crypto.randomUUID()}`;

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
        openMermaidWindow(mermaidSource);
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
