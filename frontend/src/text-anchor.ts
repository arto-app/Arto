/**
 * Naming a place in the page by its words, and finding it again.
 *
 * A highlight is kept across edits of the document, so it cannot be kept as
 * a DOM path or a source column: both move with every edit above it, and a
 * selection across a link or an emphasis has no reliable way back to source
 * columns anyway. It is kept the way W3C Web Annotation's text quote and
 * text position selectors keep one — the words, a little of what surrounds
 * them, and where they were — plus the source line of the block they start
 * in, which is what the page itself says about where a block came from.
 *
 * The page's text is its text nodes read in order, minus what is not prose:
 * code blocks, diagrams, maths (TeX source until it is drawn, and redrawn
 * on the way), and what a lens puts in place of the document. Offsets count
 * what a JavaScript string counts.
 */

import { readSourceRange } from "./source-range";

/** Where a highlight is. Mirrors `TextAnchor` in `crates/arto/src/highlights.rs`. */
export interface TextAnchor {
  exact: string;
  prefix: string;
  suffix: string;
  start: number;
  line: number;
}

/** Where a quote was found: its span in the page's text, and the block's line. */
export interface Found {
  start: number;
  end: number;
  line: number;
}

/** The page's text, and the text nodes it was read from. */
export interface TextIndex {
  root: HTMLElement;
  nodes: Text[];
  /** Where each node's text starts in `text`. */
  starts: number[];
  text: string;
}

/** What is not prose, and so neither highlighted nor counted. */
export const EXCLUDED =
  "pre, .mermaid, .preprocessed-mermaid, .preprocessed-math-inline," +
  " .preprocessed-math-display, .lens-answered, .lens-popover";

/** How much of what surrounds a quote is kept on each side. */
const CONTEXT = 32;

/**
 * How much of the surroundings a quote found more than once has to match
 * for the match to be taken as the same place. A space on either side
 * matches almost anywhere; a few characters do not.
 */
const MIN_CONTEXT = 4;

/** Read the page's text under `root`. */
export function buildIndex(root: HTMLElement): TextIndex {
  const nodes: Text[] = [];
  const starts: number[] = [];
  let text = "";
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) =>
      node.parentElement?.closest(EXCLUDED) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT,
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const textNode = node as Text;
    nodes.push(textNode);
    starts.push(text.length);
    text += textNode.data;
  }
  return { root, nodes, starts, text };
}

/**
 * The offset in the page's text of a boundary point.
 *
 * A point inside a text node the page reads is where it is; any other point
 * — on an element, or in text that is not counted — is where the next
 * counted text starts.
 */
function pointOffset(index: TextIndex, container: Node, offset: number): number {
  if (container instanceof Text) {
    const at = index.nodes.indexOf(container);
    if (at >= 0) return index.starts[at] + Math.min(offset, container.data.length);
  }
  const point = document.createRange();
  point.setStart(container, offset);
  point.collapse(true);
  for (let i = 0; i < index.nodes.length; i++) {
    const node = index.nodes[i];
    if (point.comparePoint(node, node.data.length) > 0) {
      return index.starts[i];
    }
  }
  return index.text.length;
}

/** The text node holding the character at `offset`. */
function nodeAt(index: TextIndex, offset: number): Text | null {
  const { nodes, starts } = index;
  let lo = 0;
  let hi = nodes.length - 1;
  let at = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (starts[mid] <= offset) {
      at = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  // An empty node starts where the next one does; the one holding the
  // character is the last of them with any text.
  for (; at >= 0 && starts[at] <= offset; at--) {
    if (offset < starts[at] + nodes[at].data.length) return nodes[at];
  }
  return null;
}

/** The first source line of the block the text at `offset` is in, or 0. */
export function lineAt(index: TextIndex, offset: number): number {
  let el =
    nodeAt(index, offset)?.parentElement?.closest<HTMLElement>("[data-source-range]") ?? null;
  while (el && index.root.contains(el)) {
    const range = readSourceRange(el);
    if (range) return range.start.line;
    el = el.parentElement?.closest<HTMLElement>("[data-source-range]") ?? null;
  }
  return 0;
}

/** Name the place `range` covers, or `null` when it covers no counted text. */
export function describe(index: TextIndex, range: Range): TextAnchor | null {
  let start = pointOffset(index, range.startContainer, range.startOffset);
  let end = pointOffset(index, range.endContainer, range.endOffset);
  const { text } = index;
  while (start < end && /\s/.test(text[start])) start++;
  while (end > start && /\s/.test(text[end - 1])) end--;
  if (start >= end) return null;
  return {
    exact: text.slice(start, end),
    prefix: text.slice(wholeFrom(text, Math.max(0, start - CONTEXT)), start),
    suffix: text.slice(end, wholeTo(text, end + CONTEXT)),
    start,
    line: lineAt(index, start),
  };
}

/**
 * `at`, or the character after it when it would split one: a character
 * outside the Basic Multilingual Plane — an emoji — is two code units, and
 * half of one cannot be sent to the app.
 */
function wholeFrom(text: string, at: number): number {
  return isLowSurrogate(text.charCodeAt(at)) ? at + 1 : at;
}

/** The same for the end of a slice. */
function wholeTo(text: string, at: number): number {
  return at < text.length && isLowSurrogate(text.charCodeAt(at)) ? at - 1 : at;
}

function isLowSurrogate(code: number): boolean {
  return code >= 0xdc00 && code <= 0xdfff;
}

/** How many characters `a` and `b` share at their ends. */
function sharedEnd(a: string, b: string): number {
  let n = 0;
  while (n < a.length && n < b.length && a[a.length - 1 - n] === b[b.length - 1 - n]) n++;
  return n;
}

/** How many characters `a` and `b` share at their starts. */
function sharedStart(a: string, b: string): number {
  let n = 0;
  while (n < a.length && n < b.length && a[n] === b[n]) n++;
  return n;
}

/**
 * Find `anchor` in the page again, or `null` when it is gone.
 *
 * Every place the words are is a candidate. The one whose surroundings
 * match best wins; a tie goes to the one in the block the anchor was last
 * seen on, then to the one nearest where it was. A candidate is only taken
 * when enough of its surroundings match, or it is the only place the words
 * are, or the only one on the anchor's line: a common word whose context is gone
 * could be any of its occurrences, and highlighting the wrong one is worse
 * than saying it was lost.
 */
export function resolve(index: TextIndex, anchor: TextAnchor): Found | null {
  const { text } = index;
  const { exact, prefix, suffix } = anchor;
  if (exact.length === 0) return null;

  type Candidate = { start: number; score: number };
  const candidates: Candidate[] = [];
  for (let at = text.indexOf(exact); at >= 0; at = text.indexOf(exact, at + 1)) {
    const before = text.slice(Math.max(0, at - prefix.length), at);
    const after = text.slice(at + exact.length, at + exact.length + suffix.length);
    candidates.push({ start: at, score: sharedEnd(before, prefix) + sharedStart(after, suffix) });
  }
  if (candidates.length === 0) return null;

  // Finding a block's line walks the DOM, so it is asked only of the
  // candidates that need it — a one-letter quote can be everywhere.
  const lines = new Map<number, number>();
  const lineOf = (c: Candidate): number => {
    let line = lines.get(c.start);
    if (line === undefined) {
      line = lineAt(index, c.start);
      lines.set(c.start, line);
    }
    return line;
  };
  const onLine = (c: Candidate) => (lineOf(c) === anchor.line ? 1 : 0);

  const top = candidates.reduce((most, c) => Math.max(most, c.score), 0);
  let best = candidates
    .filter((c) => c.score === top)
    .reduce((a, b) => {
      if (onLine(a) !== onLine(b)) return onLine(a) > onLine(b) ? a : b;
      return Math.abs(b.start - anchor.start) < Math.abs(a.start - anchor.start) ? b : a;
    });

  const required = Math.min(MIN_CONTEXT, prefix.length + suffix.length);
  if (best.score < required && candidates.length > 1) {
    // Nothing around the words says which they are. The block they were in
    // still can, but only when they occur in it once: a line names a block,
    // and a word can occur in a paragraph many times.
    const onAnchorLine = candidates.filter((c) => onLine(c) === 1);
    if (onAnchorLine.length !== 1) return null;
    best = onAnchorLine[0];
  }
  return { start: best.start, end: best.start + exact.length, line: lineOf(best) };
}
