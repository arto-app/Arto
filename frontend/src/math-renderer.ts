import katex from "katex";

export function renderMath(container: Element): void {
  renderInlineMath(container);
  renderDisplayMath(container);
  renderBlockMath(container);
}

function renderInlineMath(container: Element): void {
  // Process inline math: <span class="math math-inline">...</span>
  const inlineMathElements: NodeListOf<HTMLElement> = container.querySelectorAll(
    "span.preprocessed-math-inline:not([data-katex-rendered])",
  );

  // Batch: Collect all elements to render (read phase)
  const renderQueue: Array<{ element: HTMLElement; content: string }> = [];

  for (const element of Array.from(inlineMathElements)) {
    const mathContent = element.dataset.originalContent || "";
    if (mathContent) {
      renderQueue.push({
        element: element as HTMLElement,
        content: mathContent,
      });
    }
  }

  // Batch: Render all at once (write phase)
  for (const { element, content } of renderQueue) {
    try {
      // Use renderToString to avoid intermediate DOM access
      const html = katex.renderToString(content, {
        throwOnError: false,
        displayMode: false,
      });
      element.innerHTML = html;
      element.setAttribute("data-katex-rendered", "true");
    } catch (error) {
      console.error("Failed to render inline math:", error);
      element.style.color = "red";
    }
  }

  if (renderQueue.length > 0) {
    console.debug(`Rendered ${renderQueue.length} inline math expressions`);
  }
}

function renderDisplayMath(container: Element): void {
  // Process display math: <span class="math math-display">...</span>
  const displayMathElements: NodeListOf<HTMLElement> = container.querySelectorAll(
    "div.preprocessed-math-display:not([data-katex-rendered])",
  );

  // Batch: Collect all elements to render (read phase)
  const renderQueue: Array<{ element: HTMLElement; content: string }> = [];

  for (const element of Array.from(displayMathElements)) {
    const mathContent = element.dataset.originalContent || "";
    if (mathContent) {
      renderQueue.push({
        element: element as HTMLElement,
        content: mathContent,
      });
    }
  }

  // Batch: Render all at once (write phase)
  for (const { element, content } of renderQueue) {
    try {
      // Use renderToString to avoid intermediate DOM access
      const html = katex.renderToString(content, {
        throwOnError: false,
        displayMode: true,
      });
      element.innerHTML = html;
      element.setAttribute("data-katex-rendered", "true");
    } catch (error) {
      console.error("Failed to render display math:", error);
      element.style.color = "red";
    }
  }

  if (renderQueue.length > 0) {
    console.debug(`Rendered ${renderQueue.length} display math expressions`);
  }
}

function renderBlockMath(container: Element): void {
  const mathBlocks: NodeListOf<HTMLElement> = container.querySelectorAll(
    "pre.preprocessed-math:not([data-rendered])",
  );

  // Batch: Collect all elements to render (read phase)
  const renderQueue: Array<{ element: HTMLElement; content: string }> = [];

  for (const block of Array.from(mathBlocks)) {
    const element = block as HTMLElement;
    const mathContent = element.dataset.originalContent || "";

    if (mathContent) {
      renderQueue.push({ element, content: mathContent });
    } else {
      // Mark empty blocks as rendered to skip in future
      element.dataset.rendered = "true";
    }
  }

  // Batch: Render all at once (write phase)
  for (const { element, content } of renderQueue) {
    try {
      // Use renderToString to avoid intermediate DOM access
      const html = katex.renderToString(content, {
        throwOnError: false,
        displayMode: true,
      });
      element.innerHTML = html;
      element.dataset.rendered = "true";

      // Skip if listeners already attached (guard against re-registration)
      if (element.dataset.listenersAttached === "true") continue;
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
    }
  }

  if (renderQueue.length > 0) {
    console.debug(`Rendered ${renderQueue.length} math blocks`);
  }
}
