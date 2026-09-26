import { describe, test, expect, beforeEach } from "vitest";

import { STICKY_HEAD, classifyTables, refresh, shouldStickHead } from "./sticky-table-head";

/** A table that fits the page and is three windows tall. */
const long = {
  headRows: 1,
  inFrontmatter: false,
  scrollWidth: 600,
  clientWidth: 600,
  height: 2400,
  viewportHeight: 800,
};

describe("shouldStickHead", () => {
  test("a long table that fits across the page keeps its header in view", () => {
    expect(shouldStickHead(long)).toBe(true);
  });

  test("a table no taller than the view is left alone", () => {
    expect(shouldStickHead({ ...long, height: 800 })).toBe(false);
    expect(shouldStickHead({ ...long, height: 300 })).toBe(false);
  });

  test("one pixel taller than the view is enough", () => {
    expect(shouldStickHead({ ...long, height: 801 })).toBe(true);
  });

  test("a table that scrolls sideways keeps scrolling rather than sticking", () => {
    expect(shouldStickHead({ ...long, scrollWidth: 601 })).toBe(false);
  });

  test("a table with no header row has nothing to keep in view", () => {
    expect(shouldStickHead({ ...long, headRows: 0 })).toBe(false);
  });

  test("a header of several rows is left alone, as each row would stick over the last", () => {
    expect(shouldStickHead({ ...long, headRows: 2 })).toBe(false);
  });

  test("the frontmatter's table is never pinned", () => {
    expect(shouldStickHead({ ...long, inFrontmatter: true })).toBe(false);
  });
});

/**
 * Give a table the layout happy-dom does not compute.
 *
 * Only the measurements `classifyTables` reads are faked; everything else is
 * the real element.
 */
function measure(table: HTMLTableElement, size: { width: number; height: number }): void {
  Object.defineProperty(table, "scrollWidth", { configurable: true, value: size.width });
  Object.defineProperty(table, "clientWidth", { configurable: true, value: 600 });
  table.getBoundingClientRect = () => ({ height: size.height }) as DOMRect;
}

describe("classifyTables", () => {
  let body: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = `
      <article class="markdown-body">
        <details class="frontmatter" open>
          <table class="frontmatter-table"><thead><tr><th>key</th></tr></thead></table>
        </details>
        <table id="long"><thead><tr><th>a</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>
        <table id="short"><thead><tr><th>a</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>
        <table id="headless"><tbody><tr><td>1</td></tr></tbody></table>
        <table id="two-rows"><thead><tr><th>a</th></tr><tr><th>b</th></tr></thead></table>
        <table id="two-heads"><thead><tr><th>a</th></tr></thead><thead><tr><th>b</th></tr></thead></table>
      </article>
    `;
    body = document.querySelector<HTMLElement>(".markdown-body")!;
    for (const table of body.querySelectorAll("table")) {
      measure(table, { width: 600, height: 2400 });
    }
    measure(document.querySelector<HTMLTableElement>("#short")!, { width: 600, height: 200 });
  });

  const marked = (): string[] =>
    [...body.querySelectorAll(`table[${STICKY_HEAD}]`)].map((t) => t.id);

  test("marks only the long tables that have a header of one row", () => {
    classifyTables(body, 800);

    expect(marked()).toEqual(["long"]);
  });

  test("takes the mark off a table that no longer qualifies", () => {
    classifyTables(body, 800);
    measure(document.querySelector<HTMLTableElement>("#long")!, { width: 900, height: 2400 });

    classifyTables(body, 800);

    expect(marked()).toEqual([]);
  });

  test("answers to the view it is given, which zoom and resizing change", () => {
    classifyTables(body, 3000);

    expect(marked()).toEqual([]);
  });

  test("returns every table it looked at, so that each can be watched for resizing", () => {
    expect(classifyTables(body, 800)).toHaveLength(6);
  });
});

describe("refresh", () => {
  test("classifies the document in the scroller, not another rendered answer before it", async () => {
    document.body.innerHTML = `
      <header><div class="markdown-body">
        <table id="answer"><thead><tr><th>a</th></tr></thead></table>
      </div></header>
      <div class="content"><div><article class="markdown-body">
        <table id="page"><thead><tr><th>a</th></tr></thead></table>
      </article></div></div>
    `;
    Object.defineProperty(document.querySelector(".content")!, "clientHeight", {
      configurable: true,
      value: 800,
    });
    for (const table of document.querySelectorAll("table")) {
      measure(table, { width: 600, height: 2400 });
    }

    refresh();
    await new Promise((resolve) => requestAnimationFrame(resolve));

    const marked = [...document.querySelectorAll(`table[${STICKY_HEAD}]`)].map((t) => t.id);
    expect(marked).toEqual(["page"]);
  });
});
