/**
 * Where the reader is in a document, as something that survives the document
 * changing height under them.
 *
 * A pixel offset only means anything while every block is the height it was
 * when the offset was taken. That was already shaky — diagrams and formulas
 * are drawn after the document appears, and a file can be edited while it is
 * open — and it stops being true entirely once blocks are laid out lazily,
 * because then most of the document's height is an estimate.
 *
 * An anchor names a block instead: the `data-source-line` of the block at the
 * top of the view, and how far into that block the top edge sits. Restoring
 * looks the block up and measures it as it is now, so an estimate that has
 * since been replaced by a real height costs nothing.
 *
 * The Rust counterpart is `crates/arto/src/scroll_anchor.rs`, which carries
 * these through the history entries, the tab state and the navigation events.
 */

import { scrollContainer } from "./scroll-controller";

/** Where the reader is, as `crates/arto/src/scroll_anchor.rs` spells it. */
export interface ScrollAnchor {
  /** 1-based source line of the block at the top; 0 is the top of the document. */
  line: number;
  /** How far into that block the top edge sits, 0 to just under 1. */
  fraction: number;
}

/** The top of the document. */
const TOP: ScrollAnchor = { line: 0, fraction: 0 };

/** The blocks that can be anchored to, in document order. */
function blocks(): HTMLElement[] {
  const body = document.querySelector(".markdown-body");
  if (!body) {
    return [];
  }
  return Array.from(body.querySelectorAll<HTMLElement>(":scope > [data-source-line]"));
}

/** The line a block reports, or `null` when it is not a number. */
function lineOf(block: HTMLElement): number | null {
  const line = Number(block.dataset.sourceLine);
  return Number.isFinite(line) ? line : null;
}

/**
 * Index of the last block that starts at or above `offset`.
 *
 * Binary search rather than a scan: this runs on every scroll event, and the
 * documents that need it are the ones with the most blocks.
 */
function blockAt(list: HTMLElement[], offset: number): number {
  let low = 0;
  let high = list.length - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (list[middle].offsetTop <= offset) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}

/** Where the reader is now, as a value [`scrollToAnchor`] can put back. */
export function currentAnchor(): ScrollAnchor {
  const scroller = scrollContainer();
  if (!scroller || scroller.scrollTop <= 0) {
    return TOP;
  }
  const list = blocks();
  if (list.length === 0) {
    return TOP;
  }

  // `offsetTop` is relative to the offset parent rather than the scroller, so
  // the first block's own position is the origin everything else is measured
  // from.
  const origin = list[0].offsetTop;
  const offset = scroller.scrollTop + origin;
  const index = blockAt(list, offset);
  const block = list[index];
  const line = lineOf(block);
  if (line === null) {
    return TOP;
  }

  const height = block.offsetHeight || 1;
  const fraction = Math.min(Math.max((offset - block.offsetTop) / height, 0), 0.999);
  return { line, fraction };
}

/** Put the reader back where `anchor` says they were. */
export function scrollToAnchor(anchor: ScrollAnchor): void {
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

  const origin = list[0].offsetTop;
  const target = block.offsetTop - origin + anchor.fraction * (block.offsetHeight || 0);
  scroller.scrollTo(0, Math.max(target, 0));
}
