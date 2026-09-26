import { type KatexLibrary, katexLibrary } from "./libraries";
import { whenNearViewport } from "./viewport-queue";
import { restoreCopyButton } from "./code-copy";

/**
 * Where the formulas being set are shown.
 *
 * On the page they wait for the reader to come near, and a block opens the
 * math window. Anywhere else — a link's preview, laid over the page — they
 * are set at once, since the viewport queue watches the page's scroller and
 * would never see them come near, and a block is left as it is to click.
 */
type Place = "page" | "elsewhere";

type Schedule = (element: Element, job: () => void) => void;

const runNow: Schedule = (_, job) => job();

export function renderMath(container: Element): void {
  render(container, "page");
}

/** Set the formulas in `container`, which is not the page; see [`Place`]. */
export function renderMathElsewhere(container: Element): void {
  render(container, "elsewhere");
}

function render(container: Element, place: Place): void {
  // A page whose document sets no formula carries no KaTeX, and has nothing
  // here to typeset either.
  const katex = katexLibrary();
  if (!katex) {
    return;
  }
  const schedule = place === "page" ? whenNearViewport : runNow;
  renderInlineMath(container, katex, schedule);
  renderDisplayMath(container, katex, schedule);
  renderBlockMath(container, katex, schedule, place);
}

function renderInlineMath(container: Element, katex: KatexLibrary, schedule: Schedule): void {
  // Process inline math: <span class="math math-inline">...</span>
  const inlineMathElements: NodeListOf<HTMLElement> = container.querySelectorAll(
    "span.preprocessed-math-inline:not([data-katex-rendered])",
  );

  for (const element of Array.from(inlineMathElements)) {
    const mathContent = element.dataset.originalContent || "";
    if (!mathContent) {
      continue;
    }
    schedule(element, () => {
      try {
        // Use renderToString to avoid intermediate DOM access
        const html = katex.renderToString(mathContent, {
          throwOnError: false,
          displayMode: false,
        });
        element.innerHTML = html;
        element.setAttribute("data-katex-rendered", "true");
      } catch (error) {
        console.error("Failed to render inline math:", error);
        element.style.color = "red";
      }
    });
  }
}

function renderDisplayMath(container: Element, katex: KatexLibrary, schedule: Schedule): void {
  // Process display math: <span class="math math-display">...</span>
  const displayMathElements: NodeListOf<HTMLElement> = container.querySelectorAll(
    "div.preprocessed-math-display:not([data-katex-rendered])",
  );

  for (const element of Array.from(displayMathElements)) {
    const mathContent = element.dataset.originalContent || "";
    if (!mathContent) {
      continue;
    }
    schedule(element, () => {
      try {
        // Use renderToString to avoid intermediate DOM access
        const html = katex.renderToString(mathContent, {
          throwOnError: false,
          displayMode: true,
        });
        element.innerHTML = html;
        element.setAttribute("data-katex-rendered", "true");
      } catch (error) {
        console.error("Failed to render display math:", error);
        element.style.color = "red";
      }
    });
  }
}

function renderBlockMath(
  container: Element,
  katex: KatexLibrary,
  schedule: Schedule,
  place: Place,
): void {
  const mathBlocks: NodeListOf<HTMLElement> = container.querySelectorAll(
    "pre.preprocessed-math:not([data-rendered])",
  );

  for (const block of Array.from(mathBlocks)) {
    const element = block as HTMLElement;
    const content = element.dataset.originalContent || "";

    if (!content) {
      // Mark empty blocks as rendered to skip in future
      element.dataset.rendered = "true";
      continue;
    }

    schedule(element, () => {
      try {
        // Use renderToString to avoid intermediate DOM access
        const html = katex.renderToString(content, {
          throwOnError: false,
          displayMode: true,
        });
        element.innerHTML = html;
        element.dataset.rendered = "true";

        if (place !== "page") {
          return;
        }

        // Skip if listeners already attached (guard against re-registration)
        if (element.dataset.listenersAttached === "true") {
          return;
        }
        element.dataset.listenersAttached = "true";

        // Make math block clickable to open viewer (single-click, like Mermaid)
        // Hover styling (cursor, opacity, outline) is handled by CSS via
        // pre.preprocessed-math:hover in math-window.css
        element.addEventListener("click", () => {
          if (typeof window.handleMathWindowOpen === "function") {
            window.handleMathWindowOpen(content);
          }
        });
      } catch (error) {
        console.error("Failed to render math block:", error);
        element.style.color = "red";
        element.dataset.rendered = "true";
      } finally {
        // Registering this job took the block's place in the queue, so the
        // copy button was never added; typesetting would also have wiped one
        // that was. Either way the block gets its button here.
        if (place === "page") restoreCopyButton(element as HTMLPreElement);
      }
    });
  }
}
