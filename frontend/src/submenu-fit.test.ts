import { describe, test, expect } from "vitest";
import { fit, OPENS_LEFT } from "./submenu-fit";

/** A flyout that reports `rect` until it is flipped, and `flipped` after. */
function flyout(rect: Partial<DOMRect>, flipped: Partial<DOMRect> = rect): HTMLElement {
  const el = document.createElement("div");
  el.getBoundingClientRect = () =>
    ({
      top: 0,
      bottom: 0,
      left: 0,
      right: 0,
      ...(el.classList.contains(OPENS_LEFT) ? flipped : rect),
    }) as DOMRect;
  return el;
}

describe("fit", () => {
  test("a flyout with room is left where it opened", () => {
    const el = flyout({ left: 100, right: 300, top: 50, bottom: 200 });

    fit(el, 1000, 800);

    expect(el.classList.contains(OPENS_LEFT)).toBe(false);
    expect(el.style.top).toBe("");
  });

  test("a flyout that runs off the right opens to the left instead", () => {
    const el = flyout({ left: 900, right: 1100, top: 50, bottom: 200 });

    fit(el, 1000, 800);

    expect(el.classList.contains(OPENS_LEFT)).toBe(true);
  });

  test("a flyout with less room on the left than on the right stays on the right", () => {
    const el = flyout(
      { left: 500, right: 1050, top: 50, bottom: 200 },
      { left: -200, right: 350, top: 50, bottom: 200 },
    );

    fit(el, 1000, 800);

    expect(el.classList.contains(OPENS_LEFT)).toBe(false);
  });

  test("a flyout that runs off the foot rises until it fits", () => {
    const el = flyout({ left: 100, right: 300, top: 700, bottom: 900 });

    fit(el, 1000, 800);

    expect(el.style.top).toBe("-104px");
  });

  test("a flyout taller than the room below rises no higher than the window's top", () => {
    const el = flyout({ left: 100, right: 300, top: 50, bottom: 1000 });

    fit(el, 1000, 800);

    expect(el.style.top).toBe("-46px");
  });
});
