import "../style/main.css";

import { type Theme, getSystemTheme } from "./theme";
import * as markdownViewer from "./markdown-viewer";
import * as syntaxHighlighter from "./syntax-highlighter";
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
        scrollDown: typeof scrollController.scrollDown;
        scrollUp: typeof scrollController.scrollUp;
        scrollPageDown: typeof scrollController.scrollPageDown;
        scrollPageUp: typeof scrollController.scrollPageUp;
        scrollHalfPageDown: typeof scrollController.scrollHalfPageDown;
        scrollHalfPageUp: typeof scrollController.scrollHalfPageUp;
        scrollToTop: typeof scrollController.scrollToTop;
        scrollToBottom: typeof scrollController.scrollToBottom;
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
      feedback: {
        show: typeof actionFeedback.show;
      };
      print: {
        /** Switch to the light theme for printing; resolves after Mermaid re-renders. */
        prepare: () => Promise<void>;
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

function getCurrentTheme(): Theme {
  const theme = document.body.getAttribute("data-theme");
  switch (theme) {
    case "light":
    case "dark":
      return theme;
    default:
      return getSystemTheme();
  }
}

export function setCurrentTheme(theme: Theme) {
  document.body.setAttribute("data-theme", theme);
  markdownViewer.setTheme(theme);
  syntaxHighlighter.setTheme(theme);
  mermaidRenderer.setTheme(theme);
  renderCoordinator.forceRenderMermaid();
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
 */
async function preparePrint(): Promise<void> {
  if (getCurrentTheme() === "light") {
    return;
  }
  printSavedTheme = getCurrentTheme();

  const rendered = new Promise<void>((resolve) => {
    renderCoordinator.onRenderComplete(resolve);
    setTimeout(resolve, 2000);
  });
  setCurrentTheme("light");
  await rendered;
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
  markdownViewer.mount();
  syntaxHighlighter.mount();
  mermaidRenderer.init();
  renderCoordinator.init();

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
        return new Promise((resolve) => {
          const img = new Image();
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
                console.error(`Image too large to rasterize: ${scaledWidth}x${scaledHeight}`);
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
              console.error("Failed to rasterize image:", e);
              resolve(null);
            }
          };
          img.onerror = () => resolve(null);
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
      scrollDown: scrollController.scrollDown,
      scrollUp: scrollController.scrollUp,
      scrollPageDown: scrollController.scrollPageDown,
      scrollPageUp: scrollController.scrollPageUp,
      scrollHalfPageDown: scrollController.scrollHalfPageDown,
      scrollHalfPageUp: scrollController.scrollHalfPageUp,
      scrollToTop: scrollController.scrollToTop,
      scrollToBottom: scrollController.scrollToBottom,
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
    feedback: {
      show: actionFeedback.show,
    },
    print: {
      prepare: preparePrint,
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
  setCurrentTheme(getCurrentTheme());
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
