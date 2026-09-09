/**
 * The document's scrollbar, for as long as the pointer is reaching for it.
 *
 * The bar is drawn as a hairline: the page has a better position indicator
 * beside it, and two of them down one edge is one too many. But a hairline is
 * also what made it hard to catch, and the thing a reader does before catching
 * a scrollbar is move towards the edge of the window — so that is what brings
 * it up to a native width, before the aiming starts rather than after it has
 * failed. The drawing changes; the target never does (`scrollbar.css`).
 */

/** How near the edge counts as reaching for the bar. */
export const REACH = 64;

/** The class the stylesheet reads, on `body`. */
const REACHING = "pointer-near-scrollbar";

/**
 * Whether a pointer is reaching for the bar down the right edge of `box`.
 *
 * The bar runs the height of the reading area, so anywhere down that edge is
 * the same intention — but only down *that* edge: a pointer level with the
 * header, or with a window's worth of page below the scroller, is not on its
 * way to the bar however near the right of the window it is.
 */
export function reaching(x: number, y: number, box: DOMRect): boolean {
  if (y < box.top || y > box.bottom || x > box.right) {
    return false;
  }
  return box.right - x <= REACH;
}

/**
 * The scroller the bar belongs to.
 *
 * Dioxus can replace the element, and a page `arto page` wrote has none at
 * all, so it is looked up rather than held.
 */
let scroller: HTMLElement | null = null;

/**
 * Where the scroller is, measured at most once a frame.
 *
 * A pointer crossing the window sends events faster than the box can change,
 * and reading it back on each one is a layout forced per event.
 */
let box: DOMRect | null = null;
let measuring = false;

function measure(): DOMRect | null {
  if (!scroller?.isConnected) {
    scroller = document.querySelector<HTMLElement>(".content");
    box = null;
  }
  if (!scroller) {
    return null;
  }
  if (!box) {
    box = scroller.getBoundingClientRect();
    if (!measuring) {
      measuring = true;
      requestAnimationFrame(() => {
        measuring = false;
        box = null;
      });
    }
  }
  return box;
}

export function setupScrollbarReach(): void {
  document.addEventListener(
    "mousemove",
    (event) => {
      const rect = measure();
      if (!rect) {
        return;
      }
      document.body.classList.toggle(REACHING, reaching(event.clientX, event.clientY, rect));
    },
    { passive: true },
  );

  // A pointer that leaves the window stopped reaching wherever it went out —
  // and the way out it takes most often is straight over the bar, which is
  // where the last move it sent from was a reach. Three ways of hearing that
  // it has gone, because the reader who left the window rightwards is the one
  // most likely to be left looking at a bar that stayed wide: `mouseleave` on
  // the document, a `mouseout` with nothing on the other side of it, and the
  // window losing focus to whatever they went to instead.
  const away = (): void => document.body.classList.remove(REACHING);
  document.addEventListener("mouseleave", away);
  document.addEventListener("mouseout", (event) => {
    if (!event.relatedTarget) {
      away();
    }
  });
  window.addEventListener("blur", away);
}
