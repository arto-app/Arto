/**
 * Where the reader is in the document, for the two pieces of chrome that
 * answer to it: the header's line and the contents gutter's current tick.
 *
 * Both are the same question asked twice — has the document moved under the
 * header, and which heading is it under — so they share one passive scroll
 * listener and one frame's worth of work.
 */

/** The attribute the stylesheet keys the header's line off. */
const SCROLLED = "data-scrolled";

/**
 * How much of the column the margin trace has to centre itself against.
 *
 * The trace is set level with the middle of what is being read, and what is
 * being read is the document — until the document is taller than the window,
 * when it is the window. So: the smaller of the two.
 */
const TRACE_EXTENT = "--trace-extent";

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
 * How much air is left between the trace and the text it annotates.
 *
 * Kept here rather than as padding on the column, so that widening the gap
 * moves the whole column instead of eating the room its names are set in.
 */
const TRACE_GAP = 56;

/** The attribute that marks the heading the reader is inside. */
const CURRENT = "data-current";

/** The attribute that marks a heading a search has hits under. */
const HIT = "data-hit";

/** The ink a hit is drawn in, by the name the pinned search carries. */
const HIT_COLOURS: Record<string, string> = {
  green: "var(--data-green-color-emphasis)",
  blue: "var(--data-blue-color-emphasis)",
  pink: "var(--data-pink-color-emphasis)",
  orange: "var(--data-orange-color-emphasis)",
  purple: "var(--data-purple-color-emphasis)",
};

/**
 * Which heading each search hit falls under.
 *
 * The contents are the one place that can say *where* in the document a search
 * found something, and the ticks are already a map of the document — so the
 * hits are marked on them. Walking the body once in document order is what
 * attributes a hit to a heading: the last heading passed is the one it is in.
 */
function hitsByHeading(body: HTMLElement): Map<string, string> {
  const hits = new Map<string, string>();
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
    if (heading === null || hits.has(heading)) {
      continue;
    }
    const colour = pinned ? node.dataset.color : undefined;
    hits.set(heading, (colour && HIT_COLOURS[colour]) || "var(--data-lemon-color-emphasis)");
  }

  return hits;
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

  const area = content.closest<HTMLElement>(".content-area");
  const body = content.querySelector<HTMLElement>(".markdown-body");
  if (area) {
    const document_ = body?.getBoundingClientRect().height ?? content.clientHeight;
    const window_ = content.clientHeight;
    area.style.setProperty(TRACE_EXTENT, `${Math.round(Math.min(document_, window_))}px`);

    // Layout coordinates for the trace, so that the offset already applied to
    // it does not feed back into the next measurement.
    const trace = area.querySelector<HTMLElement>(".margin-trace");
    if (trace && body) {
      const traceRight = trace.offsetLeft + trace.offsetWidth;
      const bodyLeft = body.getBoundingClientRect().left - area.getBoundingClientRect().left;
      const offset = Math.max(0, Math.round(bodyLeft - traceRight - TRACE_GAP));
      area.style.setProperty(TRACE_OFFSET, `${offset}px`);
      // Until this has run once, the column is still at the window's edge
      // rather than beside the text; drawn there and then moved, it reads as
      // the page settling into place after the reader is already looking.
      area.dataset.traceReady = "";
    }
  }

  const root = document.documentElement;
  if (content.scrollTop > 0) {
    root.setAttribute(SCROLLED, "");
  } else {
    root.removeAttribute(SCROLLED);
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

  const hits = body ? hitsByHeading(body) : new Map<string, string>();

  for (const mark of marks) {
    if (mark.dataset.heading === currentId) {
      mark.setAttribute(CURRENT, "");
    } else {
      mark.removeAttribute(CURRENT);
    }

    const hit = mark.dataset.heading ? hits.get(mark.dataset.heading) : undefined;
    if (hit) {
      mark.setAttribute(HIT, "");
      mark.style.setProperty("--hit-colour", hit);
    } else {
      mark.removeAttribute(HIT);
      mark.style.removeProperty("--hit-colour");
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
