import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";
import { writeTextToClipboard, writeImageToClipboard } from "./code-copy";

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
