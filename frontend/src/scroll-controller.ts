/// Scroll controller for Arto keybinding system.
///
/// Provides programmatic scroll control for the content area,
/// called from Rust via document::eval().

const SCROLL_LINE_HEIGHT = 60;
const SCROLL_HALF_PAGE_RATIO = 0.5;

function getContentElement(): HTMLElement | null {
  return document.querySelector(".content") as HTMLElement | null;
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

export function scrollToBottom(): void {
  const el = getContentElement();
  if (el) scrollTo(el, el.scrollHeight);
}
