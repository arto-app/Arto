import { describe, test, expect, beforeEach, vi } from "vitest";

vi.mock("./scroll-controller", () => ({ toElement: vi.fn() }));
vi.mock("./reading-position", () => ({ refreshReadingPosition: vi.fn() }));

import {
  afterRender,
  clear,
  first,
  mark,
  next,
  prev,
  redraw,
  set,
  setup,
  since,
  tipFor,
} from "./changes";
import { toElement } from "./scroll-controller";

function page(html: string, generation = "1"): HTMLElement {
  document.body.innerHTML = `<div class="content"><div class="markdown-viewer"><article class="markdown-body" data-render-generation="${generation}">${html}</article><div data-arto-change-marks></div></div></div>`;
  return document.querySelector<HTMLElement>(".markdown-body")!;
}

/** Lay the page out: the layer at the window's corner, the page 124px in. */
function layout(body: HTMLElement, blocks: Record<string, DOMRect>): void {
  vi.spyOn(
    document.querySelector("[data-arto-change-marks]")!,
    "getBoundingClientRect",
  ).mockReturnValue(new DOMRect(0, 0, 1000, 1000));
  vi.spyOn(body, "getBoundingClientRect").mockReturnValue(new DOMRect(124, 0, 700, 1000));
  for (const [id, rect] of Object.entries(blocks)) {
    vi.spyOn(document.getElementById(id)!, "getBoundingClientRect").mockReturnValue(rect);
  }
}

function lines(): HTMLElement[] {
  return Array.from(
    document.querySelectorAll<HTMLElement>("[data-arto-change-marks] > .change-mark"),
  );
}

function hairlines(): HTMLElement[] {
  return Array.from(
    document.querySelectorAll<HTMLElement>("[data-arto-change-marks] > .change-mark-removed"),
  );
}

function marked(body: HTMLElement): Record<string, string> {
  const found: Record<string, string> = {};
  for (const el of Array.from(body.querySelectorAll<HTMLElement>("[data-change]"))) {
    found[el.id] = el.dataset.change!;
  }
  return found;
}

function removed(body: HTMLElement): Record<string, string> {
  const found: Record<string, string> = {};
  for (const el of Array.from(body.querySelectorAll<HTMLElement>("[data-change-removed]"))) {
    found[el.id] = el.dataset.changeRemoved!;
  }
  return found;
}

const DOCUMENT = `
  <h1 id="h" data-source-range="1:1-1:7">Title</h1>
  <p id="p1" data-source-range="3:1-4:5">One</p>
  <ul id="list" data-source-range="6:1-9:5">
    <li id="li1" data-source-range="6:1-6:5">a</li>
    <li id="li2" data-source-range="7:1-8:5">b<ul id="nested" data-source-range="8:3-8:5"><li id="li3" data-source-range="8:3-8:5">c</li></ul></li>
    <li id="li4" data-source-range="9:1-9:5">d</li>
  </ul>
  <pre id="pre" data-source-range="11:1-14:3"><code id="code" data-source-range="12:1-13:4">x
y</code></pre>
  <table id="table" data-source-range="16:1-18:9"><tbody><tr data-source-range="18:1-18:9"><td id="cell" data-source-range="18:3-18:4">1</td></tr></tbody></table>
  <p id="p2" data-source-range="20:1-20:5">Two</p>
`;

describe("mark", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  test("marks the innermost blocks a run reaches", () => {
    const body = page(DOCUMENT);
    mark(body, [
      { kind: "modified", start: 3, end: 3 },
      { kind: "added", start: 8, end: 8 },
    ]);
    expect(marked(body)).toEqual({ p1: "modified", li3: "added" });
  });

  test("draws every mark in the page's own margin, however deep the block sits", () => {
    const body = page(
      `<blockquote id="quote" data-source-range="1:1-3:5"><p id="inner" data-source-range="1:3-1:9">quoted</p><p id="other" data-source-range="3:3-3:5">x</p></blockquote>`,
    );
    layout(body, { inner: new DOMRect(164, 50, 700, 30) });

    mark(body, [{ kind: "modified", start: 1, end: 1 }]);

    expect(marked(body)).toEqual({ inner: "modified" });
    const [line] = lines();
    // 15px out from the page's edge (at 124), wherever the block starts.
    expect([line.dataset.kind, line.style.left, line.style.top, line.style.height]).toEqual([
      "modified",
      "109px",
      "50px",
      "30px",
    ]);

    mark(body, []);
    expect(lines()).toEqual([]);
  });

  test("draws the marks of a table and a code block outside them, which scroll and clip", () => {
    const body = page(DOCUMENT);
    layout(body, {
      table: new DOMRect(124, 200, 400, 60),
      pre: new DOMRect(124, 100, 700, 40),
    });

    mark(body, [
      { kind: "modified", start: 12, end: 12 },
      { kind: "added", start: 18, end: 18 },
    ]);

    expect(lines().map((line) => [line.dataset.kind, line.style.top])).toEqual([
      ["modified", "100px"],
      ["added", "200px"],
    ]);
  });

  test("draws where lines were taken out as a hairline under or over a block", () => {
    const body = page(DOCUMENT);
    layout(body, {
      p1: new DOMRect(124, 20, 700, 30),
      h: new DOMRect(124, 0, 700, 10),
    });

    mark(body, [{ kind: "removed", after: 4 }]);
    const [under] = hairlines();
    expect([under.style.left, under.style.top]).toEqual(["110px", "53px"]);

    mark(body, [{ kind: "removed", after: 0 }]);
    const [over] = hairlines();
    expect(over.style.top).toBe("-13px");
  });

  test("draws nothing for a block that is not laid out, until it is", () => {
    const body = page(
      `<details id="d"><summary>More</summary><p id="inner" data-source-range="1:1-1:5">hidden</p></details>`,
    );
    // Closed: nothing is laid out.
    layout(body, { inner: new DOMRect(0, 0, 0, 0) });
    mark(body, [{ kind: "added", start: 1, end: 1 }]);
    expect(lines()).toEqual([]);

    vi.spyOn(document.getElementById("inner")!, "getBoundingClientRect").mockReturnValue(
      new DOMRect(144, 10, 700, 20),
    );
    redraw();
    expect(lines().map((line) => line.style.top)).toEqual(["10px"]);
  });

  test("marks a list where a run reaches only the lines between its items", () => {
    const body = page(
      `<ul id="list" data-source-range="1:1-3:3"><li id="a" data-source-range="1:1-1:3">a</li><li id="b" data-source-range="3:1-3:3">b</li></ul>`,
    );
    mark(body, [{ kind: "added", start: 2, end: 2 }]);
    expect(marked(body)).toEqual({ list: "added" });
  });

  test("marks a code block rather than its content, and a table rather than a cell", () => {
    const body = page(DOCUMENT);
    mark(body, [
      { kind: "modified", start: 12, end: 12 },
      { kind: "modified", start: 18, end: 18 },
    ]);
    expect(marked(body)).toEqual({ pre: "modified", table: "modified" });
  });

  test("a block both added to and rewritten reads as rewritten", () => {
    const body = page(DOCUMENT);
    mark(body, [
      { kind: "modified", start: 3, end: 3 },
      { kind: "added", start: 4, end: 4 },
    ]);
    expect(marked(body)).toEqual({ p1: "modified" });
  });

  test("a run that reaches no block marks nothing", () => {
    const body = page(DOCUMENT);
    mark(body, [{ kind: "added", start: 2, end: 2 }]);
    expect(marked(body)).toEqual({});
  });

  test("lines taken out go under the innermost block that ends before them", () => {
    const body = page(DOCUMENT);
    mark(body, [
      { kind: "removed", after: 4 },
      { kind: "removed", after: 6 },
      { kind: "removed", after: 14 },
    ]);
    expect(removed(body)).toEqual({ p1: "after", li1: "after", pre: "after" });
  });

  test("lines taken from the top go over the first block", () => {
    const body = page(DOCUMENT);
    mark(body, [{ kind: "removed", after: 0 }]);
    expect(removed(body)).toEqual({ h: "before" });
  });

  test("lines taken from the middle of a block changed that block", () => {
    const body = page(DOCUMENT);
    mark(body, [{ kind: "removed", after: 3 }]);
    expect(marked(body)).toEqual({ p1: "modified" });
    expect(removed(body)).toEqual({});
  });

  test("marking again replaces the marks before", () => {
    const body = page(DOCUMENT);
    mark(body, [
      { kind: "modified", start: 3, end: 3 },
      { kind: "removed", after: 4 },
    ]);
    mark(body, [{ kind: "added", start: 20, end: 20 }]);
    expect(marked(body)).toEqual({ p2: "added" });
    expect(removed(body)).toEqual({});
  });
});

describe("set", () => {
  test("waits for the render the changes were computed for", async () => {
    let body = page(DOCUMENT, "1");
    set([{ kind: "added", start: 20, end: 20 }], 2);
    expect(marked(body)).toEqual({});

    body.dataset.renderGeneration = "2";
    await new Promise((resolve) => setTimeout(resolve, 0));
    body = document.querySelector<HTMLElement>(".markdown-body")!;
    expect(marked(body)).toEqual({ p2: "added" });
  });

  test("marks at once when the page already shows that render", () => {
    const body = page(DOCUMENT, "3");
    set([{ kind: "added", start: 20, end: 20 }], 3);
    expect(marked(body)).toEqual({ p2: "added" });

    clear();
    expect(marked(body)).toEqual({});
  });
});

describe("next and prev", () => {
  function placed(body: HTMLElement, tops: Record<string, number>): void {
    const content = document.querySelector<HTMLElement>(".content")!;
    Object.defineProperty(content, "clientHeight", { value: 100, configurable: true });
    content.getBoundingClientRect = () => ({ top: 0, height: 100 }) as DOMRect;
    for (const [id, top] of Object.entries(tops)) {
      body.querySelector<HTMLElement>(`#${id}`)!.getBoundingClientRect = () =>
        ({ top, height: 10 }) as DOMRect;
    }
  }

  test("walk the marks in document order from the middle of the reading area", () => {
    const body = page(DOCUMENT);
    mark(body, [
      { kind: "modified", start: 1, end: 1 },
      { kind: "modified", start: 3, end: 3 },
      { kind: "added", start: 20, end: 20 },
    ]);
    placed(body, { h: -100, p1: 45, p2: 300 });
    const target = vi.mocked(toElement);

    target.mockClear();
    expect(next()).toBe(true);
    expect(target).toHaveBeenLastCalledWith(body.querySelector("#p2"), "center");

    expect(prev()).toBe(true);
    expect(target).toHaveBeenLastCalledWith(body.querySelector("#h"), "center");

    expect(first()).toBe(true);
    expect(target).toHaveBeenLastCalledWith(body.querySelector("#h"), "center");
  });

  test("say so when there is nowhere to go", () => {
    const body = page(DOCUMENT);
    mark(body, []);
    expect(next()).toBe(false);
    expect(prev()).toBe(false);
    expect(first()).toBe(false);
  });
});

describe("since", () => {
  const now = new Date("2026-09-27T12:00:00").getTime();

  test("says how long ago the document was read", () => {
    expect(since(now - 20 * 1000, now)).toBe("just now");
    expect(since(now - 5 * 60 * 1000, now)).toBe("5 minutes ago");
    expect(since(now - 2 * 60 * 60 * 1000, now)).toBe("2 hours ago");
    expect(since(now - 26 * 60 * 60 * 1000, now)).toBe("yesterday");
    expect(since(now - 3 * 24 * 60 * 60 * 1000, now)).toBe("3 days ago");
  });
});

describe("tipFor", () => {
  const now = new Date("2026-09-27T12:00:00").getTime();
  const readAt = now - 2 * 60 * 60 * 1000;

  test("says what happened to the block, and since when", () => {
    const el = document.createElement("p");
    el.dataset.change = "added";
    expect(tipFor(el, readAt, now)).toBe("Added since you last read this, 2 hours ago");
    el.dataset.change = "modified";
    expect(tipFor(el, readAt, now)).toBe("Changed since you last read this, 2 hours ago");
  });

  test("says where text was taken out", () => {
    const el = document.createElement("p");
    el.dataset.changeRemoved = "after";
    expect(tipFor(el, readAt, now)).toBe(
      "Text was taken out below since you last read this, 2 hours ago",
    );
    el.dataset.changeRemoved = "before";
    expect(tipFor(el, readAt, now)).toBe(
      "Text was taken out above since you last read this, 2 hours ago",
    );
  });

  test("names both when a block was changed and text taken out beside it", () => {
    const el = document.createElement("p");
    el.dataset.change = "modified";
    el.dataset.changeRemoved = "after";
    expect(tipFor(el, readAt, now)).toBe(
      "Changed since you last read this, 2 hours ago\nText was taken out below since you last read this, 2 hours ago",
    );
  });

  test("leaves out when, when it is not known", () => {
    const el = document.createElement("p");
    el.dataset.change = "added";
    expect(tipFor(el, null, now)).toBe("Added since you last read this");
  });
});

describe("the tip over a mark", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    setup();
  });

  function hover(el: Element, clientY = 10): void {
    el.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: 110, clientY }));
  }

  function tipOf(): HTMLElement {
    return document.querySelector<HTMLElement>("body > .change-tip")!;
  }

  function marked(html: string, readAt: number, generation = "1"): HTMLElement {
    const body = page(html, generation);
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });
    set([{ kind: "added", start: 1, end: 1 }], generation === "1" ? 1 : null, readAt);
    return lines()[0];
  }

  test("says what the mark means, and nothing over the text", () => {
    const line = marked(
      `<p id="p1" data-source-range="1:1-1:5">One</p>`,
      Date.now() - 60 * 60 * 1000,
    );

    hover(line);
    expect(tipOf().hidden).toBe(false);
    expect(tipOf().textContent).toBe("Added since you last read this, 1 hour ago");

    hover(document.getElementById("p1")!);
    expect(tipOf().hidden).toBe(true);
  });

  test("never takes over an element of the document that shares its name", () => {
    const line = marked(
      `<p id="p1" data-source-range="1:1-1:5">One</p><h2 id="h" class="change-tip">Heading</h2>`,
      Date.now(),
    );

    hover(line);

    expect(document.getElementById("h")!.textContent).toBe("Heading");
    expect(tipOf().textContent).toBe("Added since you last read this, just now");
  });

  test("goes when the pointer leaves the window", () => {
    hover(marked(`<p id="p1" data-source-range="1:1-1:5">One</p>`, Date.now()));
    expect(tipOf().hidden).toBe(false);

    document.dispatchEvent(new MouseEvent("mouseleave"));

    expect(tipOf().hidden).toBe(true);
  });

  test("goes when the marks do", () => {
    hover(marked(`<p id="p1" data-source-range="1:1-1:5">One</p>`, Date.now()));
    expect(tipOf().hidden).toBe(false);

    clear();

    expect(tipOf().hidden).toBe(true);
  });

  test("says when the document on the page was read, not one still on its way", () => {
    const now = Date.now();
    const line = marked(`<p id="p1" data-source-range="1:1-1:5">One</p>`, now - 2 * 60 * 60 * 1000);

    // The next document's changes, waiting for its render.
    set([], 2, now - 60 * 1000);
    hover(line);

    expect(tipOf().textContent).toBe("Added since you last read this, 2 hours ago");
  });

  test("opens above the pointer when there is no room below", () => {
    const line = marked(`<p id="p1" data-source-range="1:1-1:5">One</p>`, Date.now());
    hover(line);
    vi.spyOn(tipOf(), "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 200, 40));

    hover(line, window.innerHeight - 5);

    expect(tipOf().style.top).toBe(`${window.innerHeight - 5 - 12 - 40}px`);
  });
});

describe("the layer the marks are drawn in", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    setup();
  });

  test("is the app's own, not an element of the document that shares its name", () => {
    const body = page(
      `<p id="p1" data-source-range="1:1-1:5">One</p><div id="own" class="change-marks">the document's</div>`,
    );
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });

    mark(body, [{ kind: "added", start: 1, end: 1 }]);

    expect(document.getElementById("own")!.textContent).toBe("the document's");
    expect(lines()).toHaveLength(1);
  });

  test("takes the tip down when it draws the marks again", () => {
    const body = page(`<p id="p1" data-source-range="1:1-1:5">One</p>`);
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });
    mark(body, [{ kind: "added", start: 1, end: 1 }]);
    lines()[0].dispatchEvent(
      new MouseEvent("mousemove", { bubbles: true, clientX: 110, clientY: 5 }),
    );
    const tip = document.querySelector<HTMLElement>("body > .change-tip")!;
    expect(tip.hidden).toBe(false);

    redraw();

    expect(tip.hidden).toBe(true);
  });
});

describe("marks after the page is rendered again", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    setup();
  });

  test("go with the page they were drawn for", () => {
    const body = page(`<p id="p1" data-source-range="1:1-1:5">One</p>`);
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });
    mark(body, [{ kind: "added", start: 1, end: 1 }]);
    expect(lines()).toHaveLength(1);

    // Another document's HTML, whose changes are still being worked out.
    body.innerHTML = `<p id="p2" data-source-range="1:1-1:5">Two</p>`;
    afterRender();

    expect(lines()).toEqual([]);
  });

  test("go as soon as the page's HTML is replaced, before it is rendered", async () => {
    const body = page(`<p id="p1" data-source-range="1:1-1:5">One</p>`);
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });
    mark(body, [{ kind: "added", start: 1, end: 1 }]);

    body.innerHTML = `<p id="p2" data-source-range="1:1-1:5">Two</p>`;
    await Promise.resolve();

    expect(lines()).toEqual([]);
  });

  test("follow blocks that moved without the page changing size", () => {
    const body = page(`<p id="p1" data-source-range="1:1-1:5">One</p>`);
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });
    mark(body, [{ kind: "added", start: 1, end: 1 }]);

    vi.spyOn(document.getElementById("p1")!, "getBoundingClientRect").mockReturnValue(
      new DOMRect(124, 40, 700, 20),
    );
    afterRender();

    expect(lines().map((line) => line.style.top)).toEqual(["40px"]);
  });
});

describe("the page the marks are for", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    setup();
  });

  test("is the viewer's, not a lens answer drawn before it", () => {
    document.body.innerHTML = `<div class="header"><div class="markdown-body">a lens answer</div></div>`;
    document.body.insertAdjacentHTML(
      "beforeend",
      `<div class="content"><div class="markdown-viewer"><article class="markdown-body" data-render-generation="1"><p id="p1" data-source-range="1:1-1:5">One</p></article><div data-arto-change-marks></div></div></div>`,
    );
    layout(document.querySelector<HTMLElement>(".markdown-viewer > .markdown-body")!, {
      p1: new DOMRect(124, 0, 700, 20),
    });

    set([{ kind: "added", start: 1, end: 1 }], 1, null);

    expect(document.getElementById("p1")!.dataset.change).toBe("added");
    expect(lines()).toHaveLength(1);
  });

  test("leave a hidden tip alone rather than writing to it again", () => {
    const body = page(`<p id="p1" data-source-range="1:1-1:5">One</p>`);
    layout(body, { p1: new DOMRect(124, 0, 700, 20) });
    mark(body, [{ kind: "added", start: 1, end: 1 }]);
    lines()[0].dispatchEvent(
      new MouseEvent("mousemove", { bubbles: true, clientX: 110, clientY: 5 }),
    );
    redraw();
    const tip = document.querySelector<HTMLElement>("body > .change-tip")!;
    const writes = new MutationObserver(() => {});
    writes.observe(tip, { attributes: true });

    redraw();

    expect(writes.takeRecords()).toHaveLength(0);
  });
});
