/**
 * Lenses on the page: offering the blocks a lens can look at, and keeping
 * each block's answer beside it.
 *
 * The app runs the configured command (see `crates/arto/src/lenses.rs`). A
 * per-block answer is kept here, behind a mark in the block's margin, and
 * shown on hover; the block itself is not touched beyond the mark. What a
 * lens shows in place of the whole page is the app's to render.
 *
 * Every call carries the token of the run it belongs to. Several runs can
 * mark the page at once; an answer for a run that is no longer open is
 * ignored, and nothing is looked up outside the elements a run tagged, so
 * neither a closed run nor markup the document wrote itself can be
 * mistaken for a block.
 */

import { getCurrentElement } from "./content-cursor";
import { contentZoom, placePopover } from "./popover";
import { readSourceRange } from "./source-range";

export type Scope = "cursor" | "document";
export type BlockKind =
  | "paragraph"
  | "heading"
  | "list-item"
  | "table-cell"
  | "definition-term"
  | "definition";

export interface OfferedBlock {
  id: string;
  kind: BlockKind;
  range: string;
  /** Where the block's own text ends: the start of its first nested block. */
  until?: string;
  /** Whether the lens looks at this block, or it is only context. */
  target: boolean;
}

export interface Offered {
  /** The render the blocks belong to, from the page's `data-render-generation`. */
  generation: number | null;
  blocks: OfferedBlock[];
}

const PENDING = "lens-pending";
const ANNOTATED = "lens-annotated";
const FAILED = "lens-failed";
const MARKER = "lens-marker";
const MARKER_OUTDATED = "lens-marker-outdated";
const OUTDATED = "lens-outdated";
const TRANSLATED = "lens-answered";
/** On an answered block: the position of the document's block it stands for. */
const PAIRED = "data-lens-paired";
const POPOVER = "lens-popover";
const CAPTION = "lens-popover-caption";
const SECTION = "lens-popover-section";
const HEAD = "lens-popover-head";
const BADGE = "lens-popover-badge";
const HANDLE = "lens-original-handle";
const HOVER_DELAY_MS = 400;

const KINDS: Record<string, BlockKind> = {
  P: "paragraph",
  H1: "heading",
  H2: "heading",
  H3: "heading",
  H4: "heading",
  H5: "heading",
  H6: "heading",
  LI: "list-item",
  TD: "table-cell",
  TH: "table-cell",
  DT: "definition-term",
  DD: "definition",
};

/** Where one run stands on a block it tagged. */
type Note =
  | { state: "pending" }
  | { state: "annotated"; html: string; outdated?: boolean }
  | { state: "failed"; reason: string };

/** A lens that marks blocks, open over the page. */
interface MarkRun {
  label: string;
  /** The elements the run tagged, by id. */
  tagged: Map<string, HTMLElement>;
  notes: Map<HTMLElement, Note>;
}

/**
 * The runs marking the page, by token, in the order they were opened.
 * Several can be open at once, and a block marked by more than one carries
 * one mark for all of them.
 */
const runs = new Map<number, MarkRun>();
/** The mark in each marked block's margin. */
const markers = new Map<HTMLElement, HTMLElement>();

/**
 * The element `id` of the run `token`, while it is on the page — or held
 * aside by a page lens whose answer stands in its place, to come back when
 * the page shows the document again. One rebuilt away is gone.
 */
function element(token: number, id: string): HTMLElement | null {
  const el = runs.get(token)?.tagged.get(id);
  if (!el) return null;
  if (el.isConnected) return el;
  const heldAside = pageState?.original.some((block) => block.contains(el)) ?? false;
  return heldAside ? el : null;
}

/** Set `el`'s run `token` note to `note` and redraw its marks. */
function note(token: number, el: HTMLElement, value: Note): void {
  runs.get(token)?.notes.set(el, value);
  redraw(el);
}

/** What the open runs say about `el`, in the order they were opened. */
function notesOn(el: HTMLElement): { label: string; note: Note }[] {
  const found = [];
  for (const run of runs.values()) {
    const value = run.notes.get(el);
    if (value) found.push({ label: run.label, note: value });
  }
  return found;
}

/** Draw `el`'s classes and margin mark from every open run's note on it. */
function redraw(el: HTMLElement): void {
  const notes = notesOn(el).map(({ note }) => note);
  const states = new Set(notes.map((note) => note.state));
  const outdated = notes.some((note) => note.state === "annotated" && note.outdated === true);
  el.classList.toggle(PENDING, states.has("pending"));
  el.classList.toggle(ANNOTATED, states.has("annotated"));
  el.classList.toggle(FAILED, states.has("failed"));
  el.classList.toggle(OUTDATED, outdated);
  if (el.classList.length === 0) el.removeAttribute("class");

  let marker = markers.get(el);
  if (states.has("annotated") && !marker) {
    marker = document.createElement("span");
    marker.className = MARKER;
    marker.setAttribute("aria-hidden", "true");
    el.appendChild(marker);
    markers.set(el, marker);
  } else if (!states.has("annotated") && marker) {
    marker.remove();
    markers.delete(el);
    marker = undefined;
  }
  marker?.classList.toggle(MARKER_OUTDATED, outdated);
  if (!batching) syncProxies();
}

/** The first child of `el` that is a block of its own source. */
function firstNestedBlock(el: HTMLElement): HTMLElement | null {
  for (const child of Array.from(el.children)) {
    if (child instanceof HTMLElement && readSourceRange(child)) return child;
  }
  return null;
}

/**
 * The blocks a lens can look at, tagged for the run `token`.
 *
 * All of them are offered, so the neighbours of a target can be its
 * context; `scope` decides which are targets. `current` is the block the
 * cursor is on, for the `cursor` scope.
 */
export function collect(
  token: number,
  scope: Scope,
  label = "",
  current: Element | null = getCurrentElement(),
): Offered {
  restoreMarks(token);
  const root = document.querySelector<HTMLElement>(".markdown-body");
  if (!root) return { generation: null, blocks: [] };
  const tagged = new Map<string, HTMLElement>();
  runs.set(token, { label, tagged, notes: new Map() });
  const generation = Number(root.dataset.renderGeneration);

  const selector = Object.keys(KINDS).join(", ").toLowerCase();
  // While a page lens's answer takes the page, the document's own blocks
  // are held aside: they are what a lens looks at, and what its notes stay
  // on — shown from the answer's marks — when the answer is taken off.
  const page = pageState && ownsPage(pageState) ? pageState : null;
  const candidates = page
    ? page.original.flatMap((block) => [
        ...(block instanceof HTMLElement && block.matches(selector) ? [block] : []),
        ...Array.from(block.querySelectorAll<HTMLElement>(selector)),
      ])
    : Array.from(root.querySelectorAll<HTMLElement>(selector));
  // The cursor on an answer stands on the document's block it answers for.
  const paired = page ? current?.closest(`[${PAIRED}]`) : null;
  if (page && paired) current = page.original[Number(paired.getAttribute(PAIRED))] ?? current;
  const blocks: OfferedBlock[] = [];
  for (const el of candidates) {
    const range = readSourceRange(el);
    const kind = KINDS[el.tagName];
    if (!range || !kind) continue;
    // A loose item or a definition of several paragraphs holds its text in
    // those paragraphs, which are offered themselves.
    if ((kind === "list-item" || kind === "definition") && el.querySelector(":scope > p")) {
      continue;
    }
    const nested = kind === "list-item" ? firstNestedBlock(el) : null;
    const nestedRange = nested ? readSourceRange(nested) : null;
    const id = `${token}:${blocks.length}`;
    tagged.set(id, el);
    blocks.push({
      id,
      kind,
      range: el.dataset.sourceRange ?? "",
      until: nestedRange ? `${nestedRange.start.line}:${nestedRange.start.column}` : undefined,
      target: scope === "document" || (current !== null && current.contains(el)),
    });
  }
  return { generation: Number.isFinite(generation) ? generation : null, blocks };
}

/** Mark the blocks the run `token` is waiting on. */
export function markPending(token: number, ids: string[]): void {
  for (const id of ids) {
    const el = element(token, id);
    if (el) note(token, el, { state: "pending" });
  }
}

/**
 * Keep `html` as the answer for the block `id`, behind a mark. An empty
 * answer is the lens saying it has nothing to add about the block, which
 * leaves the block unmarked: a lens asked to note only what stands out
 * would otherwise mark every block.
 */
export function annotate(token: number, id: string, html: string, outdated = false): void {
  const el = element(token, id);
  if (!el) return;
  if (html.trim() === "") {
    runs.get(token)?.notes.delete(el);
    redraw(el);
    return;
  }
  note(token, el, { state: "annotated", html, outdated });
}

/**
 * Keep several answers at once — each `[id, html, outdated]` — as a
 * document opens with answers kept from before: one call for all of them
 * rather than one per block.
 */
export function annotateAll(token: number, answers: [string, string, boolean][]): void {
  batching = true;
  try {
    for (const [id, html, outdated] of answers) annotate(token, id, html, outdated);
  } finally {
    batching = false;
  }
  syncProxies();
}

/** Whether a batch of notes is being kept, the marks on answers drawn once after. */
let batching = false;

/**
 * Marks on a page lens's answers for the notes on the blocks they stand
 * in for, by the answer element they are drawn on, with the elements whose
 * notes they show. The document's blocks are held aside while the answer
 * takes their places, and their own marks with them; without these the
 * notes of every lens over the page would vanish while a translation shows.
 */
const proxies = new Map<HTMLElement, { marker: HTMLElement; sources: HTMLElement[] }>();

/** Draw a mark on each answer whose block holds notes, and only there. */
function syncProxies(): void {
  const page = pageState;
  const wanted = new Map<HTMLElement, { sources: HTMLElement[]; outdated: boolean }>();
  if (page && ownsPage(page)) {
    for (const [index, entry] of page.answered) {
      const original = page.original[index];
      const target = entry.elements[0];
      if (!original || !(target instanceof HTMLElement)) continue;
      const sources: HTMLElement[] = [];
      let outdated = false;
      for (const run of runs.values()) {
        for (const [el, value] of run.notes) {
          if (value.state !== "annotated" || !original.contains(el)) continue;
          if (!sources.includes(el)) sources.push(el);
          outdated ||= value.outdated === true;
        }
      }
      if (sources.length > 0) wanted.set(target, { sources, outdated });
    }
  }
  for (const [el, proxy] of Array.from(proxies)) {
    if (wanted.has(el)) continue;
    proxy.marker.remove();
    el.classList.remove(ANNOTATED);
    proxies.delete(el);
  }
  for (const [el, { sources, outdated }] of wanted) {
    let proxy = proxies.get(el);
    if (!proxy) {
      const marker = document.createElement("span");
      marker.className = MARKER;
      marker.setAttribute("aria-hidden", "true");
      el.classList.add(ANNOTATED);
      el.appendChild(marker);
      proxy = { marker, sources };
      proxies.set(el, proxy);
    }
    proxy.sources = sources;
    proxy.marker.classList.toggle(MARKER_OUTDATED, outdated);
  }
}

/** Mark the block `id` as failed, keeping `reason` for the hover. */
export function fail(token: number, id: string, reason: string): void {
  const el = element(token, id);
  if (el) note(token, el, { state: "failed", reason });
}

/**
 * Clear the marks of blocks that will get no answer from the run `token`,
 * which stopped — or from any run, without a token.
 */
export function settle(token?: number): void {
  for (const [key, run] of runs) {
    if (token !== undefined && key !== token) continue;
    for (const [el, value] of run.notes) {
      if (value.state !== "pending") continue;
      run.notes.delete(el);
      redraw(el);
    }
  }
}

// ----------------------------------------------------------------------
// Page
// ----------------------------------------------------------------------

/**
 * A page lens in progress: the document's top-level blocks as written, and
 * the answer's, which take their places one by one from the top. Blocks
 * are paired by position — an answer that keeps the document's structure,
 * as a translation does, has its blocks where the document has them.
 */
interface PageState {
  token: number;
  root: HTMLElement;
  original: Element[];
  /**
   * The positions in `original` a whole answer's blocks take, in order;
   * `null` for a block of the answer that has no place of its own.
   */
  places: (number | null)[];
  /**
   * What took the place of the document's block at each position: the
   * elements the answer rendered to, and the HTML they were made from.
   */
  answered: Map<number, { html: string; outdated: boolean; elements: Element[] }>;
  shown: Element[];
}

/** One of the document's top-level blocks, as a page lens sees it. */
export interface PageBlock {
  range: string | null;
  /** Whether it is prose to hand over, rather than code, a diagram or a formula. */
  translatable: boolean;
}

let pageState: PageState | null = null;

/**
 * Whether the page still shows what this lens put there. A re-render of the
 * document replaces the children wholesale, and what the lens kept belongs
 * to the document that is gone.
 */
function ownsPage(page: PageState): boolean {
  const displayed = page.shown;
  return (
    page.root.isConnected && (displayed.length === 0 || displayed[0].parentElement === page.root)
  );
}

/**
 * Make `root`'s children `elements`, touching only the positions that
 * change: a block moved out and back would play its arrival again and have
 * its diagrams and formulas drawn again.
 */
function showBlocks(root: HTMLElement, elements: Element[]): void {
  elements.forEach((element, index) => {
    const current = root.children[index];
    if (current === element) return;
    if (current) {
      root.replaceChild(element, current);
    } else {
      root.appendChild(element);
    }
  });
  while (root.children.length > elements.length) {
    root.lastElementChild?.remove();
  }
}

/** Whether a top-level block holds prose a page lens hands over. */
function isTranslatable(el: Element): boolean {
  if (!el.hasAttribute("data-source-range")) return false;
  if (el.tagName === "PRE" || el.tagName === "HR") return false;
  return !Array.from(el.classList).some((name) => name.startsWith("preprocessed-"));
}

/**
 * Whether a top-level block is the document's raw HTML, which a whole
 * answer has no block for: an answer's raw HTML is rendered escaped, as
 * text between its blocks. Frontmatter and footnotes name no lines either,
 * but an answer renders them as the document does.
 */
function isRawHtml(el: Element): boolean {
  if (el.hasAttribute("data-source-range")) return false;
  return !el.matches("details.frontmatter, section.footnotes");
}

/**
 * The places a whole answer's blocks take among `original`. Markdown the
 * document writes inside its raw HTML is a top-level block of the answer
 * but nested in the document, with nowhere to go: its answer is passed
 * over, and the blocks after it keep their pairing.
 */
function placesOf(original: Element[]): (number | null)[] {
  return original.flatMap((el, index) => {
    if (!isRawHtml(el)) return [index];
    const nested = Array.from(el.querySelectorAll("[data-source-range]")).filter(
      (block) => !el.contains(block.parentElement?.closest("[data-source-range]") ?? null),
    );
    return nested.map(() => null);
  });
}

/**
 * Start showing a page lens over the document: returns the render it is
 * over, how many blocks the answer can take the places of, and what each
 * of the document's top-level blocks is.
 */
export function beginPage(token: number): {
  generation: number | null;
  total: number;
  blocks: PageBlock[];
} {
  restorePage();
  const root = document.querySelector<HTMLElement>(".markdown-body");
  if (!root) return { generation: null, total: 0, blocks: [] };
  const original = Array.from(root.children);
  const places = placesOf(original);
  pageState = {
    token,
    root,
    original,
    places,
    answered: new Map(),
    shown: original,
  };
  const generation = Number(root.dataset.renderGeneration);
  return {
    generation: Number.isFinite(generation) ? generation : null,
    total: places.filter((place) => place !== null).length,
    blocks: original.map((el) => ({
      range: el.getAttribute("data-source-range"),
      translatable: isTranslatable(el),
    })),
  };
}

/**
 * Put the elements `html` renders to in place of the document's block at
 * `index`, unless they are what already stands there. An answer that
 * renders to nothing leaves the document's block standing: taking it away
 * would lose the text rather than show it through the lens.
 */
function answer(page: PageState, index: number, html: string, outdated = false): void {
  const kept = page.answered.get(index);
  if (kept?.html === html && kept.outdated === outdated) return;
  const template = document.createElement("template");
  template.innerHTML = html;
  const elements = Array.from(template.content.children);
  if (elements.length === 0) {
    page.answered.delete(index);
    return;
  }
  // The answer stands where the document's block stood, so it answers for
  // the same lines: scrolling back and copying a path keep working.
  const range = page.original[index]?.getAttribute("data-source-range");
  for (const element of elements) {
    element.classList.add(TRANSLATED);
    element.classList.toggle(OUTDATED, outdated);
    element.setAttribute(PAIRED, String(index));
    if (range) element.setAttribute("data-source-range", range);
  }
  page.answered.set(index, { html, outdated, elements });
}

/** Show the answer so far in the page, or keep it for when it is shown. */
function compose(page: PageState): void {
  const beyond = Array.from(page.answered.entries())
    .filter(([index]) => index >= page.original.length)
    .sort(([a], [b]) => a - b)
    .flatMap(([, answer]) => answer.elements);
  page.shown = [
    ...page.original.flatMap((el, index) => page.answered.get(index)?.elements ?? [el]),
    ...beyond,
  ];
  showBlocks(page.root, page.shown);
  syncProxies();
}

/**
 * Show `html`, the answer so far to the whole document, in place of as many
 * of the document's blocks as it has — its raw HTML aside, which stays as
 * written: returns how many that is.
 */
export function showPage(token: number, html: string, outdated = false): number {
  const page = pageState;
  if (!page || page.token !== token || !ownsPage(page)) return 0;
  const template = document.createElement("template");
  template.innerHTML = html;
  const blocks = Array.from(template.content.children);
  const { original, places } = page;
  let reached = 0;
  blocks.forEach((block, index) => {
    const place = index < places.length ? places[index] : original.length + index - places.length;
    if (place === null) return;
    if (place < original.length) reached += 1;
    answer(page, place, block.outerHTML, outdated);
  });
  compose(page);
  return reached;
}

/**
 * Show `html`, the answer for the document's block at `index` alone, in its
 * place: returns how many blocks have been answered.
 */
export function showBlock(token: number, index: number, html: string, outdated = false): number {
  const page = pageState;
  if (!page || page.token !== token || !ownsPage(page)) return 0;
  answer(page, index, html, outdated);
  compose(page);
  return page.answered.size;
}

/**
 * Show several blocks' answers at once — each `[index, html, outdated]`,
 * by the position of the document's block — composing the page once:
 * returns how many blocks have been answered.
 */
export function showAnswers(token: number, answers: [string, string, boolean][]): number {
  const page = pageState;
  if (!page || page.token !== token || !ownsPage(page)) return 0;
  for (const [index, html, outdated] of answers) answer(page, Number(index), html, outdated);
  compose(page);
  return page.answered.size;
}

/** Take away everything any lens put on the page. */
export function restore(): void {
  restorePage();
  restoreMarks();
}

/** Put the document's blocks back in the places a page lens's answer took. */
export function restorePage(): void {
  hidePopover();
  hideHandle();
  if (pageState && ownsPage(pageState)) {
    showBlocks(pageState.root, pageState.original);
  }
  pageState = null;
  syncProxies();
}

/** Take away the marks and answers the run `token` put beside blocks — or every run's, without a token. */
export function restoreMarks(token?: number): void {
  hidePopover();
  for (const [key, run] of Array.from(runs)) {
    if (token !== undefined && key !== token) continue;
    runs.delete(key);
    for (const el of run.notes.keys()) redraw(el);
  }
}

// ----------------------------------------------------------------------
// Hover
// ----------------------------------------------------------------------

let popover: HTMLElement | null = null;
let hoverTimer: ReturnType<typeof setTimeout> | null = null;
let hovered: HTMLElement | null = null;

function hidePopover(): void {
  if (hoverTimer) clearTimeout(hoverTimer);
  hoverTimer = null;
  hovered = null;
  popover?.classList.remove("is-visible");
}

/** One thing a popover shows, under a caption naming what it is. */
/** A state worth saying beside a lens's name, in the tone it is said in. */
interface Badge {
  text: string;
  tone: "warning" | "danger";
}

interface Section {
  caption: string;
  badge?: Badge;
  content: Node;
}

/** What a popover shows: the block it is about, and what it holds. */
interface HoverContent {
  block: HTMLElement;
  sections: Section[];
}

function section(caption: string, fill: (wrapper: HTMLElement) => void, badge?: Badge): Section {
  const wrapper = document.createElement("div");
  wrapper.className = "markdown-body";
  fill(wrapper);
  return { caption, badge, content: wrapper };
}

/** The block whose answers, original or failures hovering `target` shows, and what. */
function hoverContent(target: Element, withOriginal: boolean): HoverContent | null {
  const answered = !withOriginal ? null : target.closest<HTMLElement>(`[${PAIRED}]`);
  const original = answered ? pageState?.original[Number(answered.getAttribute(PAIRED))] : null;
  if (answered && original && pageState?.root.contains(answered)) {
    const content = section("Original", (wrapper) => wrapper.appendChild(original.cloneNode(true)));
    return { block: answered, sections: [content] };
  }

  let block: HTMLElement | null = null;
  // The elements whose notes the hover shows: the block's own, or — for a
  // mark on a page lens's answer — those of the block it stands in for.
  let sources: HTMLElement[] = [];
  for (const [el, marker] of markers) {
    if (marker.contains(target)) [block, sources] = [el, [el]];
  }
  for (const [el, proxy] of proxies) {
    if (proxy.marker.contains(target)) [block, sources] = [el, proxy.sources];
  }
  if (!block) {
    for (const run of runs.values()) {
      for (const [el, value] of run.notes) {
        if (value.state === "failed" && el.contains(target)) [block, sources] = [el, [el]];
      }
    }
  }
  if (!block) return null;

  // Every open lens that has something to say about the block, in the
  // order they were opened, so one mark serves them all.
  const sections: Section[] = [];
  for (const { label, note: value } of sources.flatMap(notesOn)) {
    if (value.state === "annotated") {
      const badge: Badge | undefined = value.outdated
        ? { text: "Outdated", tone: "warning" }
        : undefined;
      sections.push(
        section(label || "Answer", (wrapper) => (wrapper.innerHTML = value.html), badge),
      );
    } else if (value.state === "failed") {
      sections.push(
        section(label || "Lens", (wrapper) => (wrapper.textContent = value.reason), {
          text: "Failed",
          tone: "danger",
        }),
      );
    }
  }
  return sections.length > 0 ? { block, sections } : null;
}

function showPopover({ block, sections }: HoverContent): void {
  if (!popover?.isConnected) {
    popover = document.createElement("div");
    popover.className = POPOVER;
    document.body.appendChild(popover);
  }
  // Each part named, so that it reads as something laid over the page
  // rather than more of it, and says which lens it came from.
  popover.replaceChildren(
    ...sections.map(({ caption, badge, content }) => {
      // Each lens's answer is a part of its own — a head naming the lens,
      // with what is wrong with the answer beside it, then the answer — so
      // where one lens ends and the next begins is plain.
      const part = document.createElement("section");
      part.className = SECTION;
      const head = document.createElement("div");
      head.className = HEAD;
      const name = document.createElement("span");
      name.className = CAPTION;
      name.textContent = caption;
      head.append(name);
      if (badge) {
        const mark = document.createElement("span");
        mark.className = `${BADGE} is-${badge.tone}`;
        mark.textContent = badge.text;
        head.append(mark);
      }
      part.append(head, content);
      return part;
    }),
  );
  popover.classList.add("is-visible");
  placePopover(popover, block);
}

let hoverInitialized = false;
/** What the pointer last rested on, for Option pressed without moving it. */
let pointed: Element | null = null;

/**
 * The way to a translated block's original that can be seen: a mark in the
 * margin beside the answered block under the pointer, which shows the
 * original when it is pointed at. One mark, moved to the block, rather
 * than one on every block, so a page being read is not strewn with them.
 */
let handle: HTMLElement | null = null;
/** The answered block the handle is beside. */
let handleBlock: HTMLElement | null = null;
/** Pointing at the handle is asking, so it answers sooner than a rest. */
const HANDLE_DELAY_MS = 100;
/** Tabler's "file-text": the document as written, whatever the lens made of it. */
const HANDLE_ICON =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 3v4a1 1 0 0 0 1 1h4"/><path d="M17 21h-10a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2h7l5 5v11a2 2 0 0 1 -2 2"/><path d="M9 9l1 0"/><path d="M9 13l6 0"/><path d="M9 17l6 0"/></svg>';

/** The answered block `target` is in, while the page shows the answer. */
function answeredBlock(target: Element): HTMLElement | null {
  const page = pageState;
  if (!page || !ownsPage(page)) return null;
  const block = target.closest<HTMLElement>(`[${PAIRED}]`);
  return block && page.root.contains(block) ? block : null;
}

function hideHandle(): void {
  handleBlock = null;
  handle?.classList.remove("is-visible");
}

/** Put the handle beside `block`, or take it away without one. */
function placeHandle(block: HTMLElement | null): void {
  if (!block) {
    hideHandle();
    return;
  }
  if (!handle?.isConnected) {
    handle = document.createElement("div");
    handle.className = HANDLE;
    handle.title = "Original — or hold Option (⌥) over the block";
    handle.innerHTML = HANDLE_ICON;
    document.body.appendChild(handle);
  }
  handleBlock = block;
  // At the document's zoom, like the marks beside it (see `placePopover`).
  const zoom = contentZoom(block);
  handle.style.zoom = String(zoom);
  const rect = block.getBoundingClientRect();
  // In the right margin: the left one holds the marks of the lenses over
  // the page, and a handle beside them either covered them or stood off
  // with a gap the pointer fell into on its way. Its padding reaches back
  // over the block's edge, so there is no gap on this side either.
  const overlap = 8 * zoom;
  const left = Math.min(rect.right - overlap, window.innerWidth - 30 * zoom);
  handle.style.left = `${left / zoom}px`;
  handle.style.top = `${(rect.top + 2 * zoom) / zoom}px`;
  handle.classList.add("is-visible");
}

/** Show what hovering `target` shows after `delay`, or hide what no longer applies. */
function consider(target: Element, withOriginal: boolean, delay: number): void {
  const found = hoverContent(target, withOriginal);
  if (!found) {
    hidePopover();
    return;
  }
  if (found.block === hovered) return;
  hidePopover();
  hovered = found.block;
  hoverTimer = setTimeout(() => showPopover(found), delay);
}

/**
 * Show an answer or a failure after a short rest on its mark or block, and
 * a translated block's original while Option is held over it.
 *
 * The original waits for Option because the pointer rests on a translated
 * page all the time while reading it: shown on every rest, it covers the
 * text the reader came for. A mark, by contrast, is only pointed at to ask.
 */
export function setup(): void {
  if (hoverInitialized) return;
  hoverInitialized = true;
  document.addEventListener("mouseover", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (popover?.contains(target)) return;
    if (handle?.contains(target)) {
      if (handleBlock) consider(handleBlock, true, HANDLE_DELAY_MS);
      return;
    }
    pointed = target;
    placeHandle(answeredBlock(target));
    consider(target, event.altKey, HOVER_DELAY_MS);
  });
  const onAlt = (event: KeyboardEvent) => {
    if (event.key !== "Alt" || !pointed?.isConnected) return;
    consider(pointed, event.altKey, 0);
  };
  document.addEventListener("keydown", onAlt);
  document.addEventListener("keyup", onAlt);
  document.addEventListener(
    "scroll",
    () => {
      hidePopover();
      hideHandle();
    },
    true,
  );
}
