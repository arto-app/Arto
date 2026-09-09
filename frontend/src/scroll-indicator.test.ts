import { describe, test, expect } from "vitest";

import { MIN_THUMB, thumbFor } from "./scroll-indicator";

describe("thumbFor", () => {
  test("a page that fits has no bar", () => {
    expect(thumbFor(0, 800, 800, 800)).toBeNull();
    expect(thumbFor(0, 800, 400, 800)).toBeNull();
  });

  test("the thumb is the share of the page that is on screen", () => {
    expect(thumbFor(0, 800, 1600, 800)).toEqual({ top: 0, height: 400 });
  });

  test("it travels the track as the page is read", () => {
    expect(thumbFor(800, 800, 1600, 800)).toEqual({ top: 400, height: 400 });
    expect(thumbFor(400, 800, 1600, 800)).toEqual({ top: 200, height: 400 });
  });

  test("a long page still gets a thumb to hold", () => {
    const thumb = thumbFor(0, 800, 200_000, 800);
    expect(thumb?.height).toEqual(MIN_THUMB);
  });

  test("overscroll does not push it off the track", () => {
    expect(thumbFor(10_000, 800, 1600, 800)).toEqual({ top: 400, height: 400 });
    expect(thumbFor(-40, 800, 1600, 800)).toEqual({ top: 0, height: 400 });
  });
});
