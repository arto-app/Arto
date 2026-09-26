import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { clearCursor, next, peekCurrentElement, setFromContextTarget } from "./content-cursor";
import {
  CURRENT,
  DRAWN,
  HOLD,
  READING_LINE,
  SETTLE_MS,
  collectUnits,
  drawFocus,
  forgetStep,
  invalidateUnits,
  nextStep,
  pickUnit,
  readingLine,
  settleOffset,
  setupFocusScrolling,
  stepFocus,
  teardownFocusScrolling,
} from "./focus-mode";

describe("readingLine", () => {
  test("is the middle of the window, where the block being read is held", () => {
    expect(READING_LINE).toBe(0.5);
    expect(readingLine(1000)).toBe(500);
  });
});

describe("pickUnit", () => {
  // Three blocks at 0-100, 120-300 and 300-400, with a gap after the first.
  const tops = [0, 120, 300];
  const bottoms = [100, 300, 400];
  const pick = (line: number, prev = -1): number =>
    pickUnit(
      tops.length,
      (i) => tops[i],
      (i) => bottoms[i],
      line,
      prev,
    );

  test("is nothing on an empty page", () => {
    expect(
      pickUnit(
        0,
        () => 0,
        () => 0,
        10,
        -1,
      ),
    ).toBe(-1);
  });

  test("is the block the line runs through", () => {
    expect(pick(50)).toBe(0);
    expect(pick(200)).toBe(1);
    expect(pick(350)).toBe(2);
  });

  test("is the block above when the line falls in a gap", () => {
    expect(pick(110)).toBe(0);
  });

  test("is the first block when the line is above everything", () => {
    expect(pick(-20)).toBe(0);
  });

  test("is the last block when the line is below everything", () => {
    expect(pick(900)).toBe(2);
  });

  test("keeps the block it had while the line is just past its edge", () => {
    expect(pick(300 + HOLD, 1)).toBe(1);
    expect(pick(300 + HOLD + 1, 1)).toBe(2);
    expect(pick(120 - HOLD, 1)).toBe(1);
  });

  test("ignores a previous block that no longer exists", () => {
    expect(pick(50, 7)).toBe(0);
  });
});

describe("collectUnits", () => {
  beforeEach(() => invalidateUnits());
  afterEach(() => {
    document.body.innerHTML = "";
  });

  function body(html: string): HTMLElement {
    document.body.innerHTML = `<article class="markdown-body">${html}</article>`;
    return document.querySelector<HTMLElement>(".markdown-body")!;
  }

  test("takes each top-level block, a list whole", () => {
    const units = collectUnits(
      body("<h1>Title</h1><p>Para</p><ul><li>a</li><li>b</li></ul><ol><li>c</li></ol>"),
    );
    expect(units.map((u) => u.tagName)).toEqual(["H1", "P", "UL", "OL"]);
  });

  test("keeps tables, quotes and code whole", () => {
    const units = collectUnits(
      body(
        "<table><tr><td>1</td></tr><tr><td>2</td></tr></table><blockquote><p>x</p><p>y</p></blockquote><pre><code>z</code></pre>",
      ),
    );
    expect(units.map((u) => u.tagName)).toEqual(["TABLE", "BLOCKQUOTE", "PRE"]);
  });

  test("skips what has nothing to read", () => {
    const units = collectUnits(body("<p>a</p><hr><p>   </p><p><img src='x.png'></p>"));
    expect(units.map((u) => u.tagName)).toEqual(["P", "P"]);
    expect(units[1].querySelector("img")).not.toBeNull();
  });

  test("needs no source ranges, so a page lens's blocks count too", () => {
    const units = collectUnits(body("<p>plain</p><p data-source-range='1:1-1:5'>ranged</p>"));
    expect(units).toHaveLength(2);
  });

  test("collects again once told the document changed", () => {
    const element = body("<p>a</p>");
    expect(collectUnits(element)).toHaveLength(1);
    element.insertAdjacentHTML("beforeend", "<p>b</p>");
    invalidateUnits();
    expect(collectUnits(element)).toHaveLength(2);
  });
});

describe("drawFocus", () => {
  function page(
    focusing: boolean,
    first = "<p>one</p>",
  ): {
    content: HTMLElement;
    blocks: HTMLElement[];
  } {
    document.body.innerHTML = `
      <div class="main-area${focusing ? " focus-mode" : ""}">
        <div class="content">
          <article class="markdown-body">${first}<p>two</p><p>three</p></article>
        </div>
      </div>`;
    const content = document.querySelector<HTMLElement>(".content")!;
    const blocks = Array.from(content.querySelectorAll<HTMLElement>("p"));
    // Lay the blocks out at 100px each, one after the other, in a 200px
    // window scrolled to 100px.
    Object.defineProperty(content, "clientHeight", { value: 200, configurable: true });
    Object.defineProperty(content, "scrollHeight", { value: 300, configurable: true });
    content.scrollTop = 100;
    vi.spyOn(content, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 500, 200));
    blocks.forEach((block, i) => {
      vi.spyOn(block, "getBoundingClientRect").mockReturnValue(
        new DOMRect(0, i * 100 - 100, 500, 100),
      );
    });
    return { content, blocks };
  }

  beforeEach(() => invalidateUnits());
  afterEach(() => {
    clearCursor();
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  test("marks the block on the reading line while focusing", () => {
    const { content, blocks } = page(true);
    drawFocus(content);

    const body = content.querySelector(".markdown-body")!;
    expect(body.hasAttribute(DRAWN)).toBe(true);
    expect(blocks.map((b) => b.hasAttribute(CURRENT))).toEqual([false, false, true]);
  });

  test("marks nothing outside focus mode, and takes back what it marked", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    content.closest(".main-area")!.classList.remove("focus-mode");

    drawFocus(content);

    expect(content.querySelector(".markdown-body")!.hasAttribute(DRAWN)).toBe(false);
    expect(blocks.some((b) => b.hasAttribute(CURRENT))).toBe(false);
  });

  test("follows the content cursor before the reading line", () => {
    const { content, blocks } = page(true);
    // The cursor lands on the first block the window shows, while the
    // reading line is on the last.
    next();

    drawFocus(content);

    expect(blocks.map((b) => b.hasAttribute(CURRENT))).toEqual([false, true, false]);
  });

  test("lets go of a cursor the reader has scrolled away from", () => {
    const { content, blocks } = page(true);
    // Above the window: the reader scrolled on without the keys.
    setFromContextTarget(blocks[0]);

    drawFocus(content);

    expect(blocks.map((b) => b.hasAttribute(CURRENT))).toEqual([false, false, true]);
  });

  test("follows a cursor on a list while any of the list is on screen", () => {
    const { content } = page(true, "<ul><li>a</li><li>b</li></ul>");
    const list = content.querySelector<HTMLElement>("ul")!;
    vi.spyOn(list, "getBoundingClientRect").mockReturnValue(new DOMRect(0, -100, 500, 150));
    setFromContextTarget(list.querySelector<HTMLElement>("li")!);

    drawFocus(content);

    expect(list.hasAttribute(CURRENT)).toBe(true);
  });

  test("owns the mark, even when the document wrote one of its own", () => {
    const { content, blocks } = page(true, "<p data-focus-current>one</p>");

    drawFocus(content);

    expect(blocks.map((b) => b.hasAttribute(CURRENT))).toEqual([false, false, true]);
  });
});

describe("nextStep", () => {
  // A 200px window; blocks measured from its top.
  const viewport = 200;

  test("moves to the next block when the one being read is all in view", () => {
    expect(nextStep(3, 1, 1, 50, 150, viewport)).toEqual({ kind: "to", index: 2 });
  });

  test("moves to the previous block going up", () => {
    expect(nextStep(3, 1, -1, 50, 150, viewport)).toEqual({ kind: "to", index: 0 });
  });

  test("scrolls through a block that runs below the window before leaving it", () => {
    expect(nextStep(3, 1, 1, -100, 400, viewport)).toEqual({ kind: "within" });
  });

  test("scrolls back through a block that runs above the window before leaving it", () => {
    expect(nextStep(3, 1, -1, -100, 400, viewport)).toEqual({ kind: "within" });
  });

  test("leaves a tall block once its end is in view", () => {
    expect(nextStep(3, 1, 1, -300, 180, viewport)).toEqual({ kind: "to", index: 2 });
  });

  test("moves on from a short block that only hangs off the window", () => {
    // Scrolling a line through it would be pulled back by the settle: it fits.
    expect(nextStep(3, 1, 1, 100, 250, viewport)).toEqual({ kind: "to", index: 2 });
    expect(nextStep(3, 1, -1, -50, 100, viewport)).toEqual({ kind: "to", index: 0 });
  });

  test("goes nowhere past either end", () => {
    expect(nextStep(3, 2, 1, 50, 150, viewport)).toEqual({ kind: "none" });
    expect(nextStep(3, 0, -1, 50, 150, viewport)).toEqual({ kind: "none" });
  });
});

describe("stepFocus", () => {
  afterEach(() => {
    clearCursor();
    vi.restoreAllMocks();
    document.body.innerHTML = "";
    invalidateUnits();
  });

  function page(focusing: boolean): { content: HTMLElement; blocks: HTMLElement[] } {
    document.body.innerHTML = `
      <div class="main-area${focusing ? " focus-mode" : ""}">
        <div class="content">
          <article class="markdown-body"><p>one</p><table><tr><td>two</td></tr></table><p>three</p></article>
        </div>
      </div>`;
    const content = document.querySelector<HTMLElement>(".content")!;
    const blocks = Array.from(content.querySelectorAll<HTMLElement>(".markdown-body > *"));
    Object.defineProperty(content, "clientHeight", { value: 200, configurable: true });
    vi.spyOn(content, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 500, 200));
    // The table is in the middle, whole.
    [-100, 50, 250].forEach((top, i) => {
      vi.spyOn(blocks[i], "getBoundingClientRect").mockReturnValue(new DOMRect(0, top, 500, 100));
    });
    return { content, blocks };
  }

  test("brings the next block to the middle instead of scrolling a line", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "center" });
  });

  test("lets go of the content cursor, which would otherwise hold the block it left", () => {
    const { content, blocks } = page(true);
    setFromContextTarget(blocks[1]);
    drawFocus(content);

    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "center" });
    expect(peekCurrentElement()).toBeNull();
  });

  test("takes each press from where the last one is going, not from where the page still is", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    // A held key repeats faster than the page travels.
    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "center" });
    expect(stepFocus(content, -1)).toEqual({ kind: "bring", block: blocks[1], place: "center" });
    expect(stepFocus(content, -1)).toEqual({ kind: "bring", block: blocks[0], place: "center" });
  });

  test("goes on past a tall block it is still on its way to", () => {
    const { content, blocks } = page(true);
    // The last block is taller than the window and still below it.
    vi.spyOn(blocks[2], "getBoundingClientRect").mockReturnValue(new DOMRect(0, 250, 500, 400));
    drawFocus(content);
    // Its start, not its middle: the middle would leave the start unread.
    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "start" });
    // Not "scroll through it": the page is not there yet.
    expect(stepFocus(content, 1)).toEqual({ kind: "stay" });
    expect(stepFocus(content, -1)).toEqual({ kind: "bring", block: blocks[1], place: "center" });
  });

  test("brings a tall block in by its end going up", () => {
    const { content, blocks } = page(true);
    vi.spyOn(blocks[0], "getBoundingClientRect").mockReturnValue(new DOMRect(0, -500, 500, 400));
    drawFocus(content);
    expect(stepFocus(content, -1)).toEqual({ kind: "bring", block: blocks[0], place: "end" });
  });

  test("forgets where a key was going once the page is scrolled another way", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    stepFocus(content, 1);

    forgetStep();

    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "center" });
  });

  test("forgets where a key was going once focus mode is left", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    stepFocus(content, 1);
    content.closest(".main-area")!.classList.remove("focus-mode");
    drawFocus(content);
    content.closest(".main-area")!.classList.add("focus-mode");

    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "center" });
  });

  test("stays at the last block", () => {
    const { content, blocks } = page(true);
    // The last block is on the middle line.
    [-250, -100, 50].forEach((top, i) => {
      vi.spyOn(blocks[i], "getBoundingClientRect").mockReturnValue(new DOMRect(0, top, 500, 100));
    });
    drawFocus(content);
    expect(stepFocus(content, 1)).toEqual({ kind: "stay" });
  });

  test("leaves the line scroll to the caller outside focus mode", () => {
    const { content } = page(false);
    expect(stepFocus(content, 1)).toEqual({ kind: "scroll" });
  });
});

describe("settleOffset", () => {
  const viewport = 200;

  test("is how far a block is from the middle of the window", () => {
    expect(settleOffset(80, 160, viewport)).toBe(20);
    expect(settleOffset(40, 120, viewport)).toBe(-20);
  });

  test("is nothing for a block already in the middle", () => {
    expect(settleOffset(50, 150, viewport)).toBeNull();
    expect(settleOffset(50.5, 150.5, viewport)).toBeNull();
  });

  test("is nothing for a block taller than the window, which is being read through", () => {
    expect(settleOffset(-100, 300, viewport)).toBeNull();
  });
});

describe("focus scrolling", () => {
  function page(focusing: boolean, height = 100) {
    document.body.innerHTML = `
      <div class="main-area${focusing ? " focus-mode" : ""}">
        <div class="content"><article class="markdown-body"><p>one</p><p>two</p><p>three</p></article></div>
      </div>`;
    const content = document.querySelector<HTMLElement>(".content")!;
    const blocks = Array.from(content.querySelectorAll<HTMLElement>("p"));
    Object.defineProperty(content, "clientHeight", { value: 200, configurable: true });
    vi.spyOn(content, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 500, 200));
    // The middle block is on the line, 30px below the middle.
    [-100, 80, 280].forEach((top, i) => {
      vi.spyOn(blocks[i], "getBoundingClientRect").mockReturnValue(
        new DOMRect(0, top, 500, i === 1 ? height : 100),
      );
    });
    return { content, blocks };
  }

  const bring = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    bring.mockClear();
    setupFocusScrolling(bring);
  });

  afterEach(() => {
    teardownFocusScrolling();
    vi.useRealTimers();
    vi.restoreAllMocks();
    document.body.innerHTML = "";
    invalidateUnits();
  });

  test("brings the block being read to the middle on the way into focus mode", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS - 1);
    expect(bring).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(bring).toHaveBeenCalledWith(blocks[1]);
  });

  test("leaves a block taller than the window where the reader has it, even then", () => {
    const { content } = page(true, 400);
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS);
    expect(bring).not.toHaveBeenCalled();
  });

  test("leaves the page where the reader scrolled it", () => {
    const { content } = page(true);
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS);
    bring.mockClear();

    content.dispatchEvent(new Event("scroll"));
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS * 10);
    expect(bring).not.toHaveBeenCalled();
  });

  test("forgets where a key was going once the reader takes the page with the wheel", () => {
    const { content, blocks } = page(true);
    drawFocus(content);
    stepFocus(content, 1);

    content.dispatchEvent(new Event("wheel"));

    expect(stepFocus(content, 1)).toEqual({ kind: "bring", block: blocks[2], place: "center" });
  });

  test("leaves the page to a reader who scrolls before it is brought in", () => {
    const { content } = page(true);
    drawFocus(content);
    document.dispatchEvent(new Event("wheel"));
    vi.advanceTimersByTime(SETTLE_MS);
    expect(bring).not.toHaveBeenCalled();
  });

  test("leaves the page to a line key pressed before it is brought in", () => {
    const { content } = page(true);
    drawFocus(content);
    stepFocus(content, 1);
    vi.advanceTimersByTime(SETTLE_MS);
    expect(bring).not.toHaveBeenCalled();
  });

  test("does not take a search closing for a way in", () => {
    const { content } = page(true);
    const area = content.closest<HTMLElement>(".main-area")!;
    area.classList.add("focus-layout");
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS);
    bring.mockClear();

    // A search pauses the dimming and keeps the layout.
    area.classList.remove("focus-mode");
    drawFocus(content);
    area.classList.add("focus-mode");
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS);

    expect(bring).not.toHaveBeenCalled();
  });

  test("does nothing outside focus mode", () => {
    const { content } = page(false);
    drawFocus(content);
    vi.advanceTimersByTime(SETTLE_MS);
    expect(bring).not.toHaveBeenCalled();
  });
});
