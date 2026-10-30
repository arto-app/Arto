import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";
import { convertSvgToDataUrl, writeTextToClipboard, writeImageToClipboard } from "./code-copy";

/** Replace `navigator.clipboard`, which is read-only on the real object. */
function stubClipboard(clipboard: unknown): void {
  Object.defineProperty(navigator, "clipboard", {
    value: clipboard,
    configurable: true,
    writable: true,
  });
}

beforeEach(() => {
  delete window.rustCopyText;
  delete window.rustCopyImage;
  stubClipboard(undefined);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("writeTextToClipboard", () => {
  test("hands the text to the app's clipboard bridge when one is registered", async () => {
    const rustCopyText = vi.fn();
    window.rustCopyText = rustCopyText;
    const writeText = vi.fn().mockResolvedValue(undefined);
    stubClipboard({ writeText });

    await writeTextToClipboard("hello");

    expect(rustCopyText).toHaveBeenCalledWith("hello");
    expect(writeText).not.toHaveBeenCalled();
  });

  test("falls back to the browser clipboard outside the app", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    stubClipboard({ writeText });

    await writeTextToClipboard("hello");

    expect(writeText).toHaveBeenCalledWith("hello");
  });

  test("rejects when neither is available, so the button reports failure", async () => {
    await expect(writeTextToClipboard("hello")).rejects.toThrow();
  });
});

describe("writeImageToClipboard", () => {
  const blob = new Blob(["png"], { type: "image/png" });
  const dataUrl = "data:image/png;base64,cG5n";

  test("hands the data URL to the app's clipboard bridge when one is registered", async () => {
    const rustCopyImage = vi.fn();
    window.rustCopyImage = rustCopyImage;
    const write = vi.fn().mockResolvedValue(undefined);
    stubClipboard({ write });

    await writeImageToClipboard(blob, dataUrl);

    expect(rustCopyImage).toHaveBeenCalledWith(dataUrl);
    expect(write).not.toHaveBeenCalled();
  });

  test("falls back to the browser clipboard with a PNG item outside the app", async () => {
    const items: Record<string, Blob>[] = [];
    vi.stubGlobal(
      "ClipboardItem",
      class {
        constructor(entries: Record<string, Blob>) {
          items.push(entries);
        }
      },
    );
    const write = vi.fn().mockResolvedValue(undefined);
    stubClipboard({ write });

    await writeImageToClipboard(blob, dataUrl);

    expect(write).toHaveBeenCalledTimes(1);
    expect(items).toEqual([{ "image/png": blob }]);
  });

  test("rejects when neither is available, so the button reports failure", async () => {
    await expect(writeImageToClipboard(blob, dataUrl)).rejects.toThrow();
  });
});

describe("convertSvgToDataUrl", () => {
  function decoded(dataUrl: string): string {
    const base64 = dataUrl.replace(/^data:image\/svg\+xml;base64,/, "");
    return decodeURIComponent(escape(atob(base64)));
  }

  test("an image of a diagram carries the colours the page resolved for it", () => {
    document.body.innerHTML = `<style>.pele { --pele-bg: #0d1117; --pele-fg: #f0f6fc; }</style>
      <svg xmlns="http://www.w3.org/2000/svg" class="pele" viewBox="0 0 10 10"
        style="--_bg:var(--pele-bg,#fff);--_fg:var(--pele-fg,#1f1f1f)"></svg>`;
    const svg = document.querySelector("svg") as SVGSVGElement;

    const image = decoded(convertSvgToDataUrl(svg, { width: 10, height: 10 }));

    expect(image).toContain("--pele-bg: #0d1117");
    expect(image).toContain("--pele-fg: #f0f6fc");
  });

  test("a variable the page leaves unset keeps the diagram's own fallback", () => {
    document.body.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" class="pele" viewBox="0 0 10 10"
        style="--_bg:var(--pele-bg,#fff)"></svg>`;
    const svg = document.querySelector("svg") as SVGSVGElement;

    const image = decoded(convertSvgToDataUrl(svg, { width: 10, height: 10 }));

    expect(image).not.toContain("--pele-bg:");
  });
});
