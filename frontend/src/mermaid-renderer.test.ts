import { describe, test, expect, beforeEach, afterEach } from "vitest";
import * as pele from "pele";

import { provideMermaid } from "./libraries";
import { renderDiagrams, renderDiagramsElsewhere } from "./mermaid-renderer";
import * as viewportQueue from "./viewport-queue";

function page(html: string): HTMLElement {
  document.body.innerHTML = `<article class="markdown-body">${html}</article>`;
  return document.querySelector(".markdown-body") as HTMLElement;
}

function diagram(source: string): string {
  const block = document.createElement("pre");
  block.className = "preprocessed-mermaid";
  block.dataset.originalContent = source;
  block.textContent = source;
  return block.outerHTML;
}

beforeEach(() => {
  provideMermaid(pele);
});

afterEach(() => {
  viewportQueue.reset();
  document.body.innerHTML = "";
});

describe("renderDiagrams", () => {
  test("a diagram is drawn as Pele's SVG once the reader comes near it", async () => {
    const body = page(diagram("flowchart LR\n  A --> B"));

    await renderDiagrams(body);
    await viewportQueue.flush();

    const block = body.querySelector("pre.preprocessed-mermaid") as HTMLElement;
    expect(block.querySelector("svg.pele")).not.toBeNull();
    expect(block.dataset.rendered).toBe("true");
  });

  test("a fenced mermaid code block is drawn too", async () => {
    const body = page(
      `<pre><code class="language-mermaid">flowchart LR\n  A --&gt; B</code></pre>`,
    );

    await renderDiagrams(body);
    await viewportQueue.flush();

    expect(body.querySelector("pre.preprocessed-mermaid svg.pele")).not.toBeNull();
  });

  test("diagrams on one page do not share an id", async () => {
    const source = "flowchart LR\n  accTitle: Steps\n  A --> B";
    const body = page(diagram(source) + diagram(source));

    await renderDiagrams(body);
    await viewportQueue.flush();

    const ids = Array.from(body.querySelectorAll("svg [id], svg[id]"), (el) => el.id);
    expect(ids.length).toBeGreaterThan(0);
    expect(new Set(ids).size).toBe(ids.length);
  });

  test("a diagram that cannot be drawn says why, as text", async () => {
    const body = page(diagram("<img src=x>"));

    await renderDiagrams(body);
    await viewportQueue.flush();

    const block = body.querySelector("pre.preprocessed-mermaid") as HTMLElement;
    expect(block.querySelector("svg.pele")).toBeNull();
    expect(block.querySelector("img")).toBeNull();
    expect(block.textContent).toContain("Mermaid Error:");
    expect(block.dataset.rendered).toBe("true");
  });

  test("a diagram keeps the direction its source names on a narrow page", async () => {
    // Below Pele's breakpoint a left-to-right flowchart may be turned on its
    // side; a document is shown as its author wrote it.
    const body = page(diagram("flowchart LR\n  A --> B --> C --> D"));

    await renderDiagrams(body);
    await viewportQueue.flush();

    const svg = body.querySelector("svg.pele") as SVGSVGElement;
    const [, , width, height] = (svg.getAttribute("viewBox") ?? "").split(/\s+/).map(Number);
    expect(width).toBeGreaterThan(height);
  });
});

describe("renderDiagramsElsewhere", () => {
  test("draws each diagram at once, with nothing in it left to run", () => {
    const body = page(diagram('flowchart LR\n  A --> B\n  click A "javascript:alert(1)"'));

    expect(renderDiagramsElsewhere(body)).toBe(true);

    const svg = body.querySelector("pre.preprocessed-mermaid svg.pele");
    expect(svg).not.toBeNull();
    for (const link of svg?.querySelectorAll("a") ?? []) {
      expect(link.getAttribute("href") ?? "").not.toMatch(/^javascript:/i);
    }
  });

  test("a diagram that cannot be drawn is shown as its source", () => {
    const body = page(diagram("not a diagram"));

    expect(renderDiagramsElsewhere(body)).toBe(false);

    const block = body.querySelector("pre") as HTMLElement;
    expect(block.classList.contains("preprocessed-mermaid")).toBe(false);
    expect(block.textContent).toBe("not a diagram");
  });
});
