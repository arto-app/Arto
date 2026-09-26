/**
 * What changed in the document since the reader last read it, marked on the
 * blocks it changed.
 *
 * The app compares the version last read with the one on screen and hands
 * over the runs of lines that differ (`crates/arto/src/baselines/changes.rs`);
 * this finds the blocks those lines were rendered into through their
 * `data-source-range`. A block is marked with an attribute, so that nothing
 * that rewrites a block's contents — a diagram drawn, a page lens holding
 * the blocks aside — takes the mark away, and nothing that counts a block's
 * children finds one more. The marks are drawn from those attributes in a
 * layer beside the page (see [`draw`]).
 */

import { refreshReadingPosition } from "./reading-position";
import { renderCoordinator } from "./render-coordinator";
import { toElement } from "./scroll-controller";
import { readSourceRange, type SourceRange } from "./source-range";

/** A run of lines that changed, in the lines of the version on screen. */
export type Change =
  | { kind: "added"; start: number; end: number }
  | { kind: "modified"; start: number; end: number }
  | { kind: "removed"; after: number };

/** On a block the run changed: `added` or `modified`. */
const CHANGE = "data-change";
/** On the block lines were taken out beside: `after` it, or `before` it. */
const REMOVED = "data-change-removed";
/**
 * The layer beside the page the marks are drawn in, which the app renders as
 * the viewer's child. Named by an attribute of the app's own and found only
 * there: a document's HTML can carry any class, and an element of the
 * document taken for the layer would be emptied and drawn over.
 */
const LAYER = "[data-arto-change-marks]";
const LAYER_IN_VIEWER = `.markdown-viewer > ${LAYER}`;
/** What the render coordinator knows the tip by. */
const TIP = "data-arto-change-tip";
const FLASH = "change-flash";
const FLASH_MS = 1200;
/** How long a render is waited for before its changes are given up. */
const WAIT_MS = 5000;

interface Block {
  el: HTMLElement;
  range: SourceRange;
}

/** The observer waiting for the render the changes were computed for. */
let waiting: MutationObserver | null = null;

/** When the version the marks are measured from was read, ms since the epoch. */
let readAt: number | null = null;

/** The viewer's page; a lens answer in the header is a `.markdown-body` too. */
function root(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".content .markdown-viewer > .markdown-body");
}

/**
 * Mark `changes` on the page, once it shows the render `generation` — the
 * page may still hold the previous one when the changes arrive. `lastRead`
 * is when the version they are measured from was read.
 */
export function set(
  changes: Change[],
  generation: number | null,
  lastRead: number | null = null,
): void {
  waiting?.disconnect();
  waiting = null;
  const ready = (): HTMLElement | null => {
    const body = root();
    if (!body) return null;
    if (generation !== null && body.dataset.renderGeneration !== String(generation)) return null;
    return body;
  };
  const body = ready();
  // The time goes on the page with the marks it belongs to: the page may
  // still hold another document's, which keep their own.
  const apply = (body: HTMLElement): void => {
    readAt = lastRead;
    mark(body, changes);
  };
  if (body) {
    apply(body);
    return;
  }
  const observer = new MutationObserver(() => {
    const body = ready();
    if (!body) return;
    observer.disconnect();
    if (waiting === observer) waiting = null;
    apply(body);
  });
  observer.observe(document.body, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["data-render-generation"],
  });
  waiting = observer;
  setTimeout(() => {
    observer.disconnect();
    if (waiting === observer) waiting = null;
  }, WAIT_MS);
}

/** Take every mark off the page. */
export function clear(): void {
  waiting?.disconnect();
  waiting = null;
  const body = root();
  if (body) {
    unmark(body);
    draw(body);
  }
  refreshReadingPosition();
}

/** Replace the marks on `body` with those for `changes`. */
export function mark(body: HTMLElement, changes: Change[]): void {
  unmark(body);
  const blocks = blocksOf(body);
  for (const change of changes) {
    if (change.kind === "removed") {
      markRemoved(blocks, change.after);
      continue;
    }
    for (const el of changed(blocks, change.start, change.end)) {
      setKind(el, change.kind);
    }
  }
  draw(body);
  refreshReadingPosition();
}

function unmark(body: HTMLElement): void {
  hideTip();
  for (const el of Array.from(body.querySelectorAll<HTMLElement>(`[${CHANGE}], [${REMOVED}]`))) {
    el.removeAttribute(CHANGE);
    el.removeAttribute(REMOVED);
    el.classList.remove(FLASH);
  }
}

/**
 * Draw the marks of `body` in the layer beside it.
 *
 * The lines stand at a fixed distance left of the page's edge, however far in
 * the block sits — a paragraph in a quote or a list is marked in the same
 * margin as any other — and outside every block, so a table or a code block,
 * which scroll sideways and clip what lies outside them, cannot hide theirs.
 * Positions are in the layer's own pixels, which zoom scales along with the
 * page. Everything is measured before anything is written, so a document
 * rewritten throughout is laid out once rather than once per mark.
 */
function draw(body: HTMLElement): void {
  const layer = body.parentElement?.querySelector<HTMLElement>(`:scope > ${LAYER}`);
  if (!layer) return;
  watch(body, layer);
  const frame = layer.getBoundingClientRect();
  const scale = layer.offsetWidth > 0 ? frame.width / layer.offsetWidth : 1;
  const x = (at: number): number => Math.round((at - frame.left) / (scale || 1));
  const y = (at: number): number => Math.round((at - frame.top) / (scale || 1));
  const page = x(body.getBoundingClientRect().left);
  const measured = Array.from(body.querySelectorAll<HTMLElement>(`[${CHANGE}], [${REMOVED}]`)).map(
    (el) => ({ el, rect: el.getBoundingClientRect() }),
  );
  const drawn: HTMLElement[] = [];
  for (const { el, rect } of measured) {
    // Not laid out — in a closed details — and so nowhere to draw; drawn
    // when the page next changes size, as opening it does.
    if (rect.width === 0 && rect.height === 0) continue;
    const kind = el.getAttribute(CHANGE);
    if (kind) {
      const line = document.createElement("div");
      line.className = "change-mark";
      line.dataset.kind = kind;
      line.style.left = `${page - 15}px`;
      line.style.top = `${y(rect.top)}px`;
      line.style.height = `${Math.round(rect.height / (scale || 1))}px`;
      hostOf.set(line, el);
      drawn.push(line);
    }
    const removed = el.getAttribute(REMOVED);
    if (removed) {
      const hairline = document.createElement("div");
      hairline.className = "change-mark-removed";
      hairline.style.left = `${page - 14}px`;
      hairline.style.top =
        removed === "after" ? `${y(rect.bottom) + 3}px` : `${y(rect.top) - 13}px`;
      hostOf.set(hairline, el);
      drawn.push(hairline);
    }
  }
  // A tip over a mark about to be replaced would outlive it.
  hideTip();
  layer.replaceChildren(...drawn);
}

/** Draw the marks again: the page changed size under them. */
export function redraw(): void {
  const body = root();
  if (body) draw(body);
  else unwatch();
}

/**
 * Draw the marks again after every render of the page, which is how the page
 * says its contents changed: another document's HTML, whose own changes are
 * still being worked out and whose predecessor's marks must not stand over it,
 * or a page lens trading blocks for others of the same height, which no size
 * change would report. Render-complete callbacks are called once, so it asks
 * to be called again.
 */
export function afterRender(): void {
  redraw();
  renderCoordinator.onRenderComplete(afterRender);
}

/** The block each drawn mark stands for. */
const hostOf = new WeakMap<Element, HTMLElement>();

let resizes: ResizeObserver | null = null;
let swaps: MutationObserver | null = null;
let watched: HTMLElement | null = null;

/**
 * Draw the marks again whenever the page changes size — a window resized, a
 * diagram drawn, a font arrived, a details opened — since each moves what
 * lies below it. Zoom alone does not: the layer is zoomed with the page.
 *
 * And as soon as the page is given other HTML: the marks stand beside it
 * rather than in it, so they would otherwise stay over the next document
 * until it was rendered, which a diagram can make take a while.
 */
function watch(body: HTMLElement, layer: HTMLElement): void {
  if (watched === body) return;
  unwatch();
  if (typeof ResizeObserver !== "undefined") {
    resizes ??= new ResizeObserver(() => redraw());
    resizes.observe(body);
    // The layer spans the viewer: a page at its widest keeps its size while
    // the window changes, and only moves, which the page's own size would not
    // say.
    resizes.observe(layer);
  }
  swaps ??= new MutationObserver(() => redraw());
  swaps.observe(body, { childList: true });
  watched = body;
}

/** Let go of a page that is no longer shown. */
function unwatch(): void {
  resizes?.disconnect();
  swaps?.disconnect();
  watched = null;
}

/** A block rewritten by one run and added to by another was rewritten. */
function setKind(el: HTMLElement, kind: "added" | "modified"): void {
  if (el.getAttribute(CHANGE) === "modified") return;
  el.setAttribute(CHANGE, kind);
}

/** Every element the page names a source range for, in document order. */
function blocksOf(body: HTMLElement): Block[] {
  const blocks: Block[] = [];
  for (const el of Array.from(body.querySelectorAll<HTMLElement>("[data-source-range]"))) {
    const range = readSourceRange(el);
    if (range) blocks.push({ el, range });
  }
  return blocks;
}

/**
 * The element a mark on `el` is drawn on.
 *
 * A code block's content has a range of its own, and so does every cell of
 * a table; a mark beside a line of code or in the middle of a row would be
 * beside nothing a reader thinks of as the block.
 */
function host(el: HTMLElement): HTMLElement {
  if (el.tagName === "CODE" && el.parentElement?.tagName === "PRE") return el.parentElement;
  return el.closest<HTMLElement>("table") ?? el;
}

function overlaps(range: SourceRange, start: number, end: number): boolean {
  return range.start.line <= end && range.end.line >= start;
}

/**
 * The blocks lines `start`–`end` were rendered into: those they reach whose
 * own blocks they do not reach. A paragraph inside a list item is marked,
 * not the list; a line between the items of a list, which only the list
 * covers, marks the list.
 */
function changed(blocks: Block[], start: number, end: number): HTMLElement[] {
  const hit = blocks.filter(({ range }) => overlaps(range, start, end));
  const leaves = hit.filter(
    ({ el }) => !hit.some((other) => other.el !== el && el.contains(other.el)),
  );
  return [...new Set(leaves.map(({ el }) => host(el)))];
}

/**
 * Mark where lines went, from just after line `after`.
 *
 * Taken from the middle of a block, they changed that block. Otherwise the
 * hairline goes under the block that ends last before them — the innermost,
 * so lines taken from a list are marked in it — or over the first block
 * when they were taken from the top.
 */
function markRemoved(blocks: Block[], after: number): void {
  if (blocks.length === 0) return;
  const containing = blocks.filter(
    ({ range }) => range.start.line <= after && after <= range.end.line,
  );
  const innermost = containing[containing.length - 1];
  const isLeaf = (el: HTMLElement): boolean =>
    !blocks.some((other) => other.el !== el && el.contains(other.el));
  if (innermost && innermost.range.end.line > after && isLeaf(innermost.el)) {
    setKind(host(innermost.el), "modified");
    return;
  }
  let before: Block | null = null;
  for (const block of blocks) {
    if (block.range.end.line > after) continue;
    if (!before || block.range.end.line >= before.range.end.line) before = block;
  }
  if (before) {
    host(before.el).setAttribute(REMOVED, "after");
    return;
  }
  const first = blocks.reduce((a, b) => (b.range.start.line < a.range.start.line ? b : a));
  host(first.el).setAttribute(REMOVED, "before");
}

/** The marks, in the order they are read. */
function marks(): HTMLElement[] {
  const body = root();
  if (!body) return [];
  return Array.from(body.querySelectorAll<HTMLElement>(`[${CHANGE}], [${REMOVED}]`));
}

/** The middle of the reading area, and of `el`, in the same coordinates. */
function middles(el: HTMLElement): { reading: number; mark: number } | null {
  const content = document.querySelector<HTMLElement>(".content");
  if (!content) return null;
  const frame = content.getBoundingClientRect();
  const rect = el.getBoundingClientRect();
  return {
    reading: frame.top + content.clientHeight / 2,
    mark: rect.top + rect.height / 2,
  };
}

function arrive(el: HTMLElement): void {
  toElement(el, "center");
  el.classList.remove(FLASH);
  // Read back so the animation starts again on a mark just flashed.
  void el.offsetWidth;
  el.classList.add(FLASH);
  setTimeout(() => el.classList.remove(FLASH), FLASH_MS);
}

/** Go to the first mark below the middle of the reading area. */
export function next(): boolean {
  const target = marks().find((el) => {
    const at = middles(el);
    return at !== null && at.mark > at.reading + 1;
  });
  if (target) arrive(target);
  return target !== undefined;
}

/** Go to the last mark above the middle of the reading area. */
export function prev(): boolean {
  const target = marks()
    .reverse()
    .find((el) => {
      const at = middles(el);
      return at !== null && at.mark < at.reading - 1;
    });
  if (target) arrive(target);
  return target !== undefined;
}

/** Go to the first mark in the document. */
export function first(): boolean {
  const target = marks()[0];
  if (target) arrive(target);
  return target !== undefined;
}

const MINUTE = 60 * 1000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** How long ago `readAt` was, as a reader would say it: "2 hours ago". */
export function since(readAt: number, now: number): string {
  const elapsed = Math.max(0, now - readAt);
  if (elapsed < MINUTE) return "just now";
  const relative = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
  if (elapsed < HOUR) return relative.format(-Math.floor(elapsed / MINUTE), "minute");
  if (elapsed < DAY) return relative.format(-Math.floor(elapsed / HOUR), "hour");
  const days = Math.floor(elapsed / DAY);
  if (days < 7) return relative.format(-days, "day");
  if (days < 30) return relative.format(-Math.floor(days / 7), "week");
  if (days < 365) return relative.format(-Math.floor(days / 30), "month");
  return relative.format(-Math.floor(days / 365), "year");
}

/**
 * What a mark on `el` says: what happened there, and since when.
 *
 * A reader who has never seen the marks has no way to tell a line in the
 * margin from decoration; this is what the mark is for, in words.
 */
export function tipFor(el: HTMLElement, readAt: number | null, now: number): string {
  const when = readAt === null ? "" : `, ${since(readAt, now)}`;
  const lines: string[] = [];
  const change = el.getAttribute(CHANGE);
  if (change === "added") lines.push(`Added since you last read this${when}`);
  if (change === "modified") lines.push(`Changed since you last read this${when}`);
  const removed = el.getAttribute(REMOVED);
  if (removed === "after") lines.push(`Text was taken out below since you last read this${when}`);
  if (removed === "before") lines.push(`Text was taken out above since you last read this${when}`);
  return lines.join("\n");
}

/** The tip, held here: a document can name an element `change-tip` too. */
let tipElement: HTMLElement | null = null;

function tip(): HTMLElement {
  if (!tipElement || !tipElement.isConnected) {
    tipElement = document.createElement("div");
    tipElement.className = "change-tip";
    tipElement.setAttribute("role", "tooltip");
    tipElement.setAttribute(TIP, "");
    tipElement.hidden = true;
    document.body.append(tipElement);
  }
  return tipElement;
}

function hideTip(): void {
  if (tipElement && !tipElement.hidden) tipElement.hidden = true;
}

/**
 * Say what a mark means while the pointer is on it.
 *
 * Only over a mark itself: over the text of the block it marks, nothing is
 * said.
 */
function onPointerMove(event: MouseEvent): void {
  const marked = event.target instanceof Element ? hostOf.get(event.target) : undefined;
  if (!marked) {
    hideTip();
    return;
  }
  const found = tip();
  found.textContent = tipFor(marked, readAt, Date.now());
  found.hidden = false;
  // Below and to the right of the pointer, unless that runs off the window.
  const size = found.getBoundingClientRect();
  const below = event.clientY + TIP_GAP;
  const top =
    below + size.height > window.innerHeight ? event.clientY - TIP_GAP - size.height : below;
  const left = Math.min(event.clientX + TIP_GAP, window.innerWidth - size.width - TIP_GAP);
  found.style.left = `${Math.max(0, left)}px`;
  found.style.top = `${Math.max(0, top)}px`;
}

/** How far the tip keeps from the pointer, and from the window's edge. */
const TIP_GAP = 12;

let ready = false;

/**
 * Start saying what the marks mean (see [`onPointerMove`]), and leave the
 * layer they are drawn in, and the tip, to them: the render coordinator would
 * otherwise take each mark drawn, or each word of the tip, for new content and
 * render the page again.
 */
export function setup(): void {
  if (ready) return;
  ready = true;
  renderCoordinator.ignoreWithin(LAYER_IN_VIEWER);
  renderCoordinator.ignoreWithin(`body > [${TIP}]`);
  renderCoordinator.onRenderComplete(afterRender);
  document.addEventListener("mousemove", onPointerMove, { passive: true });
  document.addEventListener("scroll", hideTip, { capture: true, passive: true });
  // Leaving the window sends no further move to hide it by.
  document.addEventListener("mouseleave", hideTip);
}
