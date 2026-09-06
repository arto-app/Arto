/**
 * Base viewer controller with common zoom/pan operations.
 * Provides a foundation for specialized viewers like Mermaid, Math, and Image windows.
 */

interface ViewerState {
  scale: number;
  offsetX: number;
  offsetY: number;
  isDragging: boolean;
  lastMouseX: number;
  lastMouseY: number;
}

export abstract class BaseViewerController {
  protected container: HTMLElement | null = null;
  protected wrapper: HTMLElement | null = null;
  protected contentContainer: HTMLElement | null = null;
  protected maxZoom: number = 100.0;
  protected state: ViewerState = {
    scale: 1.0,
    offsetX: 0,
    offsetY: 0,
    isDragging: false,
    lastMouseX: 0,
    lastMouseY: 0,
  };

  // Store bound handlers so the same references are used in add/removeEventListener
  private boundHandleKeyDown = this.handleKeyDown.bind(this);
  private boundHandleMouseDown = this.handleMouseDown.bind(this);
  private boundHandleMouseMove = this.handleMouseMove.bind(this);
  private boundHandleMouseUp = this.handleMouseUp.bind(this);
  private boundHandleWheel = this.handleWheel.bind(this);
  private boundHandleDoubleClick = this.handleDoubleClick.bind(this);

  constructor(containerId: string, maxZoom?: number) {
    this.container = document.getElementById(containerId);
    if (!this.container) {
      throw new Error(`Container element not found: ${containerId}`);
    }
    if (maxZoom) {
      this.maxZoom = maxZoom;
    }
  }

  /**
   * Set up event listeners for zoom/pan operations.
   * Call this method after initializing content-specific handlers.
   */
  protected setupEventListeners(): void {
    if (!this.container) return;

    // Keyboard shortcuts
    document.addEventListener("keydown", this.boundHandleKeyDown);

    // Mouse events for dragging
    this.container.addEventListener("mousedown", this.boundHandleMouseDown);
    document.addEventListener("mousemove", this.boundHandleMouseMove);
    document.addEventListener("mouseup", this.boundHandleMouseUp);

    // Scroll events
    this.container.addEventListener("wheel", this.boundHandleWheel, {
      passive: false,
    });

    // Double-click to fit
    this.container.addEventListener("dblclick", this.boundHandleDoubleClick);
  }

  /**
   * Get content dimensions (width, height).
   * Must be implemented by subclasses to provide content-specific sizing.
   */
  protected abstract getContentDimensions(): { width: number; height: number };

  /**
   * Update transform (zoom and translate) on the DOM elements.
   * Must be implemented by subclasses for content-specific transform logic.
   */
  protected abstract updateTransform(animate?: boolean): void;

  /**
   * Update zoom display UI (percentage indicator).
   * Must be implemented by subclasses.
   */
  protected abstract updateZoomDisplay(): void;

  // ============ Event Handlers ============

  protected handleKeyDown(event: KeyboardEvent): void {
    const isCmdOrCtrl = event.metaKey || event.ctrlKey;

    if (isCmdOrCtrl) {
      if (event.key === "=" || event.key === "+") {
        event.preventDefault();
        this.zoom(0.1);
      } else if (event.key === "-") {
        event.preventDefault();
        this.zoom(-0.1);
      } else if (event.key === "0") {
        event.preventDefault();
        this.fitToWindow();
      }
    }
  }

  protected handleMouseDown(event: MouseEvent): void {
    if (event.button === 0) {
      // Left click
      this.state.isDragging = true;
      this.state.lastMouseX = event.clientX;
      this.state.lastMouseY = event.clientY;
      if (this.container) {
        this.container.style.cursor = "grabbing";
      }
    }
  }

  protected handleMouseMove(event: MouseEvent): void {
    if (this.state.isDragging) {
      const dx = event.clientX - this.state.lastMouseX;
      const dy = event.clientY - this.state.lastMouseY;

      this.state.offsetX += dx;
      this.state.offsetY += dy;

      this.state.lastMouseX = event.clientX;
      this.state.lastMouseY = event.clientY;

      this.updateTransform();
    }
  }

  protected handleMouseUp(): void {
    this.state.isDragging = false;
    if (this.container) {
      this.container.style.cursor = "grab";
    }
  }

  protected handleWheel(event: WheelEvent): void {
    event.preventDefault();

    const deltaScale = this.getDeltaModeScale(event.deltaMode);
    const deltaY = event.deltaY * deltaScale;
    const ZOOM_SCALE = 0.01;
    const zoomFactor = Math.exp(-deltaY * ZOOM_SCALE);

    const oldScale = this.state.scale;
    const newScale = Math.max(0.1, Math.min(this.maxZoom, oldScale * zoomFactor));

    if (newScale !== oldScale) {
      const rect = this.container!.getBoundingClientRect();
      const mouseX = event.clientX - rect.left;
      const mouseY = event.clientY - rect.top;

      const diagramX = (mouseX - this.state.offsetX) / oldScale;
      const diagramY = (mouseY - this.state.offsetY) / oldScale;

      this.state.offsetX = mouseX - diagramX * newScale;
      this.state.offsetY = mouseY - diagramY * newScale;
      this.state.scale = newScale;

      this.updateTransform();
      this.updateZoomDisplay();
    }
  }

  protected handleDoubleClick(): void {
    this.fitToWindow();
  }

  // ============ Zoom/Pan Operations ============

  protected zoom(delta: number): void {
    const newScale = Math.max(0.1, Math.min(this.maxZoom, this.state.scale + delta));

    if (this.container) {
      const centerX = this.container.clientWidth / 2;
      const centerY = this.container.clientHeight / 2;

      const oldScale = this.state.scale;
      const scaleRatio = newScale / oldScale;

      this.state.offsetX = centerX - (centerX - this.state.offsetX) * scaleRatio;
      this.state.offsetY = centerY - (centerY - this.state.offsetY) * scaleRatio;
    }

    this.state.scale = newScale;
    this.updateTransform();
    this.updateZoomDisplay();
  }

  protected fitToWindow(): void {
    if (!this.container) return;

    const dims = this.getContentDimensions();
    const padding = 40;

    const availableWidth = Math.max(this.container.clientWidth - padding * 2, 1);
    const availableHeight = Math.max(this.container.clientHeight - padding * 2, 1);

    const scaleX = availableWidth / dims.width;
    const scaleY = availableHeight / dims.height;
    const scale = Math.max(0.1, Math.min(scaleX, scaleY, this.maxZoom));

    this.state.scale = scale;

    const scaledWidth = dims.width * scale;
    const scaledHeight = dims.height * scale;

    this.state.offsetX = (this.container.clientWidth - scaledWidth) / 2;
    this.state.offsetY = (this.container.clientHeight - scaledHeight) / 2;

    this.updateTransform(false);
    this.updateZoomDisplay();
  }

  protected getDeltaModeScale(deltaMode: number): number {
    switch (deltaMode) {
      case WheelEvent.DOM_DELTA_PIXEL:
        return 1;
      case WheelEvent.DOM_DELTA_LINE:
        return 10;
      case WheelEvent.DOM_DELTA_PAGE:
        return 20;
      default:
        return 1;
    }
  }

  // ============ Utility ============

  /**
   * Display a rendering error message in the given container.
   * Replaces container children with a styled error block.
   */
  protected static showRenderError(container: HTMLElement, error: unknown): void {
    const errorDiv = document.createElement("div");
    errorDiv.style.cssText = "color: red; padding: 2rem;";
    const strong = document.createElement("strong");
    strong.textContent = "Rendering Error:";
    const pre = document.createElement("pre");
    pre.style.whiteSpace = "pre-wrap";
    pre.textContent = String(error);
    errorDiv.append(strong, document.createElement("br"), pre);
    container.replaceChildren(errorDiv);
  }

  /**
   * Cleanup event listeners. Call in component cleanup/destroy.
   */
  destroy(): void {
    document.removeEventListener("keydown", this.boundHandleKeyDown);
    document.removeEventListener("mousemove", this.boundHandleMouseMove);
    document.removeEventListener("mouseup", this.boundHandleMouseUp);
    if (this.container) {
      this.container.removeEventListener("mousedown", this.boundHandleMouseDown);
      this.container.removeEventListener("wheel", this.boundHandleWheel);
      this.container.removeEventListener("dblclick", this.boundHandleDoubleClick);
    }
  }
}
