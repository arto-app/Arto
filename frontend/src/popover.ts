/**
 * Where a card laid over the page goes: beside what it is about, on screen,
 * and at the zoom the page is drawn at.
 *
 * Shared by every popover that hangs from the body rather than from the
 * page, so that a lens's note and a link's preview sit the same way.
 */

const MARGIN = 8;
const FALLBACK_WIDTH = 480;

/** The zoom the page `el` is on is drawn at: 1 when it is not zoomed. */
export function contentZoom(el: Element): number {
  let zoom = 1;
  for (let node = el.parentElement; node; node = node.parentElement) {
    const own = Number.parseFloat(node.style.zoom);
    if (Number.isFinite(own) && own > 0) zoom *= own;
  }
  return zoom;
}

/**
 * Put `popover` below `anchor`, or above it when it does not fit below,
 * kept inside the window.
 *
 * Drawn at the document's zoom, so what it says reads at the size of the
 * text it is about. The popover hangs from the body, outside the zoomed
 * page, so it takes the zoom itself — and its own offsets are zoomed with
 * it. It has to be showing already, since its size is measured.
 */
export function placePopover(popover: HTMLElement, anchor: Element): void {
  const zoom = contentZoom(anchor);
  popover.style.zoom = String(zoom);
  // The stylesheet caps a popover's size, and those caps are zoomed along
  // with it: at twice the size, a card half the window wide would not fit.
  // The room the window has is handed over in the popover's own pixels.
  popover.style.setProperty("--popover-room-x", `${(window.innerWidth - MARGIN * 2) / zoom}px`);
  popover.style.setProperty("--popover-room-y", `${window.innerHeight / 2 / zoom}px`);
  const rect = anchor.getBoundingClientRect();
  const size = popover.getBoundingClientRect();
  const width = Math.min(size.width || FALLBACK_WIDTH, window.innerWidth - MARGIN * 2);
  const left = Math.min(Math.max(rect.left, MARGIN), window.innerWidth - width - MARGIN);
  const below = rect.bottom + MARGIN;
  const fitsBelow = below + size.height <= window.innerHeight - MARGIN;
  const top = fitsBelow ? below : Math.max(MARGIN, rect.top - size.height - MARGIN);
  popover.style.left = `${left / zoom}px`;
  popover.style.top = `${top / zoom}px`;
}
