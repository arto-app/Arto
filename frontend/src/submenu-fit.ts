/**
 * Keeping a context menu's flyout on screen.
 *
 * A flyout opens to the right of its row and level with it. Opened near
 * the window's right edge — or two submenus deep, three columns across —
 * it runs off the side, and near the bottom it runs off the foot. Where it
 * fits is only known once it is drawn, so it is measured then: it flips to
 * the left of its row when the right has no room and the left has more, and
 * rises until its foot is inside the window.
 */

/** How far from the window's edge a flyout keeps. */
const MARGIN = 4;

/** The class that opens a flyout to the left of its row. */
export const OPENS_LEFT = "opens-left";

/** Fit the flyout of the submenu `id` into the window. */
export function fitSubmenu(id: number): void {
  const flyout = document.querySelector<HTMLElement>(`[data-submenu="${id}"]`);
  if (flyout) fit(flyout, window.innerWidth, window.innerHeight);
}

/** Fit `flyout` into a window `width` by `height`. */
export function fit(flyout: HTMLElement, width: number, height: number): void {
  let rect = flyout.getBoundingClientRect();
  const overRight = rect.right - (width - MARGIN);
  if (overRight > 0) {
    flyout.classList.add(OPENS_LEFT);
    const flipped = flyout.getBoundingClientRect();
    // Too wide for either side: it stays on the side it runs off less.
    if (MARGIN - flipped.left > overRight) {
      flyout.classList.remove(OPENS_LEFT);
    } else {
      rect = flipped;
    }
  }
  if (rect.bottom > height - MARGIN) {
    // Up by as much as it overhangs, but never past the window's top.
    const rise = Math.min(rect.bottom - (height - MARGIN), rect.top - MARGIN);
    if (rise > 0) flyout.style.top = `${-rise}px`;
  }
}
