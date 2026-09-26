import { afterEach, describe, expect, test } from "vitest";

import { clearCursor, getCurrentElement, next } from "./content-cursor";

describe("clearCursor", () => {
  afterEach(() => {
    clearCursor();
    document.body.innerHTML = "";
  });

  test("says it put a cursor away", () => {
    document.body.innerHTML = '<article class="markdown-body"><p>one</p><p>two</p></article>';
    next();
    expect(getCurrentElement()).not.toBeNull();

    expect(clearCursor()).toBe(true);
    expect(getCurrentElement()).toBeNull();
  });

  test("says there was nothing to put away", () => {
    document.body.innerHTML = '<article class="markdown-body"><p>one</p></article>';

    expect(clearCursor()).toBe(false);
  });
});
