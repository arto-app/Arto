import { BaseViewerController } from "./base-viewer-controller";

/**
 * Math window controller for displaying LaTeX expressions.
 * Inherits zoom/pan operations from BaseViewerController.
 */
export class MathWindowController extends BaseViewerController {
  #mathContainer: HTMLElement | null = null;
  #mathWrapper: HTMLElement | null = null;

  constructor() {
    super("math-window-canvas", 100.0);
    // Get wrapper and container (following BaseViewerController pattern)
    this.#mathWrapper = document.getElementById("math-wrapper");
    this.#mathContainer = document.getElementById("math-container");

    if (!this.#mathWrapper || !this.#mathContainer) {
      throw new Error("Math wrapper or container not found");
    }
  }

  /**
   * Initialize the Math window with LaTeX source.
   * @param source LaTeX source code
   * @param mathId Unique identifier for the math expression
   * @param theme Initial theme (light/dark/auto)
   */
  async init(source: string, mathId: string, theme: string): Promise<void> {
    // Set initial theme
    document.body.setAttribute("data-theme", theme);

    // Initialize and render LaTeX
    await this.#renderMath(source, mathId);

    // Setup event listeners
    this.setupEventListeners();

    // Initial fit to window
    setTimeout(() => this.fitToWindow(), 100);
  }

  /**
   * Set the theme and re-render the Math expression if needed.
   */
  setTheme(theme: string): void {
    document.body.setAttribute("data-theme", theme);
    // KaTeX automatically uses CSS variables from theme, no re-render needed
  }

  /**
   * Render the LaTeX source into the container.
   */
  async #renderMath(source: string, mathId: string): Promise<void> {
    if (!this.#mathContainer) return;

    try {
      // Wait for fonts to load before rendering
      if (document.fonts) {
        try {
          await document.fonts.ready;
        } catch {
          // Fonts not ready, but continue anyway
        }
      }

      // Store source for later reference
      this.#mathContainer.setAttribute("data-math-source", source);
      this.#mathContainer.setAttribute("data-math-id", mathId);

      // Wrap source with display math delimiters (source is pure LaTeX without delimiters)
      // Use $$ for display mode which renders as block-level math
      const wrappedSource = `$$${source}$$`;

      // Set wrapped content as text (KaTeX auto-render reads text nodes)
      this.#mathContainer.textContent = wrappedSource;

      // Dynamically import and use KaTeX auto-render
      const { default: renderMathInElement } = await import("katex/dist/contrib/auto-render.mjs");

      // Render all math in the container
      renderMathInElement(this.#mathContainer, {
        delimiters: [
          { left: "$$", right: "$$", display: true },
          { left: "$", right: "$", display: false },
          { left: "\\(", right: "\\)", display: false },
          { left: "\\[", right: "\\]", display: true },
        ],
        throwOnError: false,
        errorColor: "#cc0000",
      });

      // Ensure content has proper height for size calculation
      if (this.#mathContainer.scrollHeight === 0) {
        this.#mathContainer.style.display = "inline-block";
        this.#mathContainer.style.minHeight = "2em";
      }
    } catch (error) {
      console.error("Failed to render math:", error);
      if (this.#mathContainer) {
        BaseViewerController.showRenderError(this.#mathContainer, error);
      }
    }
  }

  protected getContentDimensions(): { width: number; height: number } {
    if (!this.#mathContainer) return { width: 1, height: 1 };

    // Get the rendered content dimensions (affected by CSS zoom)
    const rect = this.#mathContainer.getBoundingClientRect();
    const rawWidth = this.#mathContainer.scrollWidth || rect.width;
    const rawHeight = this.#mathContainer.scrollHeight || rect.height;

    // Convert from zoomed dimensions back to unscaled content size
    const scale = this.state.scale > 0 ? this.state.scale : 1;

    return {
      width: Math.max(rawWidth / scale, 1),
      height: Math.max(rawHeight / scale, 1),
    };
  }

  protected updateTransform(animate = false): void {
    if (!this.#mathWrapper || !this.#mathContainer) return;

    if (animate) {
      this.#mathWrapper.style.transition = "transform 0.3s ease-out";
      this.#mathContainer.style.transition = "zoom 0.3s ease-out";
    } else {
      this.#mathWrapper.style.transition = "none";
      this.#mathContainer.style.transition = "none";
    }

    // Separate zoom and translate to avoid coordinate space issues
    // wrapper handles position (translate)
    this.#mathWrapper.style.transform = `translate(${this.state.offsetX}px, ${this.state.offsetY}px)`;
    // inner container handles zoom
    this.#mathContainer.style.zoom = String(this.state.scale);
  }

  protected updateZoomDisplay(): void {
    // Update zoom level display via dioxus bridge
    const zoomPercent = Math.round(this.state.scale * 100);

    // Call global function to update Rust state
    window.updateZoomLevel(zoomPercent);
  }
}

// Global instance
let controller: MathWindowController | null = null;

declare global {
  interface Window {
    updateZoomLevel: (zoomPercent: number) => void;
    mathWindowController?: MathWindowController;
  }
}

/**
 * Initialize the Math window from Rust side.
 */
export async function initMathWindow(source: string, mathId: string, theme: string): Promise<void> {
  controller = new MathWindowController();
  await controller.init(source, mathId, theme);

  // Expose globally for Rust to call
  window.mathWindowController = controller;
}

/**
 * Set the theme for the Math window.
 */
export function setMathTheme(theme: string): void {
  if (controller) {
    controller.setTheme(theme);
  }
}

/**
 * Copy the Math expression as an image to the clipboard using rasterization.
 */
export async function copyMathAsImage(): Promise<void> {
  const container = document.getElementById("math-container");
  if (!container) {
    throw new Error("math-container element not found");
  }

  try {
    // Get background color from current theme
    const bgColor = getComputedStyle(document.body).getPropertyValue("--bg-color").trim();

    // Ensure fonts are loaded before rasterization so KaTeX renders correctly
    if (document.fonts) {
      try {
        await document.fonts.ready;
      } catch {
        // Fonts not ready, but continue anyway
      }
    }

    // Use html2canvas to rasterize the Math expression (same as code-copy.ts pattern)
    const html2canvas = (await import("html2canvas")).default;

    const canvas = await html2canvas(container, {
      scale: 2,
      backgroundColor: bgColor || "#ffffff",
      logging: false,
      onclone: (clonedDoc) => {
        // Ensure math content is properly rendered in clone
        const clonedContainer = clonedDoc.getElementById("math-container");
        if (clonedContainer) {
          clonedContainer.style.display = "inline-block";
        }
      },
    });

    // Convert canvas to data URL
    const dataUrl = canvas.toDataURL("image/png");

    // Send to Rust using the standard rustCopyImage handler
    if (window.rustCopyImage) {
      window.rustCopyImage(dataUrl);
    } else {
      throw new Error("Rust clipboard handler not available");
    }
  } catch (error) {
    console.error("Failed to copy math as image:", error);
    throw error;
  }
}

// Sync data-theme attribute when Rust dispatches theme changes
import { setupBodyThemeSync } from "./theme";
setupBodyThemeSync();
