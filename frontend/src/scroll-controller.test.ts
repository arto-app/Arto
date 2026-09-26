import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { down, setLineStep, toElement } from "./scroll-controller";

describe("focus mode's line step", () => {
  const forget = vi.fn();
  let block: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML =
      '<div class="content"><article class="markdown-body"><p>one</p></article></div>';
    block = document.querySelector<HTMLElement>("p")!;
    vi.spyOn(block, "scrollIntoView").mockImplementation(() => {});
    forget.mockClear();
    setLineStep(() => ({ kind: "bring", block, place: "center" }), forget);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  test("keeps the step a line key is taking", () => {
    down();
    expect(block.scrollIntoView).toHaveBeenCalled();
    expect(forget).not.toHaveBeenCalled();
  });

  test("is forgotten when the page is sent somewhere else", () => {
    toElement(block, "start");
    expect(forget).toHaveBeenCalled();
  });
});
