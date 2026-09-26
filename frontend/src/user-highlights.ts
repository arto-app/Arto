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
 *
 * A highlight can carry a note. Its last piece wears a glyph for it (drawn
 * by CSS, so the words stay the words), the note shows while the pointer is
 * on the highlight, and a click on a highlight — one that does not end a
 * selection — asks the app to open it, which is where the note is written.
 */

import { refreshReadingPosition } from "./reading-position";
import { renderCoordinator } from "./render-coordinator";
import { toElement } from "./scroll-controller";
import { type TextAnchor, buildIndex, describe, resolve } from "./text-anchor";

export type HighlightColor = "green" | "blue" | "pink" | "orange" | "purple";

/** A highlight as the app sends it. */
export interface HighlightDef {
  id: string;
  color: HighlightColor;
  anchor: TextAnchor;
  /** What the reader wrote about the words, if anything. */
  note?: string;
}

/** A box on the screen, in the window's coordinates. */
export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** A highlight the reader asked to open, and where it is drawn. */
export interface Opened {
  /** The document the page drew the highlight for. */
  doc: string;
  id: string;
  rect: Rect;
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
let openCallback: ((opened: Opened) => void) | null = null;

function body(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".markdown-body");
}

/**
 * Hear what the page found each time highlights are drawn, and which
 * highlight the reader clicked to open. The last report is sent at once, so
 * one made before the app was listening is not lost.
 */
export function setup(
  cb: (report: Report) => void,
  onOpen: ((opened: Opened) => void) | null = null,
): void {
  callback = cb;
  openCallback = onOpen;
  listen();
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
  hideTip();
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
        cut,
      );
      if (!rest) break;
      piece = rest;
      pieceStart = cut;
    }
  });
  refreshReadingPosition();
}

/**
 * Wrap `piece`, which ends at `pieceEnd` in the page's text, in a mark for
 * each of `covering`, the first outermost. The piece a highlight with a note
 * ends on is marked for the note's glyph.
 */
function wrap(piece: Text, covering: Drawn[], pieceEnd: number): void {
  const parent = piece.parentElement;
  if (covering.length === 0 || !parent) return;
  if (piece.data.trim() === "" && BLOCK_PARENTS.has(parent.tagName)) return;
  let inner: Node = piece;
  for (const { def, end } of [...covering].reverse()) {
    const mark = document.createElement("mark");
    mark.className = CLASS;
    mark.dataset.color = def.color;
    mark.dataset.highlightId = def.id;
    if (def.note && end === pieceEnd) mark.dataset.note = "";
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
  /** The innermost highlight the whole selection lies in, if any. */
  within: string | null;
  /** Where that highlight is drawn, or failing one, the selection. */
  rect: Rect | null;
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
  if (!anchor || !range) return null;
  const within = highlightAround(anchor.start, anchor.start + anchor.exact.length);
  const rect = within ? rectOf(within) : toRect(range.getBoundingClientRect());
  return { doc: state.doc, anchor, within, rect };
}

/** The innermost highlight drawn over all of `start`..`end`, if any. */
function highlightAround(start: number, end: number): string | null {
  const around = state.found.filter(
    (found) => found.start <= start && end <= found.end && rectOf(found.def.id) !== null,
  );
  around.sort((a, b) => a.end - a.start - (b.end - b.start));
  return around[0]?.def.id ?? null;
}

function toRect({ left, top, right, bottom }: DOMRect): Rect {
  return { left, top, right, bottom };
}

/**
 * The box around the pieces of the highlight `id` on the page, if any: those
 * in the window when some are. A highlight running past the window would
 * otherwise place its card beyond the window's edge, away from the words the
 * reader is looking at.
 */
export function rectOf(id: string): Rect | null {
  const all = (state.marks.get(id) ?? [])
    .filter((mark) => mark.isConnected)
    .map((mark) => mark.getBoundingClientRect());
  if (all.length === 0) return null;
  const inWindow = all.filter((box) => box.bottom > 0 && box.top < window.innerHeight);
  const boxes = inWindow.length > 0 ? inWindow : all;
  return {
    left: Math.min(...boxes.map((box) => box.left)),
    top: Math.min(...boxes.map((box) => box.top)),
    right: Math.max(...boxes.map((box) => box.right)),
    bottom: Math.max(...boxes.map((box) => box.bottom)),
  };
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

/** How often [`reveal`] looks at where the highlight is while it travels. */
const REVEAL_POLL_MS = 50;
/** How many looks in a row have to find it in one place for it to have landed. */
const REVEAL_STILL_POLLS = 3;
/** How long a journey [`reveal`] waits out before saying where it is anyway. */
const REVEAL_LIMIT_MS = 1500;

/**
 * [`scrollTo`] the highlight `id`, and say where it is once it has stopped
 * moving: something placed beside it before then would be left behind by
 * the scroll.
 */
export async function reveal(id: string): Promise<Rect | null> {
  if (!scrollTo(id)) return null;
  let last = rectOf(id);
  let still = 0;
  for (let waited = 0; last && still < REVEAL_STILL_POLLS && waited < REVEAL_LIMIT_MS;) {
    await new Promise((resolve) => setTimeout(resolve, REVEAL_POLL_MS));
    waited += REVEAL_POLL_MS;
    const now = rectOf(id);
    still = now && sameRect(now, last) ? still + 1 : 0;
    last = now;
  }
  return last;
}

function sameRect(a: Rect, b: Rect): boolean {
  return a.left === b.left && a.top === b.top && a.right === b.right && a.bottom === b.bottom;
}

/** The innermost highlight under `target`, if any. */
export function idAt(target: Element | null): string | null {
  return ancestorMarks(target)[0]?.dataset.highlightId ?? null;
}

function noteOf(id: string | undefined): string | undefined {
  return state.found.find(({ def }) => def.id === id)?.def.note;
}

/** What the render coordinator knows the note tip by. */
const TIP = "data-arto-highlight-tip";
/** How far the tip keeps from the pointer, and from the window's edge. */
const TIP_GAP = 12;

/** The tip, held here: a document can name an element `user-highlight-tip` too. */
let tipElement: HTMLElement | null = null;

function tip(): HTMLElement {
  if (!tipElement || !tipElement.isConnected) {
    tipElement = document.createElement("div");
    tipElement.className = "user-highlight-tip";
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

/** Show the note of the innermost highlight with one under the pointer. */
function onPointerMove(event: MouseEvent): void {
  const target = event.target instanceof Element ? event.target : null;
  const note = ancestorMarks(target)
    .map((mark) => noteOf(mark.dataset.highlightId))
    .find((note) => note !== undefined);
  if (note === undefined) {
    hideTip();
    return;
  }
  const found = tip();
  if (found.textContent !== note) found.textContent = note;
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

/**
 * Ask the app to open the highlight clicked on.
 *
 * Not for a click that ends a drag, which is the reader selecting words, nor
 * for one on a link inside a highlight, which is the reader following it.
 */
function onClick(event: MouseEvent): void {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
    return;
  }
  const target = event.target instanceof Element ? event.target : null;
  if (!target || target.closest("a, .md-link")) return;
  const selection = window.getSelection();
  if (selection && !selection.isCollapsed) return;
  const id = idAt(target);
  const rect = id ? rectOf(id) : null;
  if (id && rect && state.doc !== null) openCallback?.({ doc: state.doc, id, rect });
}

let listening = false;

/**
 * Start showing notes and hearing clicks, and leave the tip to them: the
 * render coordinator would otherwise take each note shown for new content
 * and render the page again.
 */
function listen(): void {
  if (listening) return;
  listening = true;
  renderCoordinator.ignoreWithin(`body > [${TIP}]`);
  document.addEventListener("mousemove", onPointerMove, { passive: true });
  document.addEventListener("scroll", hideTip, { capture: true, passive: true });
  // Leaving the window sends no further move to hide it by.
  document.addEventListener("mouseleave", hideTip);
  document.addEventListener("click", onClick);
}

/** @internal */
export function _reset(): void {
  state.doc = null;
  state.found = [];
  state.marks.clear();
  state.lastReport = null;
  callback = null;
  openCallback = null;
  hideTip();
}
