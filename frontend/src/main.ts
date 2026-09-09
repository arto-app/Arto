import "../style/main.css";

import { applyPictureTheme } from "./picture-theme";
import { refreshReadingPosition, setupReadingPosition } from "./reading-position";
import { setupRowHover } from "./row-hover";
import { setupScrollbarReach } from "./scrollbar-reach";
import { type Theme, currentTheme, isDarkTheme, themedElement } from "./theme";
import * as mermaidRenderer from "./mermaid-renderer";
import { renderCoordinator } from "./render-coordinator";
import {
  setup as setupContextMenu,
  restoreSelection,
  cleanupElementReferences,
  getSavedMermaidElement,
  getSavedMathElement,
} from "./context-menu-handler";
import { rasterizeMathBlock, rasterizeMermaidBlock } from "./special-block-rasterizer";
import * as findInPage from "./find-in-page";
import * as keyboardInterceptor from "./keyboard-interceptor";
import * as scrollController from "./scroll-controller";
import * as contentCursor from "./content-cursor";
import * as actionFeedback from "./action-feedback";
import * as viewportQueue from "./viewport-queue";
import * as scrollAnchor from "./scroll-anchor";
import type { ScrollAnchor } from "./scroll-anchor";

// Declare global Arto namespace
declare global {
  interface Window {
    Arto: {
      contextMenu: {
        setup: typeof setupContextMenu;
        restoreSelection: typeof restoreSelection;
        /** Cleanup saved element references when context menu closes. */
        cleanup: typeof cleanupElementReferences;
      };
      render: {
        /** Register a callback to be called when rendering (Mermaid, KaTeX, etc.) completes */
        onComplete: (callback: () => void) => void;
        /** Force a render pass for any content already in the DOM */
        schedule: () => void;
      };
      rasterize: {
        /** Rasterize an image to a PNG data URL via Canvas.
         *  SVG images are rendered at 2x scale for Retina quality.
         *  Raster images use 1x scale to preserve original resolution. */
        image: (src: string, opaque: boolean) => Promise<string | null>;
        /** Rasterize a Math block (KaTeX) to PNG data URL via html2canvas. */
        mathBlock: (opaque: boolean) => Promise<string | null>;
        /** Rasterize a Mermaid SVG to PNG data URL. */
        mermaidBlock: (opaque: boolean) => Promise<string | null>;
        /** Rasterize a specific Math block element to PNG data URL. */
        mathElement: (element: HTMLElement, opaque: boolean) => Promise<string | null>;
        /** Rasterize a specific Mermaid block element to PNG data URL. */
        mermaidElement: (element: HTMLElement, opaque: boolean) => Promise<string | null>;
      };
      search: {
        setup: typeof findInPage.setup;
        find: typeof findInPage.find;
        navigate: typeof findInPage.navigate;
        navigateTo: typeof findInPage.navigateTo;
        clear: typeof findInPage.clear;
        reapply: typeof findInPage.reapply;
        setPinned: typeof findInPage.setPinned;
        scrollToPinnedMatch: typeof findInPage.scrollToPinnedMatch;
      };
      keyboard: {
        onKeydown: typeof keyboardInterceptor.onKeydown;
        pause: typeof keyboardInterceptor.pause;
        resume: typeof keyboardInterceptor.resume;
        setMenuAccelerators: typeof keyboardInterceptor.setMenuAccelerators;
        setReservedKeyOverrides: typeof keyboardInterceptor.setReservedKeyOverrides;
      };
      scroll: {
        down: typeof scrollController.down;
        up: typeof scrollController.up;
        pageDown: typeof scrollController.pageDown;
        pageUp: typeof scrollController.pageUp;
        halfPageDown: typeof scrollController.halfPageDown;
        halfPageUp: typeof scrollController.halfPageUp;
        /**
         * Where the reader is, as a value that survives the document
         * changing height.
         */
        anchor: () => ScrollAnchor;
        // Destinations. Each is held until the document stops moving under
        // it, so that a diagram drawn on arrival does not carry the reader
        // past the place they asked for.
        toTop: typeof scrollController.toTop;
        toBottom: typeof scrollController.toBottom;
        toHeading: typeof scrollController.toHeading;
        /** Put the reader back where `anchor` says they were. */
        toAnchor: (anchor: ScrollAnchor | null) => void;
        /** Jump to the top for a document that has just been replaced. */
        reset: typeof scrollController.reset;
      };
      contentCursor: {
        next: typeof contentCursor.next;
        prev: typeof contentCursor.prev;
        nextHeading: typeof contentCursor.nextHeading;
        prevHeading: typeof contentCursor.prevHeading;
        setFromContextTarget: typeof contentCursor.setFromContextTarget;
        show: typeof contentCursor.show;
        clearCursor: typeof contentCursor.clearCursor;
        clearCursorDeferred: typeof contentCursor.clearCursorDeferred;
        syncToViewport: typeof contentCursor.syncToViewport;
        getCodeText: typeof contentCursor.getCodeText;
        getCodeAsMarkdown: typeof contentCursor.getCodeAsMarkdown;
        getTableAsTsv: typeof contentCursor.getTableAsTsv;
        getTableAsCsv: typeof contentCursor.getTableAsCsv;
        getTableAsMarkdown: typeof contentCursor.getTableAsMarkdown;
        getImageSrc: typeof contentCursor.getImageSrc;
        getImageAsMarkdown: typeof contentCursor.getImageAsMarkdown;
        getLinkHref: typeof contentCursor.getLinkHref;
        getSourceLineRange: typeof contentCursor.getSourceLineRange;
        getCurrentElement: typeof contentCursor.getCurrentElement;
      };
      readingPosition: {
        /**
         * Measure again where the reader is and what is set beside the page.
         *
         * A scroll and a taller document ask for this on their own. Zoom does
         * not: it changes how wide the page is drawn without changing the
         * window or the page's own layout size, so nothing observes it — and
         * the margin trace is placed against a margin that has just moved.
         */
        refresh: typeof refreshReadingPosition;
      };
      feedback: {
        show: typeof actionFeedback.show;
      };
      print: {
        /** Switch to the light theme for printing; resolves after Mermaid re-renders. */
        prepare: () => Promise<void>;
        /**
         * Draw everything the reader never scrolled to, and nothing else.
         *
         * What [`prepare`] does minus the theme switch, for the platforms
         * that cannot switch the theme because their print call gives no
         * completion signal to restore it after.
         */
        draw: () => Promise<void>;
        /** Restore the theme that was active before `prepare()`. */
        restore: () => void;
      };
    };
    /** Called from JavaScript when Math block click is detected */
    handleMathWindowOpen?: (source: string) => void;
    /** Called from JavaScript when Mermaid block click is detected */
    handleMermaidWindowOpen?: (source: string) => void;
    /** Called from JavaScript when Image block click is detected */
    handleImageWindowOpen?: (src: string, alt: string | null) => void;
  }
}

/**
 * Switch the theme.
 *
 * Returns once the diagrams have been queued again in the new theme — the
 * print path waits on that before draining the queue, everything else can
 * ignore it.
 */
export function setCurrentTheme(theme: Theme): Promise<void> {
  themedElement().setAttribute("data-theme", theme);
  // The stylesheet repaints on the attribute alone. Mermaid has to be told,
  // because its colours are baked into the SVG it already drew, and so do
  // theme-aware pictures, whose media query answers for the system rather
  // than for the theme the reader chose here.
  mermaidRenderer.setTheme(theme);
  applyPictureTheme();
  return renderCoordinator.forceRenderMermaid();
}

/**
 * Theme that was active before `preparePrint()` switched to light,
 * or null when no print is in progress (or the theme was already light).
 */
let printSavedTheme: Theme | null = null;

/**
 * Force the light theme for printing.
 *
 * Printed output must always be light-on-white regardless of the UI theme.
 * The print CSS can override plain colors, but Mermaid bakes its theme into
 * the generated SVG, so the diagrams have to be re-rendered with the light
 * theme before the print dialog captures the page. Resolves once that
 * re-render completes (with a timeout fallback so printing never hangs).
 *
 * A print job also has no reader and no scrolling, so everything the reader
 * never scrolled to has to be rendered first. This is the only place that
 * can do it: the caller awaits this promise before opening the dialog,
 * whereas a `beforeprint` listener cannot delay the capture.
 */
async function preparePrint(): Promise<void> {
  if (!isDarkTheme(currentTheme())) {
    await viewportQueue.flush();
    return;
  }
  printSavedTheme = currentTheme();

  // Flushing before the switch would draw every diagram in the dark theme
  // only for the switch to throw it away, so the queue is drained once, after
  // the theme is already light. A diagram still waiting in the queue renders
  // from the Mermaid config current when its job runs, which is the light one
  // by then.
  //
  // Waiting on the switch itself rather than on a render-complete callback:
  // the switch clears the drawn diagrams and queues them again, and draining
  // before that has happened would drain them as nothing.
  await setCurrentTheme("light");
  await viewportQueue.flush();
}

/** Restore the theme that was active before `preparePrint()`. */
function restorePrint(): void {
  if (printSavedTheme === null) {
    return;
  }
  setCurrentTheme(printSavedTheme);
  printSavedTheme = null;
}

export function init(): void {
  mermaidRenderer.init();
  renderCoordinator.init();

  // The header's line and the gutter's current tick both answer to where the
  // reader is, and a new document moves them without a scroll happening. The
  // callback fires once, so it re-arms itself for the render after this one.
  setupReadingPosition();
  // The full name of a row the panel had to cut, floating clear of the box
  // that scrolls it.
  setupRowHover();
  // The scrollbar, brought up to a native width by the pointer arriving at
  // the edge it is on.
  setupScrollbarReach();
  const trackAfterRender = (): void => {
    refreshReadingPosition();
    renderCoordinator.onRenderComplete(trackAfterRender);
  };
  renderCoordinator.onRenderComplete(trackAfterRender);

  // A page with no `.content` is one `arto page` wrote: a whole document,
  // which its reader can print with the browser's own command. Nothing can
  // delay that capture, so the rest of it is drawn in idle time instead of
  // waiting for a scroll that may never come. See `backfillWhenIdle`.
  if (!document.querySelector(".content")) {
    viewportQueue.backfillWhenIdle();
  }

  // Expose Arto API on window for Rust interop
  window.Arto = {
    contextMenu: {
      setup: setupContextMenu,
      restoreSelection,
      cleanup: cleanupElementReferences,
    },
    render: {
      onComplete: (callback) => renderCoordinator.onRenderComplete(callback),
      schedule: () => renderCoordinator.scheduleRender(),
    },
    rasterize: {
      image: (src: string, opaque: boolean): Promise<string | null> => {
        // A data URL runs to megabytes; the console only needs enough of it
        // to tell one image from another.
        const shortSrc = src.length > 120 ? `${src.slice(0, 120)}…` : src;
        return new Promise((resolve) => {
          const img = new Image();
          // No `crossOrigin` here: a custom scheme does not take part in CORS
          // in every engine, and asking for it is enough to make the load
          // itself fail. Whatever calls this hands over a `data:` URL when the
          // image is not the document's own — see `copy_image_from_src`.
          img.onload = () => {
            try {
              // SVG: 2x for Retina (vector scales perfectly)
              // Raster: 1x to preserve original resolution
              const isSvg = src.startsWith("data:image/svg") || src.endsWith(".svg");
              const scale = isSvg ? 2 : 1;
              const maxDimension = 16384;
              // ~256 MP limit prevents excessive memory allocation
              // (each pixel = 4 bytes RGBA, so 256M * 4 = ~1 GB max)
              const maxPixels = 256_000_000;
              const scaledWidth = img.naturalWidth * scale;
              const scaledHeight = img.naturalHeight * scale;
              if (
                scaledWidth > maxDimension ||
                scaledHeight > maxDimension ||
                scaledWidth * scaledHeight > maxPixels
              ) {
                console.error(
                  `Image too large to rasterize: ${scaledWidth}x${scaledHeight}`,
                  shortSrc,
                );
                resolve(null);
                return;
              }
              const canvas = document.createElement("canvas");
              canvas.width = scaledWidth;
              canvas.height = scaledHeight;
              const ctx = canvas.getContext("2d");
              if (!ctx) {
                console.error("Failed to get 2D canvas context");
                resolve(null);
                return;
              }
              ctx.scale(scale, scale);
              if (opaque) {
                const bgColor =
                  getComputedStyle(document.body).getPropertyValue("--bg-color").trim() ||
                  "#ffffff";
                ctx.fillStyle = bgColor;
                ctx.fillRect(0, 0, img.naturalWidth, img.naturalHeight);
              }
              ctx.drawImage(img, 0, 0);
              resolve(canvas.toDataURL("image/png"));
            } catch (e) {
              console.error("Failed to rasterize image:", shortSrc, e);
              resolve(null);
            }
          };
          img.onerror = () => {
            console.error("Failed to load image for rasterization:", shortSrc);
            resolve(null);
          };
          img.src = src;
        });
      },
      mathBlock: async (opaque: boolean): Promise<string | null> => {
        const element = getSavedMathElement();
        if (!element) {
          console.error("No saved Math element found");
          return null;
        }
        return rasterizeMathBlock(element, opaque);
      },
      mermaidBlock: async (opaque: boolean): Promise<string | null> => {
        const element = getSavedMermaidElement();
        if (!element) {
          console.error("No saved Mermaid element found");
          return null;
        }
        return rasterizeMermaidBlock(element, opaque);
      },
      mathElement: async (element: HTMLElement, opaque: boolean): Promise<string | null> => {
        return rasterizeMathBlock(element, opaque);
      },
      mermaidElement: async (element: HTMLElement, opaque: boolean): Promise<string | null> => {
        return rasterizeMermaidBlock(element, opaque);
      },
    },
    search: {
      setup: findInPage.setup,
      find: findInPage.find,
      navigate: findInPage.navigate,
      navigateTo: findInPage.navigateTo,
      clear: findInPage.clear,
      reapply: findInPage.reapply,
      setPinned: findInPage.setPinned,
      scrollToPinnedMatch: findInPage.scrollToPinnedMatch,
    },
    keyboard: {
      onKeydown: keyboardInterceptor.onKeydown,
      pause: keyboardInterceptor.pause,
      resume: keyboardInterceptor.resume,
      setMenuAccelerators: keyboardInterceptor.setMenuAccelerators,
      setReservedKeyOverrides: keyboardInterceptor.setReservedKeyOverrides,
    },
    scroll: {
      down: scrollController.down,
      up: scrollController.up,
      pageDown: scrollController.pageDown,
      pageUp: scrollController.pageUp,
      halfPageDown: scrollController.halfPageDown,
      halfPageUp: scrollController.halfPageUp,
      anchor: scrollAnchor.currentAnchor,
      toTop: scrollController.toTop,
      toBottom: scrollController.toBottom,
      toHeading: scrollController.toHeading,
      toAnchor: scrollAnchor.toAnchor,
      reset: scrollController.reset,
    },
    contentCursor: {
      next: contentCursor.next,
      prev: contentCursor.prev,
      nextHeading: contentCursor.nextHeading,
      prevHeading: contentCursor.prevHeading,
      setFromContextTarget: contentCursor.setFromContextTarget,
      show: contentCursor.show,
      clearCursor: contentCursor.clearCursor,
      clearCursorDeferred: contentCursor.clearCursorDeferred,
      syncToViewport: contentCursor.syncToViewport,
      getCodeText: contentCursor.getCodeText,
      getCodeAsMarkdown: contentCursor.getCodeAsMarkdown,
      getTableAsTsv: contentCursor.getTableAsTsv,
      getTableAsCsv: contentCursor.getTableAsCsv,
      getTableAsMarkdown: contentCursor.getTableAsMarkdown,
      getImageSrc: contentCursor.getImageSrc,
      getImageAsMarkdown: contentCursor.getImageAsMarkdown,
      getLinkHref: contentCursor.getLinkHref,
      getSourceLineRange: contentCursor.getSourceLineRange,
      getCurrentElement: contentCursor.getCurrentElement,
    },
    readingPosition: {
      refresh: refreshReadingPosition,
    },
    feedback: {
      show: actionFeedback.show,
    },
    print: {
      prepare: preparePrint,
      draw: () => viewportQueue.flush(),
      restore: restorePrint,
    },
  };

  // Set up keyboard interceptor event listeners
  keyboardInterceptor.setup();

  // Listen for theme changes from Rust
  document.addEventListener("arto:theme-changed", ((event: CustomEvent) => {
    setCurrentTheme(event.detail);
  }) as EventListener);

  // Set initial theme
  setCurrentTheme(currentTheme());
}

// Re-export special window functions
export { initMermaidWindow } from "./mermaid-window-controller";
export { initMathWindow, setMathTheme, copyMathAsImage } from "./math-window-controller";
export {
  initImageWindow,
  toggleImageFitMode,
  getImageDimensions,
  copyImageToClipboard,
} from "./image-window-controller";
