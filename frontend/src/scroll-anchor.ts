/**
 * Where the reader is in a document, as something that survives the document
 * changing height under them.
 *
 * A pixel offset only means anything while every block is the height it was
 * when the offset was taken. That was already shaky — diagrams and formulas
 * are drawn after the document appears, and a file can be edited while it is
 * open — and a document full of diagrams is still growing under the reader
 * for as long as they keep scrolling into undrawn ones.
 *
 * An anchor names a block instead: the `data-source-line` of the block at the
 * top of the view, and how far into that block the top edge sits. Restoring
 * looks the block up and measures it as it is now, so a height that has
 * changed since costs nothing.
 *
 * The Rust counterpart is `crates/arto/src/scroll_anchor.rs`, which carries
 * these through the history entries, the tab state and the navigation events.
 */

import { scrollContainer, scrollerTop, settleAt } from "./scroll-destination";

/** Where the reader is, as `crates/arto/src/scroll_anchor.rs` spells it. */
export interface ScrollAnchor {
  /** 1-based source line of the block at the top; 0 is the top of the document. */
  line: number;
  /** How far into that block the top edge sits, 0 to just under 1. */
  fraction: number;
}

/** The top of the document. */
const TOP: ScrollAnchor = { line: 0, fraction: 0 };

let cachedBody: Element | null = null;
let cachedBlocks: HTMLElement[] = [];

/**
 * Forget the cached block list.
 *
 * The document is replaced in place — the `.markdown-body` element survives,
 * only its children change — so element identity cannot detect the swap. The
 * batch render, which is the one moment the document is known to have
 * changed, says so here.
 */
export function invalidateBlocks(): void {
  cachedBody = null;
  cachedBlocks = [];
}

/**
 * The blocks that can be anchored to, in document order.
 *
 * Cached: [`currentAnchor`] runs on every scroll event, and collecting every
 * top-level block of a megabyte document at 120 Hz costs more than the layout
 * this file exists to keep cheap.
 */
function blocks(): HTMLElement[] {
  const body = document.querySelector(".markdown-body");
  if (!body) {
    invalidateBlocks();
    return [];
  }
  // The `isConnected` check covers the window between the document being
  // replaced and the batch render that announces it: measuring detached
  // blocks would name a line from the document that just went away. An empty
  // list is never cached — the body exists before its content does, and there
  // is no first block to ask whether it is still connected.
  if (body !== cachedBody || cachedBlocks.length === 0 || !cachedBlocks[0].isConnected) {
    cachedBody = body;
    cachedBlocks = Array.from(body.querySelectorAll<HTMLElement>(":scope > [data-source-line]"));
  }
  return cachedBlocks;
}

/** The line a block reports, or `null` when it is not a number. */
function lineOf(block: HTMLElement): number | null {
  const line = Number(block.dataset.sourceLine);
  return Number.isFinite(line) ? line : null;
}

/**
 * Index of the last block that starts at or above `top`, itself in viewport
 * coordinates.
 *
 * Everything here measures with `getBoundingClientRect` rather than
 * `offsetTop`: the app puts the document inside a `zoom:` wrapper (see
 * `crates/arto/src/components/content.rs`), and `offsetTop` reports a
 * zoomed element's position in its own unzoomed coordinate space while
 * `scrollTop` stays in the scroller's. Mixing the two puts the anchor a
 * factor of the zoom level away from the reader at any zoom but 100%.
 *
 * Binary search rather than a scan: this runs on every scroll event, and the
 * documents that need it are the ones with the most blocks.
 */
function blockAt(list: HTMLElement[], top: number): number {
  let low = 0;
  let high = list.length - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (list[middle].getBoundingClientRect().top <= top) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}

/** Where the reader is now, as a value [`toAnchor`] can put back. */
export function currentAnchor(): ScrollAnchor {
  const scroller = scrollContainer();
  if (!scroller || scroller.scrollTop <= 0) {
    return TOP;
  }
  const list = blocks();
  if (list.length === 0) {
    return TOP;
  }

  const top = scrollerTop(scroller);
  const block = list[blockAt(list, top)];
  const line = lineOf(block);
  if (line === null) {
    return TOP;
  }

  const rect = block.getBoundingClientRect();
  const fraction = Math.min(Math.max((top - rect.top) / (rect.height || 1), 0), 0.999);
  return { line, fraction };
}

/**
 * Put the reader back where `anchor` says they were.
 *
 * Null is accepted because the app is the caller and it has a failure path
 * that sends one (`crates/arto/src/components/content/file_viewer.rs`).
 * Reading `line` off it would throw inside the MutationObserver that asked,
 * which loses the second phase of the restore along with the first.
 */
export function toAnchor(anchor: ScrollAnchor | null): void {
  if (!anchor) {
    return;
  }
  // Applying it once is not enough: the blocks around the destination lay out
  // and draw *because* the view arrives at them, and that moves the very
  // position being restored.
  settleAt(() => applyAnchor(anchor));
}

function applyAnchor(anchor: ScrollAnchor): void {
  const scroller = scrollContainer();
  if (!scroller) {
    return;
  }
  if (anchor.line <= 0) {
    scroller.scrollTo(0, 0);
    return;
  }

  const list = blocks();
  if (list.length === 0) {
    return;
  }

  // The document may have been edited since, so settle for the last block
  // that starts at or before the line rather than requiring an exact match.
  let block = list[0];
  for (const candidate of list) {
    const candidateLine = lineOf(candidate);
    if (candidateLine === null || candidateLine > anchor.line) {
      break;
    }
    block = candidate;
  }

  const rect = block.getBoundingClientRect();
  const target =
    scroller.scrollTop + (rect.top - scrollerTop(scroller)) + anchor.fraction * rect.height;
  scroller.scrollTo(0, Math.max(target, 0));
}
