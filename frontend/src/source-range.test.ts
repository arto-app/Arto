import { describe, test, expect } from "vitest";
import { parseSourceRange, readSourceRange } from "./source-range";

describe("parseSourceRange", () => {
  test("reads both ends as lines and columns", () => {
    expect(parseSourceRange("12:3-14:20")).toEqual({
      start: { line: 12, column: 3 },
      end: { line: 14, column: 20 },
    });
  });

  test("rejects anything that is not the documented shape", () => {
    expect(parseSourceRange(undefined)).toBeNull();
    expect(parseSourceRange("")).toBeNull();
    expect(parseSourceRange("12")).toBeNull();
    expect(parseSourceRange("12:3-14")).toBeNull();
    expect(parseSourceRange("a:1-2:3")).toBeNull();
  });

  test("rejects positions that name no character of a file", () => {
    expect(parseSourceRange("0:1-1:1")).toBeNull();
    expect(parseSourceRange("1:0-1:1")).toBeNull();
    expect(parseSourceRange("2:1-1:1")).toBeNull();
    expect(parseSourceRange("1:5-1:4")).toBeNull();
    expect(parseSourceRange("99999999999999999999:1-99999999999999999999:1")).toBeNull();
  });

  test("accepts a range of a single character", () => {
    expect(parseSourceRange("3:7-3:7")?.start).toEqual({ line: 3, column: 7 });
  });
});

describe("readSourceRange", () => {
  test("reads the element's data-source-range attribute", () => {
    const el = document.createElement("p");
    el.dataset.sourceRange = "3:1-4:15";
    expect(readSourceRange(el)?.end).toEqual({ line: 4, column: 15 });
  });

  test("gives null for an element without one", () => {
    expect(readSourceRange(document.createElement("p"))).toBeNull();
  });
});
