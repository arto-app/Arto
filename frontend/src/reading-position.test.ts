import { describe, test, expect } from "vitest";

import { MARGIN_GAP, marginColumn } from "./reading-position";

describe("marginColumn", () => {
  test("moves the column up against the page, a gap short of the text", () => {
    // A wide window: the page is centred far from the edge the column is laid
    // out at, so the column travels most of that distance to reach it.
    expect(marginColumn(500)).toEqual({ offset: 500 - MARGIN_GAP, crowded: false });
  });

  test("stays where it is laid out when the page is exactly a gap away", () => {
    expect(marginColumn(MARGIN_GAP)).toEqual({ offset: 0, crowded: false });
  });

  test("rounds to whole pixels", () => {
    expect(marginColumn(300.6).offset).toEqual(Math.round(300.6 - MARGIN_GAP));
  });

  test("gives way when the margin cannot hold it at that distance", () => {
    // Magnifying the page widens the column while the margin's occupant keeps
    // its size, so the margin closes from the document's side.
    expect(marginColumn(MARGIN_GAP - 1)).toEqual({ offset: 0, crowded: true });
  });

  test("gives way rather than being written over the text", () => {
    expect(marginColumn(-118)).toEqual({ offset: 0, crowded: true });
  });
});
