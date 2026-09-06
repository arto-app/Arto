import mermaid from "mermaid";
import type { Theme } from "./theme";
import { buildMermaidThemeConfig } from "./mermaid-theme";
import { fixTextContrast } from "./mermaid-contrast";
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

  async init(source: string, diagramId: string): Promise<void> {
    // Initialize mermaid with current theme
    const currentTheme = document.body.getAttribute("data-theme") as Theme;
    this.#initializeMermaidTheme(currentTheme || "light");

    // Render the Mermaid diagram
    await this.#renderDiagram(source, diagramId);

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
    // Update body theme attribute
    document.body.setAttribute("data-theme", theme);

    // Re-initialize mermaid with new theme
    this.#initializeMermaidTheme(theme as Theme);

    // Re-render the diagram with new theme asynchronously
    if (this.#diagramContainer) {
      const source = this.#diagramContainer.getAttribute("data-mermaid-source");
      const diagramId = this.#diagramContainer.getAttribute("data-diagram-id");

      if (source && diagramId) {
        // Re-render asynchronously without blocking
        this.#renderDiagram(source, diagramId)
          .then(() => {
            // Restore zoom and position after re-render
            this.updateTransform();
          })
          .catch((error) => {
            console.error("Failed to re-render diagram:", error);
          });
      }
    }
  }

  #initializeMermaidTheme(theme: Theme): void {
    const config = buildMermaidThemeConfig(theme);
    mermaid.initialize({
      startOnLoad: false,
      ...config,
      securityLevel: "loose",
      fontFamily: "inherit",
    });
  }

  async #renderDiagram(source: string, diagramId: string): Promise<void> {
    try {
      const { svg } = await mermaid.render(`viewer-${diagramId}`, source);
      if (this.#diagramContainer) {
        this.#diagramContainer.innerHTML = svg;

        // Set explicit pixel dimensions for CSS zoom to work properly.
        // Use viewBox dimensions rather than getBBox() because Mermaid's
        // Gantt renderer sets viewBox to the full intended area ("0 0 w h")
        // while getBBox() may return smaller content bounds, creating a
        // mismatch between viewBox and pixel dimensions that causes the
        // diagram to appear extremely small after zoom scaling.
        const svgElement = this.#diagramContainer.querySelector("svg");
        if (svgElement) {
          // Fix text contrast for nodes with custom fill colors
          fixTextContrast(svgElement as SVGSVGElement);

          const dims = this.#getViewerDimensions(svgElement);
          svgElement.setAttribute("width", String(dims.width));
          svgElement.setAttribute("height", String(dims.height));
          // Remove responsive max-width that conflicts with explicit dimensions
          svgElement.style.removeProperty("max-width");
        }

        // Store source and ID for theme switching
        this.#diagramContainer.setAttribute("data-mermaid-source", source);
        this.#diagramContainer.setAttribute("data-diagram-id", diagramId);
      }
    } catch (error) {
      console.error("Failed to render diagram:", error);
      if (this.#diagramContainer) {
        BaseViewerController.showRenderError(this.#diagramContainer, error);
      }
    }
  }

  // Get the intended diagram dimensions from the SVG's viewBox attribute.
  // Prefer viewBox over getBBox() because Mermaid sets viewBox to the full
  // intended rendering area during diagram generation, whereas getBBox()
  // returns only the tight content bounds which may be smaller (especially
  // for Gantt charts where viewBox="0 0 w h" encompasses padding/axis area).
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

declare global {
  interface Window {
    handleMermaidWindowOpen: (source: string) => void;
    mermaidWindowController?: MermaidWindowController;
    updateZoomLevel: (zoomPercent: number) => void;
  }
}

export async function initMermaidWindow(source: string, diagramId: string): Promise<void> {
  controller = new MermaidWindowController();
  await controller.init(source, diagramId);

  // Expose globally for Rust to call
  window.mermaidWindowController = controller;
}

// Function called from main markdown viewer to open window
export function openMermaidWindow(source: string): void {
  // Call Rust function via dioxus bridge
  window.handleMermaidWindowOpen(source);
}
