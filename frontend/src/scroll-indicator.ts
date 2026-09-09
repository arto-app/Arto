/**
 * The bar that says how far down the page the reader is.
 *
 * The native scrollbar is still the one being used — it keeps its full width
 * and catches every click and drag — but it is not the one being seen: a
 * scrollbar pseudo-element is painted by the engine's own scrollbar theme,
 * which repaints when it feels like it, so a width told to change as the
 * pointer approaches changed once and stayed changed. This is the same bar
 * drawn as an ordinary element, over the invisible one, where a transition is
 * a transition.
 */

/**
 * The shortest the thumb is drawn.
 *
 * On a long document the proportion would put it below a few pixels, which
 * stops reading as a thumb and starts reading as a speck.
 */
export const MIN_THUMB = 28;

/** Where the thumb goes on the track, and how tall it is. */
export interface Thumb {
  top: number;
  height: number;
}

/**
 * The thumb for a scroller, or `null` when there is nothing to scroll.
 *
 * `viewport` is what is on screen, `content` the whole of the page, and
 * `track` the height the thumb runs down — the same as the viewport, unless
 * something is inset from it.
 */
export function thumbFor(
  scrollTop: number,
  viewport: number,
  content: number,
  track: number,
): Thumb | null {
  if (content <= viewport || track <= 0) {
    return null;
  }
  const height = Math.min(track, Math.max(MIN_THUMB, Math.round((viewport / content) * track)));
  const travelled = Math.min(1, Math.max(0, scrollTop / (content - viewport)));
  return { top: Math.round((track - height) * travelled), height };
}

/**
 * Draw the bar for `scroller` on `indicator`.
 *
 * Called from the one place that already measures the reading position, so a
 * scroll costs one frame's work for everything that answers to it.
 */
export function drawScrollIndicator(indicator: HTMLElement, scroller: HTMLElement): void {
  const track = scroller.clientHeight;
  const thumb = thumbFor(scroller.scrollTop, track, scroller.scrollHeight, track);
  const bar = indicator.firstElementChild;
  if (!(bar instanceof HTMLElement)) {
    return;
  }
  indicator.hidden = thumb === null;
  if (!thumb) {
    return;
  }
  bar.style.height = `${thumb.height}px`;
  bar.style.transform = `translateY(${thumb.top}px)`;
}
