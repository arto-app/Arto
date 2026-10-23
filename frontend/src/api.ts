import { refreshReadingPosition } from "./reading-position";
import { renderCoordinator } from "./render-coordinator";
import {
  setup as setupContextMenu,
  restoreSelection,
  cleanupElementReferences,
  getSavedMermaidElement,
  getSavedMathElement,
  getSavedRange,
} from "./context-menu-handler";
import { fitSubmenu } from "./submenu-fit";
import { rasterizeMathBlock, rasterizeMermaidBlock } from "./special-block-rasterizer";
import * as findInPage from "./find-in-page";
import * as keyboardInterceptor from "./keyboard-interceptor";
import * as mouseNavigation from "./mouse-navigation";
import * as scrollController from "./scroll-controller";
import * as contentCursor from "./content-cursor";
import * as actionFeedback from "./action-feedback";
import * as lenses from "./lenses";
import * as changes from "./changes";
import * as userHighlights from "./user-highlights";
import * as linkPreview from "./link-preview";
import * as viewportQueue from "./viewport-queue";
import * as scrollAnchor from "./scroll-anchor";
import * as stickyTableHead from "./sticky-table-head";
import { preparePrint, restorePrint } from "./theme-switch";
import type { ScrollAnchor } from "./scroll-anchor";

/** What the runtime publishes on `window.Arto` for the host to call into. */
export interface ArtoApi {
  contextMenu: {
    setup: typeof setupContextMenu;
    restoreSelection: typeof restoreSelection;
    /** Cleanup saved element references when context menu closes. */
    cleanup: typeof cleanupElementReferences;
    /** Keep a submenu's flyout on screen once it is drawn. */
    fitSubmenu: typeof fitSubmenu;
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
  highlights: {
    /**
     * Hear what the page found each time highlights are drawn, and which
     * one the reader clicked to open.
     */
    setup: typeof userHighlights.setup;
    /** Draw a document's highlights on the page that now holds it. */
    show: typeof userHighlights.show;
    /** Name the selected place, for a highlight to be made of it. */
    describeSelection: () => ReturnType<typeof userHighlights.describeSelection>;
    /**
     * The same, for an item of the context menu: the selection the menu
     * opened on stands in when picking the item took the selection away.
     */
    describeMenuSelection: () => ReturnType<typeof userHighlights.describeSelection>;
    /** The highlights the selection touches. */
    idsAtSelection: () => string[];
    scrollTo: typeof userHighlights.scrollTo;
    /** Where a highlight is drawn, for something placed beside it. */
    rectOf: typeof userHighlights.rectOf;
    /** Scroll to a highlight and say where it is once it has landed. */
    reveal: typeof userHighlights.reveal;
  };
  keyboard: {
    onKeydown: typeof keyboardInterceptor.onKeydown;
    pause: typeof keyboardInterceptor.pause;
    resume: typeof keyboardInterceptor.resume;
    setMenuAccelerators: typeof keyboardInterceptor.setMenuAccelerators;
    setReservedKeyOverrides: typeof keyboardInterceptor.setReservedKeyOverrides;
  };
  mouse: {
    /** Register a callback for the mouse's back / forward side buttons. */
    onNavigate: typeof mouseNavigation.onNavigate;
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
  stickyTableHead: {
    /**
     * Decide again which tables keep their header in view. Zoom changes
     * how tall a table is drawn without anything observing it.
     */
    refresh: typeof stickyTableHead.refresh;
  };
  feedback: {
    show: typeof actionFeedback.show;
  };
  /** What changed since the document was last read, marked on the page. */
  changes: {
    set: typeof changes.set;
    clear: typeof changes.clear;
    next: typeof changes.next;
    prev: typeof changes.prev;
    first: typeof changes.first;
  };
  lenses: {
    collect: typeof lenses.collect;
    markPending: typeof lenses.markPending;
    annotate: typeof lenses.annotate;
    annotateAll: typeof lenses.annotateAll;
    fail: typeof lenses.fail;
    settle: typeof lenses.settle;
    restorePage: typeof lenses.restorePage;
    restoreMarks: typeof lenses.restoreMarks;
    beginPage: typeof lenses.beginPage;
    showPage: typeof lenses.showPage;
    showBlock: typeof lenses.showBlock;
    showAnswers: typeof lenses.showAnswers;
  };
  linkPreview: {
    /** Register how another document is asked of the app. */
    onRequest: typeof linkPreview.onRequest;
    /** The app's answer to a request: the document rendered, or null. */
    resolve: typeof linkPreview.resolve;
    /** Preview the link under the keyboard cursor. */
    showAtCursor: typeof linkPreview.showAtCursor;
    hide: typeof linkPreview.hide;
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
}

declare global {
  interface Window {
    Arto: ArtoApi;
    /** Called from JavaScript when Math block click is detected */
    handleMathWindowOpen?: (source: string) => void;
    /** Called from JavaScript when Mermaid block click is detected */
    handleMermaidWindowOpen?: (source: string) => void;
    /** Called from JavaScript when Image block click is detected */
    handleImageWindowOpen?: (src: string, alt: string | null) => void;
    /** Follow a document link as written; installed by the app's file viewer. */
    handleMarkdownLinkClick?: (path: string, button: number) => void;
  }
}

export function createArtoApi(): ArtoApi {
  return {
    contextMenu: {
      setup: setupContextMenu,
      restoreSelection,
      cleanup: cleanupElementReferences,
      fitSubmenu,
    },
    render: {
      onComplete: (callback) => renderCoordinator.onRenderComplete(callback),
      schedule: () => renderCoordinator.scheduleRender(),
    },
    rasterize: {
      image: rasterizeImage,
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
    highlights: {
      setup: userHighlights.setup,
      show: userHighlights.show,
      describeSelection: () => userHighlights.describeSelection(),
      describeMenuSelection: () => userHighlights.describeSelection(getSavedRange()),
      idsAtSelection: () => userHighlights.idsAtSelection(),
      scrollTo: userHighlights.scrollTo,
      rectOf: userHighlights.rectOf,
      reveal: userHighlights.reveal,
    },
    keyboard: {
      onKeydown: keyboardInterceptor.onKeydown,
      pause: keyboardInterceptor.pause,
      resume: keyboardInterceptor.resume,
      setMenuAccelerators: keyboardInterceptor.setMenuAccelerators,
      setReservedKeyOverrides: keyboardInterceptor.setReservedKeyOverrides,
    },
    mouse: {
      onNavigate: mouseNavigation.onNavigate,
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
    stickyTableHead: {
      refresh: stickyTableHead.refresh,
    },
    feedback: {
      show: actionFeedback.show,
    },
    changes: {
      set: changes.set,
      clear: changes.clear,
      next: changes.next,
      prev: changes.prev,
      first: changes.first,
    },
    lenses: {
      collect: lenses.collect,
      markPending: lenses.markPending,
      annotate: lenses.annotate,
      annotateAll: lenses.annotateAll,
      fail: lenses.fail,
      settle: lenses.settle,
      restorePage: lenses.restorePage,
      restoreMarks: lenses.restoreMarks,
      beginPage: lenses.beginPage,
      showPage: lenses.showPage,
      showBlock: lenses.showBlock,
      showAnswers: lenses.showAnswers,
    },
    linkPreview: {
      onRequest: linkPreview.onRequest,
      resolve: linkPreview.resolve,
      showAtCursor: linkPreview.showAtCursor,
      hide: linkPreview.hide,
    },
    print: {
      prepare: preparePrint,
      draw: () => viewportQueue.flush(),
      restore: restorePrint,
    },
  };
}

function rasterizeImage(src: string, opaque: boolean): Promise<string | null> {
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
          console.error(`Image too large to rasterize: ${scaledWidth}x${scaledHeight}`, shortSrc);
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
            getComputedStyle(document.body).getPropertyValue("--bg-color").trim() || "#ffffff";
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
}
