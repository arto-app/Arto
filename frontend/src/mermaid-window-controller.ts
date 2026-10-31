import { render } from "pele";
import { themedElement } from "./theme";
import { diagramOptions } from "./mermaid-renderer";
import { BaseViewerController } from "./base-viewer-controller";

class MermaidWindowController extends BaseViewerController {
  #wrapper: HTMLElement | null = null;
  #diagramContainer: HTMLElement | null = null;

  constructor() {
    super("mermaid-window-canvas", 100.0);
    this.#wrapper = document.getElementById("mermaid-diagram-wrapper");
    this.#diagramContainer = document.getElementById("mermaid-diagram-container");

    if (!this.#wrapper || !this.#diagramContainer) {
      throw new Error("Viewer container not found");
    }
  }

  init(source: string): void {
    this.#renderDiagram(source);

    // Setup event listeners
    this.setupEventListeners();

    // Listen for theme changes
    document.addEventListener("arto:theme-changed", ((event: CustomEvent) => {
      this.setTheme(event.detail);
    }) as EventListener);

    // Initial fit to window
    setTimeout(() => this.fitToWindow(), 100);
  }

  setTheme(theme: string): void {
    // The diagram names its colours as CSS variables, so the attribute alone
    // repaints it.
    themedElement().setAttribute("data-theme", theme);
  }

  #renderDiagram(source: string): void {
    if (!this.#diagramContainer) return;
    try {
      // At its natural pixel size, which the window zooms from; a responsive
      // SVG would shrink to the container instead.
      const { svg } = render(source, {
        ...diagramOptions(this.#diagramContainer),
        responsive: false,
      });
      this.#diagramContainer.innerHTML = svg;
    } catch (error) {
      console.warn("Failed to render diagram:", error);
      BaseViewerController.showRenderError(this.#diagramContainer, error);
    }
  }

  // The area the diagram was laid out in, from its viewBox: getBBox() returns
  // only the tight content bounds, which leave out its padding.
  #getViewerDimensions(svg: SVGSVGElement): { width: number; height: number } {
    const viewBox = svg.getAttribute("viewBox");
    if (viewBox) {
      const parts = viewBox.split(/[\s,]+/).map(Number);
      if (parts.length === 4 && parts[2] > 0 && parts[3] > 0) {
        return { width: parts[2], height: parts[3] };
      }
    }
    // Fall back to getBBox for SVGs without viewBox
    const bbox = svg.getBBox();
    return { width: bbox.width || 1, height: bbox.height || 1 };
  }

  protected getContentDimensions(): { width: number; height: number } {
    if (!this.#diagramContainer) return { width: 1, height: 1 };
    const svg = this.#diagramContainer.querySelector("svg");
    if (!svg) return { width: 1, height: 1 };
    return this.#getViewerDimensions(svg);
  }

  protected updateTransform(animate = false): void {
    if (!this.#wrapper || !this.#diagramContainer) return;

    if (animate) {
      this.#wrapper.style.transition = "transform 0.3s ease-out";
      this.#diagramContainer.style.transition = "zoom 0.3s ease-out";
    } else {
      this.#wrapper.style.transition = "none";
      this.#diagramContainer.style.transition = "none";
    }

    // Separate zoom and translate to avoid coordinate space issues
    // wrapper handles position (translate)
    this.#wrapper.style.transform = `translate(${this.state.offsetX}px, ${this.state.offsetY}px)`;
    // inner container handles zoom
    this.#diagramContainer.style.zoom = String(this.state.scale);
  }

  protected updateZoomDisplay(): void {
    // Update zoom level display via dioxus bridge
    const zoomPercent = Math.round(this.state.scale * 100);

    // Call global function to update Rust state
    window.updateZoomLevel(zoomPercent);
  }
}

// Global instance
let controller: MermaidWindowController | null = null;

// `handleMermaidWindowOpen` is declared once, in main.ts, alongside the other
// window-open callbacks the Rust side installs in the main viewer window.
declare global {
  interface Window {
    mermaidWindowController?: MermaidWindowController;
    updateZoomLevel: (zoomPercent: number) => void;
  }
}

export function initMermaidWindow(source: string): void {
  controller = new MermaidWindowController();
  controller.init(source);

  // Expose globally for Rust to call
  window.mermaidWindowController = controller;
}
