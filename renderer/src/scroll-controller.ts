/**
 * Scroll controller for the keybinding engine.
 *
 * Provides smooth scrolling APIs for the `.content` container.
 * All scroll operations target the `.content` element which is the
 * scrollable container for the markdown viewer.
 */

/** Get the scrollable content container */
function getContentElement(): HTMLElement | null {
  return document.querySelector(".content");
}

/**
 * Scroll the content by a pixel amount (positive = down, negative = up).
 */
export function scrollBy(pixels: number): void {
  const el = getContentElement();
  if (!el) return;

  el.scrollBy({
    top: pixels,
    behavior: "smooth",
  });
}

/**
 * Scroll by one full page (viewport height of the content container).
 */
export function scrollPage(direction: "up" | "down"): void {
  const el = getContentElement();
  if (!el) return;

  const amount = el.clientHeight;
  el.scrollBy({
    top: direction === "down" ? amount : -amount,
    behavior: "smooth",
  });
}

/**
 * Scroll by half a page.
 */
export function scrollHalfPage(direction: "up" | "down"): void {
  const el = getContentElement();
  if (!el) return;

  const amount = Math.floor(el.clientHeight / 2);
  el.scrollBy({
    top: direction === "down" ? amount : -amount,
    behavior: "smooth",
  });
}

/**
 * Scroll to the top or bottom edge.
 */
export function scrollToEdge(edge: "top" | "bottom"): void {
  const el = getContentElement();
  if (!el) return;

  if (edge === "top") {
    el.scrollTo({ top: 0, behavior: "smooth" });
  } else {
    el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
  }
}
