import { BaseViewerController } from "./base-viewer-controller";

/**
 * Image window controller for displaying and zooming images.
 * Inherits zoom/pan operations from BaseViewerController.
 */
export class ImageWindowController extends BaseViewerController {
  #imageElement: HTMLImageElement | null = null;
  #imageContainer: HTMLElement | null = null;
  #imageWrapper: HTMLElement | null = null;
  #fitMode: "fit" | "actual" = "fit";
  #naturalWidth: number = 0;
  #naturalHeight: number = 0;

  constructor() {
    super("image-window-canvas", 1000.0);
    // Get wrapper and container (following BaseViewerController pattern)
    this.#imageWrapper = document.getElementById("image-wrapper");
    this.#imageContainer = document.getElementById("image-container");
    this.#imageElement = this.#imageContainer?.querySelector("img") || null;

    if (!this.#imageWrapper || !this.#imageContainer || !this.#imageElement) {
      throw new Error("Image wrapper, container, or img element not found");
    }
  }

  /**
   * Initialize the Image window with image source.
   * @param src Image URL (data URL or HTTP URL)
   * @param imageId Unique identifier for the image
   */
  async init(src: string, imageId: string): Promise<void> {
    // Store source and ID
    this.#imageElement!.setAttribute("data-image-src", src);
    this.#imageElement!.setAttribute("data-image-id", imageId);

    // Wait for image to load with timeout
    return new Promise<void>((resolve, reject) => {
      const cleanup = () => {
        this.#imageElement!.onload = null;
        this.#imageElement!.onerror = null;
      };

      const timeout = setTimeout(() => {
        cleanup();
        reject(new Error("Image load timeout (30s)"));
      }, 30000);

      this.#imageElement!.onload = () => {
        clearTimeout(timeout);
        cleanup();
        this.#naturalWidth = this.#imageElement!.naturalWidth;
        this.#naturalHeight = this.#imageElement!.naturalHeight;
        this.fitToWindow();
        this.setupEventListeners();
        resolve();
      };

      this.#imageElement!.onerror = () => {
        clearTimeout(timeout);
        cleanup();
        reject(new Error("Failed to load image"));
      };

      // Set source to trigger load
      this.#imageElement!.src = src;
    });
  }

  /**
   * Toggle between fit-to-window and actual-size display modes.
   */
  toggleFitMode(): void {
    if (this.#fitMode === "fit") {
      this.#fitMode = "actual";
      // Reset to 1:1 display
      this.state.scale = 1.0;
      this.state.offsetX = 0;
      this.state.offsetY = 0;
    } else {
      this.#fitMode = "fit";
      this.fitToWindow();
    }
    this.updateTransform();
    this.updateZoomDisplay();
  }

  /**
   * Get the current fit mode.
   */
  getFitMode(): "fit" | "actual" {
    return this.#fitMode;
  }

  protected getContentDimensions(): { width: number; height: number } {
    return {
      width: Math.max(this.#naturalWidth, 1),
      height: Math.max(this.#naturalHeight, 1),
    };
  }

  protected updateTransform(animate = false): void {
    if (!this.#imageWrapper || !this.#imageContainer) return;

    if (animate) {
      this.#imageWrapper.style.transition = "transform 0.3s ease-out";
      this.#imageContainer.style.transition = "zoom 0.3s ease-out";
    } else {
      this.#imageWrapper.style.transition = "none";
      this.#imageContainer.style.transition = "none";
    }

    // Separate zoom and translate to avoid coordinate space issues
    // wrapper handles position (translate)
    this.#imageWrapper.style.transform = `translate(${this.state.offsetX}px, ${this.state.offsetY}px)`;
    // inner container handles zoom
    this.#imageContainer.style.zoom = String(this.state.scale);
  }

  protected updateZoomDisplay(): void {
    // Update zoom level display via dioxus bridge
    const zoomPercent = Math.round(this.state.scale * 100);

    // Call global function to update Rust state
    window.updateZoomLevel(zoomPercent);
  }

  protected override fitToWindow(): void {
    // Get the canvas container (parent of wrapper) to get available dimensions
    const canvas = document.getElementById("image-window-canvas");
    if (!canvas || this.#fitMode === "actual") return;

    const padding = 40;
    const availableWidth = Math.max(canvas.clientWidth - padding * 2, 1);
    const availableHeight = Math.max(canvas.clientHeight - padding * 2, 1);

    const scaleX = availableWidth / this.#naturalWidth;
    const scaleY = availableHeight / this.#naturalHeight;
    const scale = Math.max(0.1, Math.min(scaleX, scaleY, 1));

    this.state.scale = scale;

    const scaledWidth = this.#naturalWidth * scale;
    const scaledHeight = this.#naturalHeight * scale;

    this.state.offsetX = (canvas.clientWidth - scaledWidth) / 2;
    this.state.offsetY = (canvas.clientHeight - scaledHeight) / 2;

    this.updateTransform(false);
    this.updateZoomDisplay();
  }
}

// Global instance
let controller: ImageWindowController | null = null;

declare global {
  interface Window {
    updateZoomLevel: (zoomPercent: number) => void;
    imageWindowController?: ImageWindowController;
  }
}

/**
 * Initialize the Image window from Rust side.
 */
export async function initImageWindow(src: string, imageId: string): Promise<void> {
  controller = new ImageWindowController();
  await controller.init(src, imageId);

  // Expose globally for Rust to call
  window.imageWindowController = controller;
}

/**
 * Toggle between fit and actual size modes.
 */
export function toggleImageFitMode(): void {
  if (controller) {
    controller.toggleFitMode();
  }
}

/**
 * Get current image dimensions.
 */
export function getImageDimensions(): { width: number; height: number; mode: "fit" | "actual" } {
  if (!controller) {
    return { width: 0, height: 0, mode: "fit" };
  }
  const dims = controller.getContentDimensions();
  return {
    width: dims.width,
    height: dims.height,
    mode: controller.getFitMode(),
  };
}

/**
 * Copy the image to the clipboard.
 */
export async function copyImageToClipboard(): Promise<void> {
  const container = document.getElementById("image-container");
  if (!container) {
    throw new Error("image-container element not found");
  }

  const image = container.querySelector("img") as HTMLImageElement;
  if (!image) {
    throw new Error("img element not found");
  }

  try {
    // Draw image at its natural size to avoid OOM when zoomed in.
    // html2canvas would rasterize the zoomed container, creating huge canvases.
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;

    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("Failed to get canvas 2d context");
    }

    // Fill with background color for transparent images
    const bgColor =
      getComputedStyle(document.body).getPropertyValue("--bg-color").trim() || "#ffffff";
    ctx.fillStyle = bgColor;
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    ctx.drawImage(image, 0, 0);

    // Convert canvas to data URL
    const dataUrl = canvas.toDataURL("image/png");

    // Send to Rust using the standard rustCopyImage handler
    if (window.rustCopyImage) {
      window.rustCopyImage(dataUrl);
    } else {
      throw new Error("Rust clipboard handler not available");
    }
  } catch (error) {
    console.error("Failed to copy image:", error);
    throw error;
  }
}

// Sync data-theme attribute when Rust dispatches theme changes
import { setupBodyThemeSync } from "./theme";
setupBodyThemeSync();
