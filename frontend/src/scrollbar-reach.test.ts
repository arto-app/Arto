import { describe, test, expect } from "vitest";

import { REACH, reaching } from "./scrollbar-reach";

/** The reading area, as `getBoundingClientRect` reports it. */
const box = { top: 40, bottom: 800, left: 0, right: 1200 } as DOMRect;

describe("reaching", () => {
  test("a pointer on the bar is reaching for it", () => {
    expect(reaching(1200, 400, box)).toBe(true);
  });

  test("a pointer within reach of the edge is reaching for it", () => {
    expect(reaching(1200 - REACH, 400, box)).toBe(true);
  });

  test("a pointer out in the page is not", () => {
    expect(reaching(1200 - REACH - 1, 400, box)).toBe(false);
    expect(reaching(400, 400, box)).toBe(false);
  });

  test("a pointer above or below the reading area is not, however near the edge", () => {
    expect(reaching(1190, 20, box)).toBe(false);
    expect(reaching(1190, 900, box)).toBe(false);
  });

  test("nor is one past the edge entirely", () => {
    expect(reaching(1300, 400, box)).toBe(false);
  });
});
