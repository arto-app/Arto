import { describe, test, expect } from "vitest";

import { TRACE_GAP, traceColumn } from "./reading-position";

describe("traceColumn", () => {
  test("moves the column up against the page, a gap short of the text", () => {
    // A wide window: the page is centred far from the area's left edge, so
    // the trace travels most of that distance to reach it.
    expect(traceColumn(500, 138)).toEqual({ offset: 500 - 138 - TRACE_GAP, crowded: false });
  });

  test("stays where it is laid out when the page is exactly a gap away", () => {
    expect(traceColumn(138 + TRACE_GAP, 138)).toEqual({ offset: 0, crowded: false });
  });

  test("rounds to whole pixels", () => {
    expect(traceColumn(300.6, 138).offset).toEqual(Math.round(300.6 - 138 - TRACE_GAP));
  });

  test("gives way when the margin cannot hold it at that distance", () => {
    // Magnifying the page widens the column while the trace keeps its size,
    // so the margin closes from the document's side.
    expect(traceColumn(138 + TRACE_GAP - 1, 138)).toEqual({ offset: 0, crowded: true });
  });

  test("gives way rather than being written over the text", () => {
    expect(traceColumn(20, 138)).toEqual({ offset: 0, crowded: true });
  });
});
