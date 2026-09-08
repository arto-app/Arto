import { applyPictureTheme } from "./picture-theme";
import * as mathRenderer from "./math-renderer";
import * as mermaidRenderer from "./mermaid-renderer";
import * as syntaxHighlighter from "./syntax-highlighter";
import * as codeCopy from "./code-copy";
import * as viewportQueue from "./viewport-queue";
import * as scrollAnchor from "./scroll-anchor";

/**
 * Setup single-click listeners for Image blocks.
 * - Math blocks: Click handled by math-renderer during rendering
 * - Mermaid blocks: Click handled by mermaid-renderer during rendering
 * - Image blocks (`img`): Single-click opens Image window
 */
function setupSpecialBlockListeners(markdownBody: Element): void {
  // Image single-click listener
  markdownBody.querySelectorAll("img").forEach((img) => {
    // Skip if already has listener
    if (img.dataset.listenersAttached === "true") {
      return;
    }

    // Skip images inside links to avoid conflicting with link navigation
    if (img.closest("a")) {
      return;
    }

    // Hover styling (cursor, opacity, outline) is handled by CSS via
    // .markdown-body img[data-listeners-attached="true"]:hover in image-window.css
    img.addEventListener("click", () => {
      // `currentSrc` is the candidate the browser picked, which inside a
      // <picture> or with a srcset is not what the `src` attribute holds:
      // the window has to show the variant on screen.
      const src = img.currentSrc || img.getAttribute("src");
      const alt = img.getAttribute("alt");
      if (src && typeof window.handleImageWindowOpen === "function") {
        window.handleImageWindowOpen(src, alt);
      }
    });

    img.dataset.listenersAttached = "true";
  });
}

class RenderCoordinator {
  #rafId: number | null = null;
  /** The Mermaid re-theme pass in flight, so a second ask joins the first. */
  #mermaidPass: Promise<void> | null = null;
  /**
   * How many batch renders are in flight.
   *
   * A count rather than a flag: a batch render spans several frames now that
   * it waits for the blocks on screen to be drawn, which is long enough for a
   * theme change to start a Mermaid re-render inside that window. With a flag
   * the inner render's completion would drop the guard while the outer one is
   * still writing.
   */
  #batchDepth = 0;
  /** Whether the viewport queue is drawing a block right now. */
  #queueDrawing = false;

  /**
   * Whether anything this class is responsible for is writing to the DOM.
   *
   * Both a batch render and a queued job write, and they overlap: a job for
   * a block near the viewport starts while the batch that registered it is
   * still finishing. One flag for both would let whichever finished first
   * drop the guard for the other.
   */
  get #isRendering(): boolean {
    return this.#batchDepth > 0 || this.#queueDrawing;
  }
  #hasPendingMutations = false;
  #pendingMutationRetries = 0;
  #renderCompleteCallbacks: Array<() => void> = [];
  #observer: MutationObserver | null = null;
  #beforePrint: (() => void) | null = null;

  // Safety limit to prevent infinite render loops caused by
  // renderers modifying the DOM (e.g., Mermaid SVG insertion).
  // In practice, data-rendered/data-highlighted guards on individual
  // renderers terminate the cycle after 1-2 iterations.
  static readonly #MAX_PENDING_RETRIES = 3;

  // Upper bound on how long a batch render waits for the viewport queue to
  // draw the screen. Without it a reader who keeps scrolling would keep the
  // queue busy, and the render-complete callbacks — which a scroll restore
  // waits on — would never fire.
  static readonly #MAX_SETTLE_FRAMES = 30;

  init(): void {
    this.#observer = new MutationObserver((mutations) => {
      // Defer mutations that arrive while rendering to avoid cascade.
      // They will be re-scheduled after the current render completes.
      if (this.#isRendering) {
        // Unless every one of them stayed inside a block the queue is
        // drawing, in which case they are that drawing and there is nothing
        // new to render. Deferring those would replay them as a full pass
        // over the document each time the queue goes idle — once per frame
        // of scrolling, on the document the queue exists to make fast.
        if (mutations.length > 0 && mutations.every((m) => viewportQueue.isDrawing(m.target))) {
          return;
        }
        this.#hasPendingMutations = true;
        return;
      }

      // Check if there's an actual content change
      const hasContentChange = mutations.some(
        (m) => m.type === "childList" || m.type === "attributes",
      );

      if (hasContentChange) {
        console.debug("RenderCoordinator: Content change detected, scheduling render");
        this.scheduleRender();
      }
    });

    this.#observer.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
    });
    console.debug("RenderCoordinator: MutationObserver set up on document.body");

    // A block drawn while the reader scrolls writes to the DOM, and without
    // this the observer above reads that as new content and pays for a pass
    // over the whole document — per block, on the document this exists to
    // make fast. The queue is the same kind of rendering as a batch render,
    // so it takes the same guard.
    viewportQueue.setActivityListener((active) => {
      this.#queueDrawing = active;
      if (!active && this.#batchDepth === 0) {
        this.#processPendingMutations();
      }
    });

    // Best effort for a print the app did not start itself. A listener cannot
    // delay the capture — the flush is asynchronous and `beforeprint` is not
    // awaited — so a print driven by the app goes through
    // `window.Arto.print.prepare`, which awaits the same flush before the
    // dialog opens.
    this.#beforePrint = () => {
      void viewportQueue.flush();
    };
    window.addEventListener("beforeprint", this.#beforePrint);

    // Schedule an initial render
    this.scheduleRender();
  }

  destroy(): void {
    viewportQueue.setActivityListener(null);
    if (this.#beforePrint) {
      window.removeEventListener("beforeprint", this.#beforePrint);
      this.#beforePrint = null;
    }
    if (this.#observer) {
      this.#observer.disconnect();
      this.#observer = null;
    }
    if (this.#rafId !== null) {
      cancelAnimationFrame(this.#rafId);
      this.#rafId = null;
    }
  }

  scheduleRender(): void {
    if (this.#rafId !== null) {
      return; // Already scheduled
    }
    this.#rafId = requestAnimationFrame(() => {
      this.#rafId = null;
      this.#executeBatchRender();
    });
  }

  /**
   * Register a one-time callback to be called when the next render completes.
   * Used for restoring scroll position after Mermaid/KaTeX rendering.
   */
  onRenderComplete(callback: () => void): void {
    this.#renderCompleteCallbacks.push(callback);
  }

  #fireRenderCompleteCallbacks(): void {
    const callbacks = this.#renderCompleteCallbacks;
    this.#renderCompleteCallbacks = [];
    for (const callback of callbacks) {
      try {
        callback();
      } catch (error) {
        console.error("RenderCoordinator: Error in render complete callback:", error);
      }
    }
  }

  /**
   * Draw every Mermaid diagram again, in the theme that is current now.
   *
   * Resolves once the diagrams have been queued again, which is what the
   * print path has to wait for: it clears them here and drains the queue
   * next, and a clear whose re-queue has not happened yet would be drained
   * as nothing at all.
   */
  forceRenderMermaid(): Promise<void> {
    const markdownBodies = document.querySelectorAll(".markdown-body");
    if (markdownBodies.length === 0) {
      return Promise.resolve();
    }

    this.#clearDrawnDiagrams();
    return this.#scheduleMermaidRender();
  }

  /** Throw away every drawn diagram, so the next pass draws them again. */
  #clearDrawnDiagrams(): void {
    const markdownBodies = document.querySelectorAll(".markdown-body");

    markdownBodies.forEach((markdownBody) => {
      markdownBody.querySelectorAll("pre.preprocessed-mermaid[data-rendered]").forEach((el) => {
        const element = el as HTMLElement;

        // Clear the rendered content and copy button flag
        element.innerHTML = "";
        element.removeAttribute("data-rendered");
        element.removeAttribute("data-copy-button-added");
      });
    });
  }

  #scheduleMermaidRender(): Promise<void> {
    if (this.#mermaidPass !== null) {
      return this.#mermaidPass;
    }

    this.#mermaidPass = new Promise<void>((settled) => {
      this.#rafId = requestAnimationFrame(async () => {
        this.#rafId = null;
        try {
          // A job that was already drawing a diagram when the theme changed
          // finishes by writing it in the theme being replaced and marking it
          // drawn, which would make the pass below skip it. Let those land,
          // then throw them away too.
          await viewportQueue.idle();
          this.#clearDrawnDiagrams();
          await this.#runMermaidPass();
        } finally {
          this.#mermaidPass = null;
          settled();
        }
      });
    });
    return this.#mermaidPass;
  }

  async #runMermaidPass(): Promise<void> {
    const markdownBodies = document.querySelectorAll(".markdown-body");
    if (markdownBodies.length === 0) {
      return;
    }

    this.#batchDepth++;
    try {
      await Promise.all(
        Array.from(markdownBodies).map(async (markdownBody) => {
          await mermaidRenderer.renderDiagrams(markdownBody);
          // Re-add copy buttons after Mermaid re-render
          codeCopy.addCopyButtons(markdownBody);
          setupSpecialBlockListeners(markdownBody);
        }),
      );
      console.debug("RenderCoordinator: Mermaid re-render completed");
    } catch (error) {
      console.error("RenderCoordinator: Error during Mermaid re-render:", error);
    } finally {
      this.#batchDepth--;
      this.#processPendingMutations();
    }
  }

  /**
   * Wait until the queued work for the blocks on screen has run.
   *
   * An IntersectionObserver reports after the frame's animation callbacks,
   * so two frames pass before the jobs for what is already visible have even
   * started; `idle()` then waits for those to finish. Jobs for blocks the
   * reader has not reached stay queued, so this does not wait for the
   * document.
   */
  async #renderedNearViewport(): Promise<void> {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    });
    // The queue drains one block at a time, so nothing is running in the gap
    // between two jobs and a single `idle()` returns there — after the first
    // block on screen rather than after the screen. Keep waiting while the
    // queue still holds work it considers near, bounded so that a reader who
    // scrolls into new blocks cannot hold the render open indefinitely.
    for (
      let frame = 0;
      frame < RenderCoordinator.#MAX_SETTLE_FRAMES && viewportQueue.busy();
      frame++
    ) {
      await viewportQueue.idle();
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    }
  }

  async #executeBatchRender(): Promise<void> {
    // A batch render is the one moment the document is known to have changed,
    // so it is where the queue sheds the blocks of the document it replaced
    // and the anchor's block list is declared stale. Neither writes to the
    // DOM, so both stay outside the guard: raising `#batchDepth` before
    // anything that can throw without a `finally` under it would leave the
    // guard raised for good, and every later mutation deferred for ever.
    viewportQueue.prune();
    scrollAnchor.invalidateBlocks();

    const markdownBodies = document.querySelectorAll(".markdown-body");
    if (markdownBodies.length === 0) {
      this.#fireRenderCompleteCallbacks();
      this.#processPendingMutations();
      return;
    }

    this.#batchDepth++;
    try {
      await Promise.all(
        Array.from(markdownBodies).map(async (markdownBody) => {
          applyPictureTheme(markdownBody);
          mathRenderer.renderMath(markdownBody);
          syntaxHighlighter.highlightCodeBlocks(markdownBody);
          await mermaidRenderer.renderDiagrams(markdownBody);
          codeCopy.addCopyButtons(markdownBody);
          setupSpecialBlockListeners(markdownBody);
        }),
      );
      // The renderers only queued their work, so the document is untouched
      // at this point. The callbacks are the signal that heights have
      // stopped moving — a scroll position is restored against them — so
      // wait for the screen the reader is looking at to be drawn. Only the
      // blocks near the viewport are involved; the rest stay queued.
      await this.#renderedNearViewport();
      console.debug("RenderCoordinator: Batch render completed");
    } catch (error) {
      console.error("RenderCoordinator: Error during batch render:", error);
    } finally {
      this.#batchDepth--;
      this.#fireRenderCompleteCallbacks();
      this.#processPendingMutations();
    }
  }

  #processPendingMutations(): void {
    if (!this.#hasPendingMutations) {
      this.#pendingMutationRetries = 0;
      return;
    }

    this.#hasPendingMutations = false;
    this.#pendingMutationRetries++;

    if (this.#pendingMutationRetries > RenderCoordinator.#MAX_PENDING_RETRIES) {
      console.warn(
        `RenderCoordinator: Max pending mutation retries (${RenderCoordinator.#MAX_PENDING_RETRIES}) reached, breaking potential loop`,
      );
      this.#pendingMutationRetries = 0;
      return;
    }

    console.debug(
      `RenderCoordinator: Processing deferred mutations (attempt ${this.#pendingMutationRetries})`,
    );
    this.scheduleRender();
  }
}

export const renderCoordinator = new RenderCoordinator();

/** @internal */
export const _internal = { RenderCoordinator };
