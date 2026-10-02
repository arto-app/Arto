/**
 * Following a link to somewhere else on the same page — a footnote, the
 * reference a footnote came from, a heading.
 *
 * The browser's own fragment jump is not used. It lands once, on a position
 * computed while the diagrams on the way are still code blocks, so drawing
 * them carries the target away; and the host may take the click for itself
 * before the browser gets to jump at all. The scroll controller arrives the
 * way every other jump in the app does, and keeps arriving until the page
 * holds still.
 */

import * as scrollController from "./scroll-controller";

const HEADING = /^H[1-6]$/;

function decodeFragment(fragment: string): string {
  try {
    return decodeURIComponent(fragment);
  } catch {
    return fragment;
  }
}

function onClick(event: MouseEvent): void {
  if (event.defaultPrevented || event.button !== 0) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  if (!(event.target instanceof Element)) return;
  const anchor = event.target.closest<HTMLAnchorElement>('a[href^="#"]');
  const body = anchor?.closest(".markdown-body");
  if (!anchor || !body) return;
  const href = anchor.getAttribute("href") ?? "";
  if (href.length < 2) return;
  // A preview or a lens answer is a rendered body too, and may use the same
  // ids as the page; a link is followed only to a place in its own body.
  const id = CSS.escape(decodeFragment(href.slice(1)));
  const target = body.querySelector(`[id="${id}"]`);
  if (!target) return;

  event.preventDefault();
  event.stopPropagation();
  // A heading opens what follows it; a reference or a note is read in the
  // middle of the text around it.
  scrollController.toElement(target, HEADING.test(target.tagName) ? "start" : "center");
}

export function setup(): void {
  // Capturing, so the host never sees the click as a link to open.
  document.addEventListener("click", onClick, true);
}
