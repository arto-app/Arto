import { describe, test, expect, beforeEach, afterEach } from "vitest";

import * as capture from "./capture";
import { provideHtml2Canvas, type Html2CanvasLibrary } from "./libraries";
import * as viewportQueue from "./viewport-queue";

/** A document of top-level blocks, each with the range it was rendered from. */
function documentOf(blocks: Array<[tag: string, range: string, id?: string]>): HTMLElement[] {
  document.body.innerHTML = `<div class="content"><article class="markdown-body"></article></div>`;
  const body = document.querySelector(".markdown-body")!;
  return blocks.map(([tag, range, id]) => {
    const element = document.createElement(tag);
    element.dataset.sourceRange = range;
    if (id) element.id = id;
    body.append(element);
    return element;
  });
}

/** A canvas of a size, whose picture is always the same bytes. */
function canvasOf(width: number, height: number, png = "UE5H"): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  canvas.toDataURL = () => `data:image/png;base64,${png}`;
  return canvas;
}

/** A stand-in html2canvas: records what it was asked to draw, answers with a canvas of a size. */
function fakeHtml2Canvas(width = 200, height = 100, before?: () => void) {
  const drawn: HTMLElement[] = [];
  const library: Html2CanvasLibrary = async (element) => {
    before?.();
    drawn.push(element);
    return canvasOf(width, height);
  };
  provideHtml2Canvas(library);
  return { drawn };
}

beforeEach(() => {
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { ready: Promise.resolve() },
  });
});

afterEach(() => {
  viewportQueue.reset();
  document.body.innerHTML = "";
});

describe("blockForLine", () => {
  test("names the block whose range holds the line", () => {
    const [, list] = documentOf([
      ["p", "1:1-2:10"],
      ["ul", "4:1-7:3"],
    ]);
    expect(capture.blockForLine(5)).toBe(list);
  });

  test("gives a line between two blocks to the one above it", () => {
    const [paragraph] = documentOf([
      ["p", "1:1-2:10"],
      ["ul", "4:1-7:3"],
    ]);
    expect(capture.blockForLine(3)).toBe(paragraph);
  });

  test("names nothing for a line before the first block", () => {
    documentOf([["p", "3:1-3:5"]]);
    expect(capture.blockForLine(1)).toBeNull();
  });

  test("names the last block for a line past the end", () => {
    const [, last] = documentOf([
      ["p", "1:1-1:5"],
      ["p", "3:1-3:5"],
    ]);
    expect(capture.blockForLine(99)).toBe(last);
  });
});

describe("blockForHeading", () => {
  test("names the heading's own block", () => {
    const [, heading] = documentOf([
      ["p", "1:1-1:5"],
      ["h2", "3:1-3:10", "install"],
    ]);
    expect(capture.blockForHeading("install")).toBe(heading);
  });

  test("names nothing for an id no heading has", () => {
    documentOf([["h2", "1:1-1:5", "install"]]);
    expect(capture.blockForHeading("usage")).toBeNull();
  });
});

describe("captureLine", () => {
  test("draws the block and says which lines it holds", async () => {
    const [, list] = documentOf([
      ["p", "1:1-2:10"],
      ["ul", "4:1-7:3"],
    ]);
    const { drawn } = fakeHtml2Canvas(400, 300);

    const picture = await capture.captureLine(5);

    expect(drawn).toEqual([list]);
    expect(picture).toEqual({ png: "UE5H", width: 400, height: 300, lines: { start: 4, end: 7 } });
  });

  test("draws a diagram the block is still waiting for before the block", async () => {
    const [block] = documentOf([["div", "1:1-5:3"]]);
    const order: string[] = [];
    viewportQueue.whenNearViewport(block, () => {
      order.push("diagram");
    });
    fakeHtml2Canvas(1, 1, () => order.push("picture"));

    await capture.captureLine(1);

    expect(order).toEqual(["diagram", "picture"]);
  });

  test("answers nothing when no block holds the line", async () => {
    documentOf([["p", "3:1-3:5"]]);
    const { drawn } = fakeHtml2Canvas();
    expect(await capture.captureLine(1)).toBeNull();
    expect(drawn).toEqual([]);
  });

  test("answers nothing for a picture that came out empty", async () => {
    documentOf([["p", "1:1-1:5"]]);
    fakeHtml2Canvas(0, 0);
    expect(await capture.captureLine(1)).toBeNull();
  });
});

describe("captureViewport", () => {
  test("draws the scroller the document is read in, with no lines", async () => {
    documentOf([["p", "1:1-1:5"]]);
    const { drawn } = fakeHtml2Canvas(800, 600);

    const picture = await capture.captureViewport();

    expect(drawn).toEqual([document.querySelector(".content")]);
    expect(picture).toEqual({ png: "UE5H", width: 800, height: 600 });
  });
});

describe("scaleFor", () => {
  test("follows the screen up to twice", () => {
    expect(capture.scaleFor(100, 100, 1)).toBe(1);
    expect(capture.scaleFor(100, 100, 2)).toBe(2);
    expect(capture.scaleFor(100, 100, 3)).toBe(2);
  });

  test("shrinks a picture that would be too large for a canvas", () => {
    // A long table at twice the pixels would be a canvas the browser draws
    // nothing into.
    expect(capture.scaleFor(800, 16384, 2)).toBe(1);
    expect(capture.scaleFor(800, 32768, 2)).toBe(0.5);
  });
});
