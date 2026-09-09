/**
 * Where the reader is in the document, for the chrome that answers to it: the
 * contents gutter's current tick, the marks a search left on it, and the
 * margin trace's place beside the page.
 *
 * All of it is the same question asked in different words — where in this
 * document are we, and what is around us — so it shares one passive scroll
 * listener and one frame's worth of work. The scrollbar the reader sees is
 * the same question again, so it is drawn from here too.
 */

import { drawScrollIndicator } from "./scroll-indicator";

/**
 * How much of the column a marginal one has to centre itself against.
 *
 * Both are set level with the middle of what is being read, and what is being
 * read is the document — until the document is taller than the window, when
 * it is the window. So: the smaller of the two.
 */
const MARGIN_EXTENT = "--margin-extent";

/**
 * How far the margin trace has to move to reach the document.
 *
 * The trace is laid out at the left edge of the content area, but the column
 * it annotates is centred in what is left of the window, so at a wide window
 * the two are half a screen apart. This closes that: the names sit in the
 * document's own margin, which is the only place a marginal note means
 * anything.
 */
const TRACE_OFFSET = "--trace-offset";

/**
 * How far the contents ruler has to move to reach the document.
 *
 * The same errand as [`TRACE_OFFSET`], on the other side: the ruler is laid
 * out at the right edge of the reading area and moved left into the page's
 * own margin, so that the two columns stand the same distance from the text
 * they are about.
 */
const GUTTER_OFFSET = "--gutter-offset";

/**
 * How much air is left between a marginal column and the text it is about.
 *
 * Kept here rather than as padding on the column, so that widening the gap
 * moves the whole column instead of eating the room its names are set in.
 */
export const MARGIN_GAP = 56;

/** The attribute that says the page has taken the margin the trace sits in. */
const TRACE_CROWDED = "data-trace-crowded";

/** The same, for the margin the contents ruler sits in. */
const GUTTER_CROWDED = "data-gutter-crowded";

/** The attribute that says the page is keeping the ruler's column back. */
const RULER = "data-ruler";

/**
 * Where a column set in the page's margin goes, and whether there is a margin
 * left to put it in.
 *
 * `room` is the distance between the column where it is laid out — at one
 * edge of the reading area — and the page. The column is moved that far,
 * less the gap it keeps from the text.
 *
 * The layout budget reserves each column's width against the document's
 * *minimum* width, but the page is set to its own width and centred in
 * whatever is left of the window — so magnifying the page, or a wide panel
 * beside it, closes the margin while the budget still allows the column. When
 * the margin can no longer hold it at its distance from the text, the column
 * gives way, as it does to every other claim on the space; the alternative is
 * a column written over the first inch of every line.
 */
export function marginColumn(room: number): { offset: number; crowded: boolean } {
  if (room < MARGIN_GAP) {
    return { offset: 0, crowded: true };
  }
  return { offset: Math.round(room - MARGIN_GAP), crowded: false };
}

/** The attribute that marks the heading the reader is inside. */
const CURRENT = "data-current";

/** The attribute that marks a heading a search has hits under. */
const HIT = "data-hit";

/** The ink a hit is drawn in, by the name the pinned search carries. */
const HIT_COLOURS: Record<string, string> = {
  green: "var(--mark-green)",
  blue: "var(--mark-blue)",
  pink: "var(--mark-pink)",
  orange: "var(--mark-orange)",
  purple: "var(--mark-purple)",
};

/**
 * Which marks fall under each heading.
 *
 * The contents are the one place that can say *where* in the document a search
 * found something, and the ticks are already a map of the document — so the
 * hits are marked on them. Walking the body once in document order is what
 * attributes a hit to a heading: the last heading passed is the one it is in.
 *
 * Every colour under a heading, not the first: a section that holds two marks
 * is a different answer from one that holds either of them, and the reader
 * asked the question by keeping both.
 */
function hitsByHeading(body: HTMLElement): Map<string, string[]> {
  const hits = new Map<string, string[]>();
  let heading: string | null = null;

  const walker = document.createTreeWalker(body, NodeFilter.SHOW_ELEMENT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (!(node instanceof HTMLElement)) {
      continue;
    }
    if (/^H[1-6]$/.test(node.tagName) && node.id) {
      heading = node.id;
      continue;
    }
    const pinned = node.classList.contains("pinned-highlight");
    if (!pinned && !node.classList.contains("search-highlight")) {
      continue;
    }
    if (heading === null) {
      continue;
    }
    const name = pinned ? node.dataset.color : undefined;
    const colour = (name && HIT_COLOURS[name]) || "var(--data-lemon-color-emphasis)";
    const found = hits.get(heading);
    if (!found) {
      hits.set(heading, [colour]);
    } else if (!found.includes(colour)) {
      found.push(colour);
    }
  }

  return hits;
}

/**
 * The dots on a contents row: one per mark found under that heading.
 *
 * Separate shapes with air between them, because that is the only way two
 * colours read as two — banded into one six-pixel dot they are a longer dot.
 * The row keeps the box; this fills it.
 */
function drawHits(row: HTMLElement, colours: string[]): void {
  const box = row.querySelector<HTMLElement>(".contents-toc-hits");
  if (!box) {
    return;
  }
  const drawn = Array.from(box.children).map((dot) =>
    dot instanceof HTMLElement ? dot.style.background : "",
  );
  if (drawn.length === colours.length && drawn.every((c, i) => c === colours[i])) {
    return;
  }
  box.replaceChildren(
    ...colours.map((colour) => {
      const dot = document.createElement("i");
      dot.style.background = colour;
      return dot;
    }),
  );
}

/**
 * How far past the top of the reading area a heading counts as behind us.
 *
 * A heading sitting exactly on the fold is the one being read, not the one
 * before it, and a rounding error either way should not flicker the tick.
 */
const HEADING_MARGIN = 4;

let scheduled = false;

/**
 * The scroller the listener is on.
 *
 * Dioxus can replace the element, and it does not exist at all in a page
 * `arto page` wrote, so the listener is attached where the element is found
 * rather than once at startup.
 */
let listening: HTMLElement | null = null;

function scroller(): HTMLElement | null {
  return document.querySelector(".content");
}

/**
 * The page whose height everything here is measured against.
 *
 * It does not arrive at its full height: code blocks are highlighted,
 * diagrams and formulae are drawn, images load, and each of those makes the
 * page taller after it was last measured. A scroll does not necessarily
 * follow — the reader may still be looking at the first screen — so nothing
 * else would ask for the measurement again, and the trace would sit level
 * with the middle of a document that no longer exists.
 */
let measured: HTMLElement | null = null;

/**
 * The reading area the page is centred in.
 *
 * What the trace is placed against is the margin between the two, and the
 * area is the side of it that the panel moves: widening a pinned panel takes
 * the margin without touching the page, which keeps its own width and simply
 * has less room to be centred in.
 */
let framed: HTMLElement | null = null;
let growth: ResizeObserver | null = null;

/** Move the one observation kept in a slot from `current` to `next`. */
function watch(current: HTMLElement | null, next: HTMLElement | null): HTMLElement | null {
  if (current === next) {
    return current;
  }
  growth ??= new ResizeObserver(() => refreshReadingPosition());
  if (current) {
    growth.unobserve(current);
  }
  if (next) {
    growth.observe(next);
  }
  return next;
}

function update(): void {
  scheduled = false;

  const content = scroller();
  if (!content) {
    return;
  }
  if (listening !== content) {
    listening?.removeEventListener("scroll", refreshReadingPosition);
    content.addEventListener("scroll", refreshReadingPosition, { passive: true });
    listening = content;
  }

  const indicator = document.querySelector<HTMLElement>(".scroll-indicator");
  if (indicator) {
    drawScrollIndicator(indicator, content);
  }

  const area = content.closest<HTMLElement>(".content-area");
  const body = content.querySelector<HTMLElement>(".markdown-body");
  measured = watch(measured, body);
  framed = watch(framed, content);
  if (area) {
    const document_ = body?.getBoundingClientRect().height ?? content.clientHeight;
    const window_ = content.clientHeight;
    area.style.setProperty(MARGIN_EXTENT, `${Math.round(Math.min(document_, window_))}px`);

    // Layout coordinates for the columns, so that the offset already applied
    // to one does not feed back into the next measurement. The page is
    // measured where it is painted, because zoom is what closes these
    // margins: the page is magnified, the columns beside it are not.
    // `offsetLeft` reads the same space, both columns being outside the
    // zoomed wrapper.
    const page = body?.getBoundingClientRect();
    const areaLeft = area.getBoundingClientRect().left;

    const trace = area.querySelector<HTMLElement>(".margin-trace");
    if (trace && page) {
      const traceRight = trace.offsetLeft + trace.offsetWidth;
      const { offset, crowded } = marginColumn(page.left - areaLeft - traceRight);
      area.style.setProperty(TRACE_OFFSET, `${offset}px`);
      area.toggleAttribute(TRACE_CROWDED, crowded);
      // Until this has run once, the column is still at the window's edge
      // rather than beside the text; drawn there and then moved, it reads as
      // the page settling into place after the reader is already looking.
      area.dataset.traceReady = "";
    }

    // The ruler's own margin, which is the trace's read from the other side:
    // it is laid out against the right edge and travels left. Measured
    // against the area and the column's width rather than against where the
    // column currently is, because that is what the offset moves: reading it
    // back would make each measurement an answer to the last one.
    const gutter = area.querySelector<HTMLElement>(".contents-gutter");
    // Whether the page keeps the ruler's column back. Answered by looking for
    // the ruler rather than by asking whether the width allows one: a
    // document with no headings and nothing pinned has no map to draw, and
    // the page would have given up the column to nothing.
    area.toggleAttribute(RULER, gutter !== null);
    if (gutter && page) {
      const shelf = area.clientWidth - gutter.offsetWidth;
      const { offset, crowded } = marginColumn(shelf - (page.right - areaLeft));
      area.style.setProperty(GUTTER_OFFSET, `${offset}px`);
      area.toggleAttribute(GUTTER_CROWDED, crowded);
      area.dataset.gutterReady = "";
    } else {
      // What the ruler left behind, when the width folds it away or the
      // document has no headings: the contents are still opened by name, and
      // they are placed against the column that is no longer there.
      area.style.setProperty(GUTTER_OFFSET, "0px");
      area.toggleAttribute(GUTTER_CROWDED, false);
    }
  }

  // The ticks and the names they stand for are two views of one list, so the
  // current heading is marked on both.
  const marks = document.querySelectorAll<HTMLElement>(
    ".contents-gutter-tick[data-heading], .contents-toc-row[data-heading]",
  );
  if (marks.length === 0) {
    return;
  }

  const fold = content.getBoundingClientRect().top + HEADING_MARGIN;
  let currentId: string | null = null;
  for (const mark of marks) {
    const id = mark.dataset.heading;
    const heading = id ? document.getElementById(id) : null;
    if (!id || !heading) {
      continue;
    }
    // Above the first heading there is no heading to be inside, so the first
    // one stands in: the reader is in the document it opens.
    currentId ??= id;
    if (heading.getBoundingClientRect().top <= fold) {
      currentId = id;
    }
  }

  const hits = body ? hitsByHeading(body) : new Map<string, string[]>();

  for (const mark of marks) {
    if (mark.dataset.heading === currentId) {
      mark.setAttribute(CURRENT, "");
    } else {
      mark.removeAttribute(CURRENT);
    }

    const hit = (mark.dataset.heading ? hits.get(mark.dataset.heading) : undefined) ?? [];
    const row = mark.classList.contains("contents-toc-row");
    if (hit.length > 0) {
      mark.setAttribute(HIT, "");
      // The ruler answers "is there something here", which one colour says as
      // well as five; the list beside it is where they are told apart.
      mark.style.setProperty("--hit-colour", hit[0]);
    } else {
      mark.removeAttribute(HIT);
      mark.style.removeProperty("--hit-colour");
    }
    if (row) {
      drawHits(mark, hit);
    }
  }
}

/** Recompute on the next frame; several calls in one frame cost one pass. */
export function refreshReadingPosition(): void {
  if (scheduled) {
    return;
  }
  scheduled = true;
  requestAnimationFrame(update);
}

export function setupReadingPosition(): void {
  window.addEventListener("resize", refreshReadingPosition, { passive: true });
  refreshReadingPosition();
}
