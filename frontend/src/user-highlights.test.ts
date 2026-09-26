import { describe, test, expect, beforeEach } from "vitest";

import * as findInPage from "./find-in-page";
import { hitsByHeading } from "./reading-position";
import type { TextAnchor } from "./text-anchor";
import {
  type HighlightDef,
  type Report,
  _reset,
  describeSelection,
  idsAtSelection,
  idsIn,
  lift,
  setup,
  show,
} from "./user-highlights";

function page(html: string): HTMLElement {
  document.body.innerHTML = `<article class="markdown-body">${html}</article>`;
  return document.querySelector(".markdown-body") as HTMLElement;
}

function anchor(exact: string, prefix: string, suffix: string, line = 1): TextAnchor {
  return { exact, prefix, suffix, start: 0, line };
}

function highlight(id: string, a: TextAnchor, color: HighlightDef["color"] = "blue"): HighlightDef {
  return { id, color, anchor: a };
}

function marks(root: HTMLElement): Array<[string, string, string]> {
  return Array.from(root.querySelectorAll<HTMLElement>("mark.user-highlight")).map((m) => [
    m.dataset.highlightId ?? "",
    m.dataset.color ?? "",
    m.textContent ?? "",
  ]);
}

const DOC = `<h1 id="intro" data-source-range="1:1-1:7">Intro</h1>
<p data-source-range="3:1-3:30">The quick <strong>brown</strong> fox jumps.</p>
<h2 id="more" data-source-range="5:1-5:7">More</h2>
<p data-source-range="7:1-7:20">Lazy dogs sleep.</p>`;

let reports: Report[] = [];

beforeEach(() => {
  _reset();
  findInPage.setPinned([]);
  findInPage.clear();
  reports = [];
  setup((report) => reports.push(report));
});

describe("show", () => {
  test("marks each text node a highlight covers, in its colour", () => {
    const root = page(DOC);

    show("/doc.md", [highlight("hl_1", anchor("quick brown fox", "The ", " jumps."), "pink")]);

    expect(marks(root)).toEqual([
      ["hl_1", "pink", "quick "],
      ["hl_1", "pink", "brown"],
      ["hl_1", "pink", " fox"],
    ]);
    expect(root.querySelector("p")?.textContent).toBe("The quick brown fox jumps.");
  });

  test("reports where each was found, under which heading, and which were lost", () => {
    page(DOC);

    show("/doc.md", [
      highlight("hl_1", anchor("dogs", "Lazy ", " sleep.")),
      highlight("hl_2", anchor("cats", "Lazy ", " sleep.")),
    ]);

    expect(reports).toEqual([
      {
        doc: "/doc.md",
        placed: [{ id: "hl_1", start: 43, line: 7, heading: "more" }],
        orphans: ["hl_2"],
      },
    ]);
  });

  test("sends the last report to a listener that arrives late", () => {
    page(DOC);
    _reset();
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);

    const late: Report[] = [];
    setup((report) => late.push(report));

    expect(late.map((report) => report.placed.map((p) => p.id))).toEqual([["hl_1"]]);
  });

  test("draws overlapping highlights one inside the other", () => {
    const root = page(DOC);

    show("/doc.md", [
      highlight("hl_1", anchor("Lazy dogs", "", " sleep.")),
      highlight("hl_2", anchor("dogs sleep", "Lazy ", ".")),
    ]);

    expect(marks(root).map(([id, , text]) => [id, text])).toEqual([
      ["hl_1", "Lazy "],
      ["hl_1", "dogs"],
      ["hl_2", "dogs"],
      ["hl_2", " sleep"],
    ]);
    expect(
      root.querySelector('[data-highlight-id="hl_1"] > [data-highlight-id="hl_2"]')?.textContent,
    ).toBe("dogs");
  });

  test("draws only on the render it was asked for", () => {
    const root = page(DOC);
    root.dataset.renderGeneration = "7";

    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))], 6);
    expect(marks(root)).toEqual([]);
    expect(reports).toEqual([]);

    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))], 7);
    expect(marks(root).map(([id]) => id)).toEqual(["hl_1"]);
  });

  test("replaces what was drawn before, and leaves the page as it was when lifted", () => {
    const root = page(DOC);
    const before = root.innerHTML;

    show("/doc.md", [highlight("hl_1", anchor("quick brown fox", "The ", " jumps."))]);
    show("/doc.md", [highlight("hl_2", anchor("dogs", "Lazy ", " sleep."))]);
    expect(marks(root).map(([id]) => id)).toEqual(["hl_2"]);

    lift();
    expect(root.innerHTML).toBe(before);
  });

  test("takes its marks off blocks held aside from the page", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);
    const aside = root.querySelectorAll("p")[1];
    aside.remove();

    show("/doc.md", []);

    expect(aside.querySelector("mark")).toBeNull();
    expect(aside.textContent).toBe("Lazy dogs sleep.");
  });

  test("leaves its marks on blocks held aside while the other marks are redrawn", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);
    const aside = root.querySelectorAll("p")[1];
    const after = aside.nextSibling;
    aside.remove();

    findInPage.find("Intro");
    root.insertBefore(aside, after);

    expect(aside.querySelector("mark.user-highlight")?.textContent).toBe("dogs");
  });

  test("does not put a mark between blocks", () => {
    const root = page(`<ul data-source-range="1:1-2:5">
<li data-source-range="1:1-1:5">one</li>
<li data-source-range="2:1-2:5">two</li>
</ul>`);

    show("/doc.md", [highlight("hl_1", anchor("one\ntwo", "", ""))]);

    expect(marks(root).map(([, , text]) => text)).toEqual(["one", "two"]);
    expect(root.querySelector("ul > mark")).toBeNull();
  });
});

describe("with search", () => {
  test("a search across a highlight's edge still finds its words", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs sleep", "Lazy ", "."))]);

    let count = -1;
    findInPage.setup((data) => {
      count = data.count;
    });
    findInPage.find("Lazy dogs");

    expect(count).toBe(1);
    expect(marks(root).map(([, , text]) => text)).toEqual(["dogs", " sleep"]);
    // The highlight sits inside the search's mark, drawn over it.
    expect(root.querySelector(".search-highlight mark.user-highlight")?.textContent).toBe("dogs");

    findInPage.clear();
    expect(root.querySelector(".search-highlight")).toBeNull();
    expect(marks(root).map(([, , text]) => text)).toEqual(["dogs sleep"]);
  });
});

describe("selection", () => {
  test("describes what is selected in the page", () => {
    const root = page(DOC);
    const text = root.querySelectorAll("p")[1].firstChild as Text;
    const range = document.createRange();
    range.setStart(text, 5);
    range.setEnd(text, 9);
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);

    show("/doc.md", []);
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);

    expect(describeSelection()).toMatchObject({
      doc: "/doc.md",
      anchor: { exact: "dogs", line: 7 },
    });
  });

  test("falls back to a range kept by the menu when nothing is selected", () => {
    const root = page(DOC);
    const text = root.querySelectorAll("p")[1].firstChild as Text;
    const range = document.createRange();
    range.setStart(text, 0);
    range.setEnd(text, 4);
    window.getSelection()?.removeAllRanges();

    expect(describeSelection()).toBeNull();
    expect(describeSelection(range)?.anchor.exact).toBe("Lazy");
  });

  test("finds the highlights a selection touches, or the one clicked on", () => {
    const root = page(DOC);
    show("/doc.md", [
      highlight("hl_1", anchor("quick", "The ", " brown")),
      highlight("hl_2", anchor("dogs", "Lazy ", " sleep.")),
    ]);
    const range = document.createRange();
    range.selectNodeContents(root.querySelectorAll("p")[0]);

    expect(idsIn(range)).toEqual(["hl_1"]);
    expect(idsIn(null, root.querySelector('[data-highlight-id="hl_2"]'))).toEqual(["hl_2"]);
    expect(idsIn(null, root.querySelector("h1"))).toEqual([]);

    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);
    expect(idsAtSelection()).toEqual(["hl_1"]);

    show("/doc.md", [
      highlight("hl_3", anchor("Lazy dogs", "", " sleep.")),
      highlight("hl_4", anchor("dogs", "Lazy ", " sleep.")),
    ]);
    const inner = root.querySelector('[data-highlight-id="hl_3"] > [data-highlight-id="hl_4"]');
    expect(idsIn(null, inner)).toEqual(["hl_4", "hl_3"]);
  });
});

describe("contents", () => {
  test("a highlight colours the heading it is under", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."), "orange")]);

    expect(hitsByHeading(root)).toEqual(new Map([["more", ["var(--mark-orange)"]]]));
  });
});
