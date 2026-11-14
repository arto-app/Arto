/**
 * Pictures of what the document draws, for `arto/capture`.
 *
 * Drawn from the document with html2canvas rather than taken from the
 * screen, so a block out of view is captured where it stands and the reader
 * is not moved to it (docs/adr/20261114-draw-a-capture-from-the-page-rather-than-the-screen.md).
 */

import { html2canvasLibrary } from "./libraries";
import { scrollContainer } from "./scroll-destination";
import { readSourceRange } from "./source-range";
import * as viewportQueue from "./viewport-queue";

/** One picture, as `crates/arto-lsp`'s `CaptureResult` carries it. */
export interface Capture {
  /** Base64 of a PNG, without the `data:` prefix. */
  png: string;
  width: number;
  height: number;
  /** The source lines of the block; absent for the viewport. */
  lines?: { start: number; end: number };
}

/**
 * The largest side a canvas is given. Browsers refuse larger ones by drawing
 * nothing, which would come back as an empty picture rather than an error.
 */
const MAX_SIDE = 16384;

/** Pixels per CSS pixel: the screen's, but never more than two. */
const MAX_SCALE = 2;

function topLevelBlocks(): HTMLElement[] {
  const body = document.querySelector(".markdown-body");
  return body ? Array.from(body.querySelectorAll<HTMLElement>(":scope > [data-source-range]")) : [];
}

/**
 * The top-level block `line` belongs to: the one whose range holds it, or
 * else the last one that starts before it — a blank line between two blocks
 * belongs to the one above, which is the block a reader scrolled to it sees.
 */
export function blockForLine(line: number): HTMLElement | null {
  let before: HTMLElement | null = null;
  for (const block of topLevelBlocks()) {
    const range = readSourceRange(block);
    if (!range) {
      continue;
    }
    if (range.start.line > line) {
      break;
    }
    if (line <= range.end.line) {
      return block;
    }
    before = block;
  }
  return before;
}

/** The top-level block holding the heading with this `id`. */
export function blockForHeading(id: string): HTMLElement | null {
  const heading = document.getElementById(id);
  return topLevelBlocks().find((block) => block === heading || block.contains(heading)) ?? null;
}

/** A picture of the block `line` belongs to, or `null` when there is none. */
export async function captureLine(line: number): Promise<Capture | null> {
  const block = blockForLine(line);
  return block ? captureBlock(block) : null;
}

/** A picture of the heading's block, or `null` when no heading has the id. */
export async function captureHeading(id: string): Promise<Capture | null> {
  const block = blockForHeading(id);
  return block ? captureBlock(block) : null;
}

async function captureBlock(block: HTMLElement): Promise<Capture | null> {
  // A diagram or formula still waiting for the reader to come near it would
  // otherwise be pictured as the placeholder it is until then.
  await viewportQueue.drawWithin(block);
  const range = readSourceRange(block);
  const picture = await draw(block, { unclip: true });
  if (!picture) {
    return null;
  }
  return range ? { ...picture, lines: { start: range.start.line, end: range.end.line } } : picture;
}

/** A picture of what the reader sees of the document now. */
export async function captureViewport(): Promise<Capture | null> {
  const scroller = scrollContainer();
  if (!scroller) {
    return null;
  }
  await viewportQueue.idle();
  return draw(scroller, { unclip: false });
}

/** The pixels per CSS pixel that keep a picture of this size drawable. */
export function scaleFor(width: number, height: number, deviceScale: number): number {
  const wanted = Math.min(Math.max(deviceScale, 1), MAX_SCALE);
  const largest = Math.max(width, height, 1);
  return Math.min(wanted, MAX_SIDE / largest);
}

async function draw(
  element: HTMLElement,
  { unclip }: { unclip: boolean },
): Promise<Capture | null> {
  const html2canvas = html2canvasLibrary();
  if (!html2canvas) {
    return null;
  }
  // html2canvas does not wait for a font a cloned node asks for, so one
  // still loading would be drawn in its fallback.
  await document.fonts.ready;

  const rect = element.getBoundingClientRect();
  const scroller = scrollContainer();
  const canvas = await html2canvas(element, {
    backgroundColor: getComputedStyle(document.body).backgroundColor || null,
    scale: scaleFor(rect.width, rect.height, window.devicePixelRatio || 1),
    logging: false,
    useCORS: true,
    onclone: (clonedDocument) => {
      for (const button of clonedDocument.querySelectorAll<HTMLElement>(".copy-button")) {
        button.style.display = "none";
      }
      // The clone keeps the reader's scroll, so a block scrolled out of view
      // would be cut away by the scroller it sits in. Letting the scroller
      // grow to its content puts every block inside it.
      if (unclip && scroller?.classList.contains("content")) {
        const clonedScroller = clonedDocument.querySelector<HTMLElement>(".content");
        if (clonedScroller) {
          clonedScroller.style.overflow = "visible";
          clonedScroller.style.height = "auto";
          clonedScroller.style.maxHeight = "none";
        }
      }
    },
  });

  const url = canvas.toDataURL("image/png");
  const comma = url.indexOf(",");
  if (comma < 0 || canvas.width === 0 || canvas.height === 0) {
    return null;
  }
  return { png: url.slice(comma + 1), width: canvas.width, height: canvas.height };
}
