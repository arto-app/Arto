/**
 * Focus mode's half in the page: which block is being read.
 *
 * The window decides whether it is focusing — `crates/arto/src/state/app_state/focus_mode.rs`
 * — and says so with a `focus-mode` class on the main area, which folds the
 * header and lets the stylesheet dim the page. What the window cannot know is
 * where the reader is, so this marks the one block to leave alone, from the
 * same frame's work as the rest of `reading-position.ts`.
 */

import { clearCursor, peekCurrentElement } from "./content-cursor";

/** The attribute on the block being read. */
export const CURRENT = "data-focus-current";

/**
 * The attribute on the page once a block has been marked.
 *
 * The stylesheet dims nothing until it is there: dimming as soon as the class
 * arrives would dim the block being read too, for the frame before it is
 * marked, and the fade would flicker on the one block that should stay put.
 */
export const DRAWN = "data-focus-drawn";

/**
 * How far down the window the reading line sits, as a share of its height.
 *
 * The middle, where a key or the way into focus mode brings the block being
 * read. A fixed line crosses the page at the speed it scrolls, so the
 * blocks are met one after another; the page is padded above and below so the
 * first and last can reach it too.
 */
export const READING_LINE = 0.5;

/**
 * How far past its edge the reading line can go before the block it was on
 * lets go.
 *
 * Two blocks meet at a single pixel, and a scroll that rests on it would flick
 * the dimming back and forth between them.
 */
export const HOLD = 8;

/** What counts as something to look at in a block without text. */
const MEDIA = "img, svg, video, canvas, iframe, picture";

/** Where the reading line is, measured from the top of the window. */
export function readingLine(viewport: number): number {
  return viewport * READING_LINE;
}

/**
 * Which block the reading line is in.
 *
 * `top` and `bottom` measure a block by its index, in the same coordinates as
 * `line`, and are only asked about the blocks a binary search visits: this
 * runs on every frame of a scroll. A line in the gap between two blocks is in
 * the one above; a line above the first block is in the first. `prev` is the
 * block marked last time, kept while the line is within [`HOLD`] of it.
 *
 * `-1` when there are no blocks.
 */
export function pickUnit(
  count: number,
  top: (index: number) => number,
  bottom: (index: number) => number,
  line: number,
  prev: number,
): number {
  if (count === 0) {
    return -1;
  }
  if (prev >= 0 && prev < count && top(prev) - HOLD <= line && line <= bottom(prev) + HOLD) {
    return prev;
  }
  let low = 0;
  let high = count - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (top(middle) <= line) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}

let cachedBody: Element | null = null;
let cachedUnits: HTMLElement[] = [];
let previous = -1;
/**
 * The block a line-scroll key is taking the page to, until the page gets there.
 *
 * A held key repeats faster than the page travels, and each press has to go on
 * from the last one's block rather than from the one still under the line.
 */
let heading = -1;

/**
 * Whether the way into this spell of focus mode has been seen.
 *
 * A search pauses the dimming, which takes the drawn mark away, but not focus
 * mode: coming back from it is not a way in, and must not bring the page
 * away from the match the reader was looking at. Only leaving the layout —
 * focus mode itself — starts a new spell.
 */
let entered = false;
let marked: HTMLElement | null = null;

function hasContent(element: Element): boolean {
  return (
    (element.textContent ?? "").trim() !== "" ||
    element.matches(MEDIA) ||
    element.querySelector(MEDIA) !== null
  );
}

/**
 * The blocks the page is dimmed by, in document order.
 *
 * Each block at the top of the page: a list, a table, a quote, an alert, code
 * and a diagram are each one. A list is not split into its items, which are
 * often a single line each — too short to be held one at a time, a scroll
 * passing several at once. A block taller than the window is scrolled
 * through before the next is reached. What has nothing to read — a rule, an
 * empty paragraph — is never the block being read.
 *
 * The page is walked by its elements rather than by `data-source-range`,
 * which a page lens's answer does not carry.
 *
 * Cached until [`invalidateUnits`]; the checks on both ends cover the frames
 * between a document being replaced and the render that says so.
 */
export function collectUnits(body: Element): HTMLElement[] {
  const first = cachedUnits[0];
  const last = cachedUnits[cachedUnits.length - 1];
  if (body === cachedBody && first?.isConnected && last?.isConnected) {
    return cachedUnits;
  }
  const units: HTMLElement[] = [];
  for (const child of Array.from(body.children)) {
    if (!(child instanceof HTMLElement)) {
      continue;
    }
    if (hasContent(child)) {
      units.push(child);
    }
  }
  // The mark is this module's to give. A document's own HTML can carry the
  // attribute too, and a block it named would otherwise stay lit for good.
  for (const stray of Array.from(body.querySelectorAll(`[${CURRENT}]`))) {
    if (stray !== marked) {
      stray.removeAttribute(CURRENT);
    }
  }
  cachedBody = body;
  cachedUnits = units;
  previous = -1;
  return units;
}

/**
 * Forget the blocks; the document has been replaced.
 *
 * Nothing is written to the page here: this runs at the start of every batch
 * render, and the next frame's [`drawFocus`] moves the mark where it belongs.
 */
export function invalidateUnits(): void {
  cachedBody = null;
  cachedUnits = [];
  previous = -1;
  heading = -1;
}

function mark(unit: HTMLElement | null): void {
  if (marked === unit) {
    return;
  }
  marked?.removeAttribute(CURRENT);
  unit?.setAttribute(CURRENT, "");
  marked = unit;
}

/**
 * The block the content cursor is in, if it is on one.
 *
 * Only while it is on screen: a reader who stepped to a block and then
 * scrolled on with the wheel has moved on, and the cursor left behind above
 * the window is not where they are reading.
 */
function unitOfCursor(units: HTMLElement[], body: Element, content: HTMLElement): number {
  const cursor = peekCurrentElement();
  if (!cursor || !body.contains(cursor)) {
    return -1;
  }
  const index = units.findIndex(
    (unit) => unit === cursor || unit.contains(cursor) || cursor.contains(unit),
  );
  if (index < 0) {
    return -1;
  }
  // The block, not the cursor: a cursor on an item of a long list is the
  // list, which is still being read while any of it is on screen.
  const view = content.getBoundingClientRect();
  const rect = units[index].getBoundingClientRect();
  return rect.bottom <= view.top || rect.top >= view.bottom ? -1 : index;
}

/**
 * Mark the block being read, or take the mark away outside focus mode.
 *
 * The keyboard's cursor comes first: a reader stepping through blocks with it
 * is saying which one they are on, and it moves without the page scrolling.
 * It has to be in the window for that; see [`unitOfCursor`].
 * Otherwise it is the block under the reading line.
 */
export function drawFocus(content: HTMLElement): void {
  const body = content.querySelector<HTMLElement>(".markdown-body");
  if (!body || !content.closest(".focus-mode")) {
    body?.removeAttribute(DRAWN);
    previous = -1;
    heading = -1;
    if (!content.closest(".focus-layout")) {
      entered = false;
    }
    mark(null);
    return;
  }

  const units = collectUnits(body);
  let index = unitOfCursor(units, body, content);
  if (index < 0) {
    const origin = content.getBoundingClientRect().top;
    const line = readingLine(content.clientHeight);
    index = pickUnit(
      units.length,
      (i) => units[i].getBoundingClientRect().top - origin,
      (i) => units[i].getBoundingClientRect().bottom - origin,
      line,
      previous,
    );
  }
  // The page has got where a key was taking it.
  if (index === heading) {
    heading = -1;
  }
  // Coming into focus mode moves nothing, so nothing else would bring the
  // block being read to the middle.
  const entering = index >= 0 && !entered;
  if (index >= 0) {
    entered = true;
  }
  previous = index;
  mark(index >= 0 ? units[index] : null);
  body.toggleAttribute(DRAWN, index >= 0);
  if (entering) {
    scheduleSettle(content);
  }
}

/** Where a step of the line-scroll keys takes the reader in focus mode. */
export type Step = { kind: "to"; index: number } | { kind: "within" } | { kind: "none" };

/**
 * Where one press of a line-scroll key goes while focusing.
 *
 * A press moves to the next block, the unit the page is read in while
 * focusing, and brings it to the middle — unless the block being read is
 * taller than the window and runs past it on that side, when the press
 * scrolls through it as usual. `top` and `bottom` are the block's, measured from
 * the top of the window.
 */
export function nextStep(
  count: number,
  current: number,
  direction: 1 | -1,
  top: number,
  bottom: number,
  viewport: number,
): Step {
  if (current >= 0 && bottom - top > viewport) {
    if (direction > 0 && bottom > viewport + 1) {
      return { kind: "within" };
    }
    if (direction < 0 && top < -1) {
      return { kind: "within" };
    }
  }
  const index = current + direction;
  return index >= 0 && index < count ? { kind: "to", index } : { kind: "none" };
}

/**
 * What a line-scroll key does: scroll the line, stay put, or bring a block in,
 * placed where it can be read from — the middle for one that fits the window,
 * and for a taller one the end the reader arrives at, so none of it is passed
 * over.
 */
export type FocusStep =
  | { kind: "scroll" }
  | { kind: "stay" }
  | { kind: "bring"; block: HTMLElement; place: "center" | "start" | "end" };

/**
 * One step of a line-scroll key; see [`nextStep`].
 *
 * `scroll` outside focus mode, and inside a tall block. Moving on lets go of
 * the content cursor: the block it is on would otherwise stay the one being
 * read, and the mark would stay behind with it.
 */
export function stepFocus(content: HTMLElement, direction: 1 | -1): FocusStep {
  const body = content.querySelector<HTMLElement>(".markdown-body");
  if (!body || !content.closest(".focus-mode")) {
    return { kind: "scroll" };
  }
  keepPage();
  if (previous < 0) {
    drawFocus(content);
  }
  const units = collectUnits(body);
  // A block a key is still taking the page to is not being read through yet,
  // however tall: the next press goes on from it rather than scrolling in it.
  const current = heading >= 0 ? heading : previous;
  const origin = content.getBoundingClientRect().top;
  const rect = heading < 0 && current >= 0 ? units[current].getBoundingClientRect() : null;
  const step = nextStep(
    units.length,
    current,
    direction,
    rect ? rect.top - origin : 0,
    rect ? rect.bottom - origin : 0,
    content.clientHeight,
  );
  if (step.kind === "within") {
    return { kind: "scroll" };
  }
  if (step.kind === "none") {
    return { kind: "stay" };
  }
  clearCursor();
  heading = step.index;
  const block = units[step.index];
  const tall = block.getBoundingClientRect().height > content.clientHeight;
  const place = !tall ? "center" : direction > 0 ? "start" : "end";
  return { kind: "bring", block, place };
}

/**
 * Forget where a key was taking the page: it has been scrolled another way — a
 * page at a time, to the top — and the next press goes on from where it is.
 */
export function forgetStep(): void {
  heading = -1;
  keepPage();
}

/**
 * The reader has moved the page themselves: it is not brought in on the way
 * into focus mode after that.
 */
function keepPage(): void {
  if (settleTimer !== null) {
    clearTimeout(settleTimer);
    settleTimer = null;
  }
}

/**
 * How long after coming into focus mode the block being read is brought to the
 * middle.
 *
 * Long enough for the header to have folded away, which moves the page.
 */
export const SETTLE_MS = 150;

/**
 * How far to scroll to bring a block to the middle of the window, or `null`
 * when there is nothing to do.
 *
 * Nothing for a block already there, and nothing for one taller than the
 * window: that one is being read through, and the reader decides where in it
 * the window is. `top` and `bottom` are measured from the top of the window.
 */
export function settleOffset(top: number, bottom: number, viewport: number): number | null {
  if (bottom - top > viewport) {
    return null;
  }
  const offset = (top + bottom) / 2 - viewport / 2;
  return Math.abs(offset) <= 1 ? null : offset;
}

let bringToMiddle: ((block: HTMLElement) => void) | null = null;
let settleTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Bring the block being read to the middle of the window.
 *
 * Only on the way into focus mode. After that the page is where the reader
 * scrolls it: a page pulled to a block once a scroll stops reads as being
 * held there, the way a snap does.
 */
function settle(content: HTMLElement): void {
  const body = content.querySelector<HTMLElement>(".markdown-body");
  if (!body || !content.closest(".focus-mode")) {
    return;
  }
  drawFocus(content);
  if (previous < 0) {
    return;
  }
  const origin = content.getBoundingClientRect().top;
  const block = collectUnits(body)[previous];
  const rect = block.getBoundingClientRect();
  if (settleOffset(rect.top - origin, rect.bottom - origin, content.clientHeight) !== null) {
    bringToMiddle?.(block);
  }
}

/** Settle `content` after [`SETTLE_MS`]. */
function scheduleSettle(content: HTMLElement): void {
  if (settleTimer !== null) {
    clearTimeout(settleTimer);
  }
  settleTimer = setTimeout(() => {
    settleTimer = null;
    settle(content);
  }, SETTLE_MS);
}

/**
 * The reader has taken the page with the wheel or a finger: wherever a key was
 * taking it, the next press goes on from where the page is.
 */
function onReaderScroll(): void {
  forgetStep();
}

/**
 * Set up how focus mode moves the page: once into the middle on the way in,
 * and on from where the reader takes it by hand.
 *
 * `bring` carries a block to the middle. It is the scroll controller's, so
 * that it gives up a destination still being held from a jump rather than
 * being pulled back to it by the next block drawn.
 */
export function setupFocusScrolling(bring: (block: HTMLElement) => void): void {
  bringToMiddle = bring;
  entered = false;
  document.addEventListener("wheel", onReaderScroll, { capture: true, passive: true });
  document.addEventListener("touchstart", onReaderScroll, { capture: true, passive: true });
}

/** Undo [`setupFocusScrolling`]. */
export function teardownFocusScrolling(): void {
  document.removeEventListener("wheel", onReaderScroll, { capture: true });
  document.removeEventListener("touchstart", onReaderScroll, { capture: true });
  if (settleTimer !== null) {
    clearTimeout(settleTimer);
    settleTimer = null;
  }
  entered = false;
  bringToMiddle = null;
}
