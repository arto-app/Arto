/**
 * The reader's highlights, drawn on the page.
 *
 * The app keeps them (see `crates/arto/src/highlights.rs`) and hands this
 * the ones on the document shown. Each is found again by its words (see
 * `text-anchor.ts`) and drawn as `mark.user-highlight` around the text it
 * covers, one mark per text node when it crosses several; what was found
 * where, and what could not be found, goes back to the app, which keeps
 * the new places and lists the lost ones.
 *
 * Search and pinned search mark the page too, and they match within one
 * text node at a time: a highlight splitting a word would hide the word
 * from a search. So these marks are drawn last and lifted whenever the
 * others are redrawn (`find-in-page.ts` brackets its work with [`lift`] and
 * [`draw`]) — the others always see the page's text as written, and a
 * highlight ends up inside a search mark rather than around it.
 */

import { refreshReadingPosition } from "./reading-position";
import { toElement } from "./scroll-controller";
import { type TextAnchor, buildIndex, describe, resolve } from "./text-anchor";

export type HighlightColor = "green" | "blue" | "pink" | "orange" | "purple";

/** A highlight as the app sends it. */
export interface HighlightDef {
  id: string;
  color: HighlightColor;
  anchor: TextAnchor;
}

/** Where a highlight was found. */
export interface Placed {
  id: string;
  start: number;
  line: number;
  /** The id of the heading it is under, or `null` above the first. */
  heading: string | null;
}

/** What drawing a document's highlights found, for the app. */
export interface Report {
  /** The document, as the app named it. */
  doc: string;
  placed: Placed[];
  orphans: string[];
}

const CLASS = "user-highlight";
const FLASH = "user-highlight-flash";

/**
 * Elements a stretch of white space may sit directly in without being part
 * of any sentence — the new line between two list items. Marking that would
 * put a mark where only blocks may go.
 */
const BLOCK_PARENTS = new Set([
  "ARTICLE",
  "BLOCKQUOTE",
  "DETAILS",
  "DIV",
  "DL",
  "OL",
  "SECTION",
  "TABLE",
  "TBODY",
  "TFOOT",
  "THEAD",
  "TR",
  "UL",
]);

interface Drawn {
  def: HighlightDef;
  start: number;
  end: number;
  line: number;
}

interface State {
  /** The document the highlights drawn belong to, as the app named it. */
  doc: string | null;
  /** Where each highlight found was found, in the page's text. */
  found: Drawn[];
  marks: Map<string, HTMLElement[]>;
  lastReport: Report | null;
}

const state: State = { doc: null, found: [], marks: new Map(), lastReport: null };

let callback: ((report: Report) => void) | null = null;

function body(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".markdown-body");
}

/**
 * Hear what the page found each time highlights are drawn. The last report
 * is sent at once, so one made before the app was listening is not lost.
 */
export function setup(cb: (report: Report) => void): void {
  callback = cb;
  if (state.lastReport) cb(state.lastReport);
}

/**
 * Show `highlights` on `doc`, the document the page now holds.
 *
 * With a `generation`, only while the page still holds that render of it:
 * the app can name a document before its page has arrived, and the page it
 * would be drawn on then is the one before.
 */
export function show(
  doc: string,
  highlights: HighlightDef[],
  generation: number | null = null,
): void {
  const root = body();
  if (generation !== null && root?.dataset.renderGeneration !== String(generation)) return;
  state.doc = doc;
  clear();
  state.found = [];
  const orphans: string[] = [];
  if (root) {
    const index = buildIndex(root);
    for (const def of highlights) {
      const found = resolve(index, def.anchor);
      if (found) {
        state.found.push({ def, ...found });
      } else {
        orphans.push(def.id);
      }
    }
  }
  draw();
  const placed: Placed[] = state.found.map(({ def, start, line }) => {
    const first = state.marks.get(def.id)?.find((mark) => mark.isConnected);
    return { id: def.id, start, line, heading: root && first ? headingBefore(root, first) : null };
  });
  const report = { doc, placed, orphans };
  state.lastReport = report;
  callback?.(report);
}

/**
 * Take the highlights off the page, leaving its text as it was, for the
 * other marks to be redrawn under them.
 *
 * Only what is in the page: a page lens holds the document's own blocks
 * aside while it shows its answer and puts them back as they were, and the
 * highlights on them have to come back with them. They stay listed, so
 * that [`clear`] can still reach them.
 */
export function lift(): void {
  const aside = new Map<string, HTMLElement[]>();
  for (const [id, marks] of state.marks) {
    const held = marks.filter((mark) => !mark.isConnected);
    if (held.length > 0) aside.set(id, held);
  }
  unwrapAll(document.querySelectorAll<HTMLElement>(`mark.${CLASS}`));
  state.marks = aside;
}

/** Take every highlight off, including those on blocks held aside. */
function clear(): void {
  unwrapAll(Array.from(state.marks.values()).flat());
  unwrapAll(document.querySelectorAll<HTMLElement>(`mark.${CLASS}`));
  state.marks = new Map();
}

function unwrapAll(marks: Iterable<HTMLElement>): void {
  for (const mark of Array.from(marks)) {
    const parent = mark.parentNode;
    if (!parent) continue;
    while (mark.firstChild) parent.insertBefore(mark.firstChild, mark);
    parent.removeChild(mark);
    parent.normalize();
  }
}

/**
 * Draw the highlights found last time again, after [`lift`].
 *
 * Nothing is looked for again: the page's text is what it was when they
 * were found — the other marks do not change it — so where they were found
 * still holds. A place whose text is not what was found there any more is
 * left undrawn rather than drawn over the wrong words.
 */
export function draw(): void {
  lift();
  const root = body();
  if (!root) return;
  const index = buildIndex(root);
  const drawn = state.found.filter(
    ({ def, start, end }) => index.text.slice(start, end) === def.anchor.exact,
  );
  for (const { def } of drawn) {
    if (!state.marks.has(def.id)) state.marks.set(def.id, []);
  }
  // Every node is cut once, at every edge of every highlight in it, before
  // anything is wrapped: cutting one highlight's node would move the offsets
  // the next one was found at. Where highlights overlap, a piece is wrapped
  // once per highlight, the first outermost.
  index.nodes.forEach((node, i) => {
    const nodeStart = index.starts[i];
    const nodeEnd = nodeStart + node.data.length;
    const over = drawn.filter(({ start, end }) => start < nodeEnd && end > nodeStart);
    if (over.length === 0) return;
    const cuts = Array.from(
      new Set(
        over
          .flatMap(({ start, end }) => [start, end])
          .filter((at) => at > nodeStart && at < nodeEnd),
      ),
    ).sort((a, b) => a - b);
    let piece = node;
    let pieceStart = nodeStart;
    for (const cut of [...cuts, nodeEnd]) {
      const rest = cut < nodeEnd ? piece.splitText(cut - pieceStart) : null;
      wrap(
        piece,
        over.filter(({ start, end }) => start <= pieceStart && end >= cut),
      );
      if (!rest) break;
      piece = rest;
      pieceStart = cut;
    }
  });
  refreshReadingPosition();
}

/** Wrap `piece` in a mark for each of `covering`, the first outermost. */
function wrap(piece: Text, covering: Drawn[]): void {
  const parent = piece.parentElement;
  if (covering.length === 0 || !parent) return;
  if (piece.data.trim() === "" && BLOCK_PARENTS.has(parent.tagName)) return;
  let inner: Node = piece;
  for (const { def } of [...covering].reverse()) {
    const mark = document.createElement("mark");
    mark.className = CLASS;
    mark.dataset.color = def.color;
    mark.dataset.highlightId = def.id;
    inner.parentNode?.insertBefore(mark, inner);
    mark.appendChild(inner);
    state.marks.get(def.id)?.push(mark);
    inner = mark;
  }
}

/** The id of the last heading at or before `el`, or `null`. */
function headingBefore(root: HTMLElement, el: Element): string | null {
  let last: string | null = null;
  for (const heading of root.querySelectorAll<HTMLElement>(
    "h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]",
  )) {
    if (heading === el || heading.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING) {
      last = heading.id;
    } else {
      break;
    }
  }
  return last;
}

/**
 * The selection a highlight is made from: the reader's, or failing that
 * `fallback` — the one the context menu kept when it opened, since picking an
 * item from it can have taken the selection away by then.
 */
function selectionRange(fallback: Range | null): Range | null {
  const root = body();
  if (!root) return null;
  const selection = window.getSelection();
  if (selection && selection.rangeCount > 0 && !selection.isCollapsed) {
    const range = selection.getRangeAt(0);
    if (range.intersectsNode(root)) return range;
  }
  return fallback && !fallback.collapsed && fallback.intersectsNode(root) ? fallback : null;
}

/** A place selected, and the document it was selected in. */
export interface Selected {
  /** The document the page holds, as the app named it when it drew it. */
  doc: string | null;
  anchor: TextAnchor;
}

/**
 * Name the selected place, or `null` when nothing highlightable is selected.
 *
 * With the document it is in: the app may already have moved on to another
 * one that the page has not drawn yet, and a place is only a place in the
 * document it was taken from.
 */
export function describeSelection(fallback: Range | null = null): Selected | null {
  const root = body();
  const range = selectionRange(fallback);
  const anchor = root && range ? describe(buildIndex(root), range) : null;
  return anchor ? { doc: state.doc, anchor } : null;
}

/**
 * The highlights `range` touches, or with no range the ones `target` is in.
 * Each once.
 */
export function idsIn(range: Range | null, target?: Element | null): string[] {
  const marks = range
    ? Array.from(document.querySelectorAll<HTMLElement>(`mark.${CLASS}`)).filter((mark) =>
        range.intersectsNode(mark),
      )
    : ancestorMarks(target ?? null);
  const ids: string[] = [];
  for (const mark of marks) {
    const id = mark.dataset.highlightId;
    if (id && !ids.includes(id)) ids.push(id);
  }
  return ids;
}

/** Every highlight mark `el` is in, innermost first: highlights nest. */
function ancestorMarks(el: Element | null): HTMLElement[] {
  const marks: HTMLElement[] = [];
  for (let mark = el?.closest<HTMLElement>(`mark.${CLASS}`); mark;) {
    marks.push(mark);
    mark = mark.parentElement?.closest<HTMLElement>(`mark.${CLASS}`);
  }
  return marks;
}

/** The highlights the selection touches. */
export function idsAtSelection(): string[] {
  const range = selectionRange(null);
  return range ? idsIn(range) : [];
}

/** Bring the highlight `id` into view and flash it. */
export function scrollTo(id: string): boolean {
  const marks = (state.marks.get(id) ?? []).filter((mark) => mark.isConnected);
  if (marks.length === 0) return false;
  toElement(marks[0], "center");
  for (const mark of marks) {
    mark.classList.remove(FLASH);
    // Restart the animation when the same highlight is picked twice.
    void mark.offsetWidth;
    mark.classList.add(FLASH);
  }
  setTimeout(() => {
    for (const mark of marks) mark.classList.remove(FLASH);
  }, 600);
  return true;
}

/** @internal */
export function _reset(): void {
  state.doc = null;
  state.found = [];
  state.marks.clear();
  state.lastReport = null;
  callback = null;
}
