/**
 * Where the reader is in the document, for the chrome that answers to it: the
 * contents gutter's current tick and the marks a search left on it.
 *
 * All of it is the same question asked in different words — where in this
 * document are we, and what is around us — so it shares one passive scroll
 * listener and one frame's worth of work.
 */

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

  const body = content.querySelector<HTMLElement>(".markdown-body");

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
