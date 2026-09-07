/// Scroll controller for Arto keybinding system.
///
/// Provides programmatic scroll control for the content area,
/// called from Rust via document::eval().

import {
  afterScroll,
  releaseDestination,
  scrollContainer,
  settleAt,
  withinSmoothDistance,
  withinSmoothReach,
} from "./scroll-destination";

const SCROLL_LINE_HEIGHT = 60;
const SCROLL_HALF_PAGE_RATIO = 0.5;

function getContentElement(): HTMLElement | null {
  return scrollContainer();
}

function scrollBy(el: HTMLElement, delta: number): void {
  releaseDestination();
  el.scrollBy({ top: delta, behavior: "smooth" });
}

function scrollTo(el: HTMLElement, top: number): void {
  releaseDestination();
  el.scrollTo({ top, behavior: "smooth" });
}

/**
 * Jump to the top of a document that has just been replaced.
 *
 * Instant rather than animated: the reader asked for a different document,
 * not for a journey through the one being thrown away. Going through here
 * rather than scrolling `.content` directly is what gives up a destination
 * still being held from the previous document — otherwise the first block
 * that draws in the new one carries the reader to the old document's anchor.
 */
export function reset(): void {
  releaseDestination();
  getContentElement()?.scrollTo(0, 0);
}

export function down(): void {
  const el = getContentElement();
  if (el) scrollBy(el, SCROLL_LINE_HEIGHT);
}

export function up(): void {
  const el = getContentElement();
  if (el) scrollBy(el, -SCROLL_LINE_HEIGHT);
}

export function pageDown(): void {
  const el = getContentElement();
  if (el) scrollBy(el, el.clientHeight);
}

export function pageUp(): void {
  const el = getContentElement();
  if (el) scrollBy(el, -el.clientHeight);
}

export function halfPageDown(): void {
  const el = getContentElement();
  if (el) scrollBy(el, el.clientHeight * SCROLL_HALF_PAGE_RATIO);
}

export function halfPageUp(): void {
  const el = getContentElement();
  if (el) scrollBy(el, -el.clientHeight * SCROLL_HALF_PAGE_RATIO);
}

/**
 * Go to the top of the document.
 *
 * No settling is needed once there: nothing above the top can change height.
 * The journey does take the same reach test as [`toBottom`] — travelling
 * through a long document is what lays out and draws every block on the way,
 * and from the end of a long one the top is the furthest there is.
 */
export function toTop(): void {
  const el = getContentElement();
  if (!el) {
    return;
  }
  if (!withinSmoothDistance(el, el.scrollTop)) {
    releaseDestination();
    el.scrollTo(0, 0);
    return;
  }
  scrollTo(el, 0);
}

/**
 * Go to `target`, and stay on it while the document settles.
 *
 * The target moves as the blocks around it draw their diagrams, and arriving
 * is what makes them draw, so where it was when the jump started is not where
 * it ends up. Correcting during the journey would mean jumping repeatedly in
 * front of the reader; the journey is left alone and the correction happens
 * after it lands.
 *
 * Everything that sends the reader to a particular element goes through here —
 * a heading, a search match, the content cursor — so that none of them lands
 * on a place the next drawn diagram then carries away.
 */
export function toElement(target: Element, block: ScrollLogicalPosition = "start"): void {
  const el = getContentElement();
  if (!el) {
    return;
  }

  const arrive = (): void => {
    target.scrollIntoView({ block, behavior: "instant" });
  };

  if (!withinSmoothReach(el, target)) {
    settleAt(arrive);
    return;
  }
  afterScroll(el, () => settleAt(arrive));
  target.scrollIntoView({ block, behavior: "smooth" });
}

/** [`toElement`] for the heading with `id`, if the document has one. */
export function toHeading(id: string): void {
  const target = document.getElementById(id);
  if (target) {
    toElement(target, "start");
  }
}

/**
 * Go to the end of the document.
 *
 * The journey and the arriving are asked for differently. `scrollHeight` grows
 * as the diagrams on the way there draw, so a smooth scroll aimed at the
 * number it had when it set off lands short; naming the last block instead
 * hands the arriving to the browser, whatever those blocks turn out to
 * measure. Once there, `scrollHeight` is the right target and re-applying it
 * under [`settleAt`] converges on the true end as the document stops growing.
 *
 * The end of a long document is the furthest journey there is, so it takes the
 * same reach test as [`toHeading`]: animating it would draw every block
 * between here and there, which is the whole document.
 */
export function toBottom(): void {
  const el = getContentElement();
  if (!el) {
    return;
  }
  const last = document.querySelector(".markdown-body")?.lastElementChild;
  if (!last) {
    // Nothing lazily laid out to settle: the preferences page and the like
    // are short and their height is already true.
    scrollTo(el, el.scrollHeight);
    return;
  }

  // Clamped by the browser to the last scrollable pixel, which is the end
  // including whatever padding sits under the last block.
  const atEnd = (): void => {
    el.scrollTo({ top: el.scrollHeight });
  };

  if (!withinSmoothReach(el, last)) {
    settleAt(atEnd);
    return;
  }
  afterScroll(el, () => settleAt(atEnd));
  last.scrollIntoView({ block: "end", behavior: "smooth" });
}
