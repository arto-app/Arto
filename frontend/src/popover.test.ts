import { describe, test, expect, beforeEach, vi } from "vitest";
import { contentZoom, placePopover } from "./popover";

function rect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    x: left,
    y: top,
    toJSON: () => ({}),
  };
}

beforeEach(() => {
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

describe("contentZoom", () => {
  test("multiplies the zoom of every ancestor", () => {
    document.body.innerHTML = `<div style="zoom: 2"><div style="zoom: 1.5"><p>text</p></div></div>`;
    expect(contentZoom(document.querySelector("p") as Element)).toBe(3);
  });

  test("is 1 on a page that is not zoomed", () => {
    document.body.innerHTML = `<p>text</p>`;
    expect(contentZoom(document.querySelector("p") as Element)).toBe(1);
  });
});

describe("placePopover", () => {
  function setup(anchorRect: DOMRect, size: DOMRect): { popover: HTMLElement; anchor: Element } {
    document.body.innerHTML = `<span id="anchor">link</span><div id="popover"></div>`;
    const anchor = document.getElementById("anchor") as Element;
    const popover = document.getElementById("popover") as HTMLElement;
    vi.spyOn(anchor, "getBoundingClientRect").mockReturnValue(anchorRect);
    vi.spyOn(popover, "getBoundingClientRect").mockReturnValue(size);
    return { popover, anchor };
  }

  test("hangs below the anchor when there is room", () => {
    const { popover, anchor } = setup(rect(100, 100, 50, 20), rect(0, 0, 200, 100));
    placePopover(popover, anchor);
    expect(popover.style.left).toBe("100px");
    expect(popover.style.top).toBe("128px");
  });

  test("goes above the anchor when it would not fit below", () => {
    const bottom = window.innerHeight - 30;
    const { popover, anchor } = setup(rect(100, bottom, 50, 20), rect(0, 0, 200, 100));
    placePopover(popover, anchor);
    expect(popover.style.top).toBe(`${bottom - 100 - 8}px`);
  });

  test("takes the zoom of the page the anchor is on", () => {
    document.body.innerHTML = `<div style="zoom: 2"><span id="anchor">link</span></div><div id="popover"></div>`;
    const anchor = document.getElementById("anchor") as Element;
    const popover = document.getElementById("popover") as HTMLElement;
    vi.spyOn(anchor, "getBoundingClientRect").mockReturnValue(rect(100, 100, 50, 20));
    vi.spyOn(popover, "getBoundingClientRect").mockReturnValue(rect(0, 0, 200, 100));
    placePopover(popover, anchor);
    expect(popover.style.zoom).toBe("2");
    expect(popover.style.left).toBe("50px");
    expect(popover.style.top).toBe("64px");
  });

  test("is given the room the window has, in the popover's own zoomed pixels", () => {
    document.body.innerHTML = `<div style="zoom: 2"><span id="anchor">link</span></div><div id="popover"></div>`;
    const anchor = document.getElementById("anchor") as Element;
    const popover = document.getElementById("popover") as HTMLElement;
    placePopover(popover, anchor);
    expect(popover.style.getPropertyValue("--popover-room-x")).toBe(
      `${(window.innerWidth - 16) / 2}px`,
    );
    expect(popover.style.getPropertyValue("--popover-room-y")).toBe(
      `${window.innerHeight / 2 / 2}px`,
    );
  });
});
