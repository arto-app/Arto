/// Scroll controller for Arto keybinding system.
///
/// Provides programmatic scroll control for the content area,
/// called from Rust via document::eval().

const SCROLL_LINE_HEIGHT = 60;
const SCROLL_HALF_PAGE_RATIO = 0.5;

/**
 * The element the document scrolls in.
 *
 * The app puts the document inside `.content`; the standalone page renderer
 * has no such element and scrolls the document itself. Everything that reads
 * or sets a scroll position goes through here so the two hosts cannot drift
 * apart.
 */
export function scrollContainer(): HTMLElement | null {
  return (
    document.querySelector<HTMLElement>(".content") ??
    (document.scrollingElement as HTMLElement | null)
  );
}

function getContentElement(): HTMLElement | null {
  return scrollContainer();
}

function scrollBy(el: HTMLElement, delta: number): void {
  el.scrollBy({ top: delta, behavior: "smooth" });
}

function scrollTo(el: HTMLElement, top: number): void {
  el.scrollTo({ top, behavior: "smooth" });
}

export function scrollDown(): void {
  const el = getContentElement();
  if (el) scrollBy(el, SCROLL_LINE_HEIGHT);
}

export function scrollUp(): void {
  const el = getContentElement();
  if (el) scrollBy(el, -SCROLL_LINE_HEIGHT);
}

export function scrollPageDown(): void {
  const el = getContentElement();
  if (el) scrollBy(el, el.clientHeight);
}

export function scrollPageUp(): void {
  const el = getContentElement();
  if (el) scrollBy(el, -el.clientHeight);
}

export function scrollHalfPageDown(): void {
  const el = getContentElement();
  if (el) scrollBy(el, el.clientHeight * SCROLL_HALF_PAGE_RATIO);
}

export function scrollHalfPageUp(): void {
  const el = getContentElement();
  if (el) scrollBy(el, -el.clientHeight * SCROLL_HALF_PAGE_RATIO);
}

export function scrollToTop(): void {
  const el = getContentElement();
  if (el) scrollTo(el, 0);
}

/**
 * How many times [`scrollToBottom`] re-aims at the end of the document.
 *
 * Blocks below the viewport are laid out lazily, so `scrollHeight` is partly
 * an estimate and it changes as the scroll approaches the end and the real
 * heights replace the guesses. Each pass lands closer; a handful is far more
 * than a document needs, and the loop stops as soon as the height holds
 * still.
 */
const SCROLL_TO_BOTTOM_PASSES = 8;

export function scrollToBottom(): void {
  const el = getContentElement();
  if (!el) {
    return;
  }
  let previous = -1;
  let passes = 0;
  const aim = (): void => {
    const height = el.scrollHeight;
    // Jump rather than smooth-scroll on the passes after the first: the
    // reader asked to be at the end, and animating each correction would
    // show the document creeping there.
    if (passes === 0) {
      scrollTo(el, height);
    } else {
      el.scrollTo({ top: height });
    }
    passes += 1;
    if (height === previous || passes >= SCROLL_TO_BOTTOM_PASSES) {
      return;
    }
    previous = height;
    requestAnimationFrame(aim);
  };
  aim();
}
