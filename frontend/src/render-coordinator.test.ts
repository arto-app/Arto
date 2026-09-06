import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";

// Mock external renderer dependencies before import
vi.mock("./math-renderer", () => ({
  renderMath: vi.fn(),
}));
vi.mock("./syntax-highlighter", () => ({
  highlightCodeBlocks: vi.fn(),
}));
vi.mock("./mermaid-renderer", () => ({
  renderDiagrams: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("./code-copy", () => ({
  addCopyButtons: vi.fn(),
}));

import { _internal } from "./render-coordinator";
import * as mathRenderer from "./math-renderer";
import * as syntaxHighlighter from "./syntax-highlighter";
import * as mermaidRenderer from "./mermaid-renderer";
import * as codeCopy from "./code-copy";

const { RenderCoordinator } = _internal;

// Capture rAF callbacks for manual flushing
let rafCallbacks: Array<() => void> = [];
function flushRaf(): void {
  const callbacks = rafCallbacks;
  rafCallbacks = [];
  for (const cb of callbacks) {
    cb();
  }
}

// Track MutationObserver instances to prevent cross-test leakage
let trackedObservers: MutationObserver[] = [];
const OriginalMutationObserver = globalThis.MutationObserver;

beforeEach(() => {
  // Disconnect all MutationObservers from previous tests
  for (const obs of trackedObservers) {
    obs.disconnect();
  }
  trackedObservers = [];

  rafCallbacks = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    const id = rafCallbacks.length + 1;
    rafCallbacks.push(() => cb(0));
    return id;
  });
  vi.stubGlobal(
    "MutationObserver",
    class extends OriginalMutationObserver {
      constructor(callback: MutationCallback) {
        super(callback);
        trackedObservers.push(this);
      }
    },
  );
  document.body.innerHTML = "";
  vi.clearAllMocks();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("RenderCoordinator", () => {
  describe("scheduleRender", () => {
    test("schedules a rAF callback", () => {
      const coordinator = new RenderCoordinator();
      coordinator.scheduleRender();
      expect(rafCallbacks.length).toBe(1);
    });

    test("deduplicates multiple calls before rAF fires", () => {
      const coordinator = new RenderCoordinator();
      coordinator.scheduleRender();
      coordinator.scheduleRender();
      coordinator.scheduleRender();
      expect(rafCallbacks.length).toBe(1);
    });

    test("allows new schedule after rAF callback executes", () => {
      const coordinator = new RenderCoordinator();
      coordinator.scheduleRender();
      flushRaf();

      coordinator.scheduleRender();
      expect(rafCallbacks.length).toBe(1);
    });
  });

  describe("#executeBatchRender", () => {
    test("calls all renderers when .markdown-body exists", async () => {
      document.body.innerHTML = '<div class="markdown-body">content</div>';
      const coordinator = new RenderCoordinator();

      coordinator.scheduleRender();
      flushRaf();
      // Allow async rendering to complete
      await vi.waitFor(() => {
        expect(mathRenderer.renderMath).toHaveBeenCalledOnce();
      });

      expect(syntaxHighlighter.highlightCodeBlocks).toHaveBeenCalledOnce();
      expect(mermaidRenderer.renderDiagrams).toHaveBeenCalledOnce();
      expect(codeCopy.addCopyButtons).toHaveBeenCalledOnce();
    });

    test("passes markdown-body element to each renderer", async () => {
      document.body.innerHTML = '<div class="markdown-body">content</div>';
      const markdownBody = document.querySelector(".markdown-body")!;
      const coordinator = new RenderCoordinator();

      coordinator.scheduleRender();
      flushRaf();
      await vi.waitFor(() => {
        expect(mathRenderer.renderMath).toHaveBeenCalledWith(markdownBody);
      });

      expect(syntaxHighlighter.highlightCodeBlocks).toHaveBeenCalledWith(markdownBody);
      expect(mermaidRenderer.renderDiagrams).toHaveBeenCalledWith(markdownBody);
      expect(codeCopy.addCopyButtons).toHaveBeenCalledWith(markdownBody);
    });

    test("skips rendering when no .markdown-body exists", async () => {
      document.body.innerHTML = "<div>no markdown here</div>";
      const coordinator = new RenderCoordinator();

      coordinator.scheduleRender();
      flushRaf();
      // Give async a tick to settle
      await new Promise((r) => setTimeout(r, 0));

      expect(mathRenderer.renderMath).not.toHaveBeenCalled();
      expect(mermaidRenderer.renderDiagrams).not.toHaveBeenCalled();
    });

    test("handles multiple .markdown-body elements", async () => {
      document.body.innerHTML = `
        <div class="markdown-body">first</div>
        <div class="markdown-body">second</div>
      `;
      const coordinator = new RenderCoordinator();

      coordinator.scheduleRender();
      flushRaf();
      await vi.waitFor(() => {
        expect(mathRenderer.renderMath).toHaveBeenCalledTimes(2);
      });

      expect(mermaidRenderer.renderDiagrams).toHaveBeenCalledTimes(2);
    });
  });

  describe("init", () => {
    test("schedules an initial render", () => {
      const coordinator = new RenderCoordinator();
      coordinator.init();
      expect(rafCallbacks.length).toBe(1);
    });

    test("sets up MutationObserver that triggers render on DOM changes", async () => {
      document.body.innerHTML = '<div class="markdown-body"></div>';
      const coordinator = new RenderCoordinator();
      coordinator.init();
      // Flush the initial render
      flushRaf();
      // Wait for the full async render cycle to complete (including finally block)
      await vi.waitFor(() => {
        expect(codeCopy.addCopyButtons).toHaveBeenCalledOnce();
      });
      // Flush microtask queue so #isRendering = false in the finally block
      await new Promise((r) => setTimeout(r, 0));

      vi.clearAllMocks();

      // Trigger a DOM mutation
      const child = document.createElement("p");
      child.textContent = "new content";
      document.body.querySelector(".markdown-body")!.appendChild(child);

      // MutationObserver fires asynchronously
      await vi.waitFor(() => {
        expect(rafCallbacks.length).toBe(1);
      });
    });
  });

  describe("onRenderComplete", () => {
    test("fires callback after successful render", async () => {
      document.body.innerHTML = '<div class="markdown-body">content</div>';
      const coordinator = new RenderCoordinator();
      const callback = vi.fn();

      coordinator.onRenderComplete(callback);
      coordinator.scheduleRender();
      flushRaf();
      await vi.waitFor(() => {
        expect(callback).toHaveBeenCalledOnce();
      });
    });

    test("callbacks are one-time (cleared after firing)", async () => {
      document.body.innerHTML = '<div class="markdown-body">content</div>';
      const coordinator = new RenderCoordinator();
      const callback = vi.fn();

      coordinator.onRenderComplete(callback);
      coordinator.scheduleRender();
      flushRaf();
      await vi.waitFor(() => {
        expect(callback).toHaveBeenCalledOnce();
      });

      // Second render should not fire the callback again
      coordinator.scheduleRender();
      flushRaf();
      await new Promise((r) => setTimeout(r, 0));
      expect(callback).toHaveBeenCalledOnce();
    });

    test("error in one callback does not prevent others from firing", async () => {
      document.body.innerHTML = '<div class="markdown-body">content</div>';
      const coordinator = new RenderCoordinator();
      const errorCallback = vi.fn(() => {
        throw new Error("callback error");
      });
      const normalCallback = vi.fn();

      coordinator.onRenderComplete(errorCallback);
      coordinator.onRenderComplete(normalCallback);
      coordinator.scheduleRender();
      flushRaf();
      await vi.waitFor(() => {
        expect(normalCallback).toHaveBeenCalledOnce();
      });

      expect(errorCallback).toHaveBeenCalledOnce();
    });

    test("still fired when no .markdown-body exists (no-op render completes)", async () => {
      document.body.innerHTML = "<div>no markdown</div>";
      const coordinator = new RenderCoordinator();
      const callback = vi.fn();

      coordinator.onRenderComplete(callback);
      coordinator.scheduleRender();
      flushRaf();
      await new Promise((r) => setTimeout(r, 0));

      // Callbacks fire even on no-op renders to prevent leaking
      // registered callbacks that would fire at unexpected later times.
      expect(callback).toHaveBeenCalledOnce();
    });
  });

  describe("MutationObserver guard during rendering", () => {
    test("ignores DOM mutations while rendering is in progress", async () => {
      document.body.innerHTML = '<div class="markdown-body"></div>';

      // Make mermaidRenderer.renderDiagrams trigger a DOM mutation while rendering
      vi.mocked(mermaidRenderer.renderDiagrams).mockImplementation(async (el) => {
        // Simulate Mermaid adding SVG content (DOM mutation during render)
        const svg = document.createElement("div");
        svg.className = "mermaid-svg";
        (el as Element).appendChild(svg);
      });

      const coordinator = new RenderCoordinator();
      coordinator.init();

      // Flush initial render
      flushRaf();
      await vi.waitFor(() => {
        expect(mermaidRenderer.renderDiagrams).toHaveBeenCalledOnce();
      });

      // The self-triggered mutation should NOT have scheduled another render
      // (because #isRendering was true when MutationObserver fired)
      expect(rafCallbacks.length).toBe(0);
    });
  });

  describe("forceRenderMermaid", () => {
    test("clears data-rendered and data-copy-button-added attributes", () => {
      document.body.innerHTML = `
        <div class="markdown-body">
          <pre class="preprocessed-mermaid" data-rendered data-copy-button-added>
            <svg>old diagram</svg>
          </pre>
        </div>
      `;
      const coordinator = new RenderCoordinator();
      coordinator.forceRenderMermaid();

      const el = document.querySelector(".preprocessed-mermaid")!;
      expect(el.hasAttribute("data-rendered")).toBe(false);
      expect(el.hasAttribute("data-copy-button-added")).toBe(false);
      expect(el.innerHTML).toBe("");
    });

    test("does nothing when no .markdown-body exists", () => {
      document.body.innerHTML = "<div>no markdown</div>";
      const coordinator = new RenderCoordinator();
      coordinator.forceRenderMermaid();

      // No rAF should have been scheduled
      expect(rafCallbacks.length).toBe(0);
    });

    test("schedules only mermaid + codeCopy rendering (not math or syntax)", async () => {
      document.body.innerHTML = `
        <div class="markdown-body">
          <pre class="preprocessed-mermaid" data-rendered>old</pre>
        </div>
      `;
      const coordinator = new RenderCoordinator();
      coordinator.forceRenderMermaid();

      flushRaf();
      await vi.waitFor(() => {
        expect(mermaidRenderer.renderDiagrams).toHaveBeenCalledOnce();
      });

      expect(codeCopy.addCopyButtons).toHaveBeenCalledOnce();
      expect(mathRenderer.renderMath).not.toHaveBeenCalled();
      expect(syntaxHighlighter.highlightCodeBlocks).not.toHaveBeenCalled();
    });
  });
});
