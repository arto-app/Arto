import { describe, test, expect, beforeEach, afterEach, vi } from "vitest";

import * as findInPage from "./find-in-page";
import { hitsByHeading } from "./reading-position";
import type { TextAnchor } from "./text-anchor";
import {
  type HighlightDef,
  type Opened,
  type Report,
  _reset,
  describeSelection,
  idsAtSelection,
  idsIn,
  lift,
  rectOf,
  reveal,
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

describe("notes", () => {
  test("only the last piece of a highlight with a note carries the note's glyph", () => {
    const root = page(DOC);

    show("/doc.md", [
      { ...highlight("hl_1", anchor("quick brown fox", "The ", " jumps.")), note: "why" },
      highlight("hl_2", anchor("dogs", "Lazy ", " sleep.")),
    ]);

    const noted = Array.from(root.querySelectorAll<HTMLElement>("mark[data-note]"));
    expect(noted.map((mark) => [mark.dataset.highlightId, mark.textContent])).toEqual([
      ["hl_1", " fox"],
    ]);
    // The glyph is drawn, not written: the words are what they were.
    expect(root.querySelector("p")?.textContent).toBe("The quick brown fox jumps.");
  });

  test("hovering a highlight with a note shows the note until the pointer leaves", () => {
    const root = page(DOC);
    show("/doc.md", [
      { ...highlight("hl_1", anchor("dogs", "Lazy ", " sleep.")), note: "first\nsecond" },
    ]);
    const mark = root.querySelector('[data-highlight-id="hl_1"]') as HTMLElement;

    mark.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: 5, clientY: 5 }));
    const tip = document.querySelector<HTMLElement>("body > .user-highlight-tip");
    expect(tip?.hidden).toBe(false);
    expect(tip?.textContent).toBe("first\nsecond");

    root.querySelector("h1")?.dispatchEvent(new MouseEvent("mousemove", { bubbles: true }));
    expect(tip?.hidden).toBe(true);
  });

  test("scrolling hides the note", () => {
    const root = page(DOC);
    show("/doc.md", [{ ...highlight("hl_1", anchor("dogs", "Lazy ", " sleep.")), note: "n" }]);
    const mark = root.querySelector('[data-highlight-id="hl_1"]') as HTMLElement;

    mark.dispatchEvent(new MouseEvent("mousemove", { bubbles: true }));
    document.dispatchEvent(new Event("scroll"));

    expect(document.querySelector<HTMLElement>("body > .user-highlight-tip")?.hidden).toBe(true);
  });

  test("a highlight without a note says nothing on hover", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);
    const mark = root.querySelector('[data-highlight-id="hl_1"]') as HTMLElement;

    mark.dispatchEvent(new MouseEvent("mousemove", { bubbles: true }));

    const tip = document.querySelector<HTMLElement>("body > .user-highlight-tip");
    expect(tip === null || tip.hidden).toBe(true);
  });
});

describe("opening a highlight", () => {
  let opened: Opened[] = [];

  beforeEach(() => {
    opened = [];
    setup(
      (report) => reports.push(report),
      (open) => opened.push(open),
    );
    window.getSelection()?.removeAllRanges();
  });

  function click(el: Element | null): void {
    el?.dispatchEvent(new MouseEvent("click", { bubbles: true, button: 0 }));
  }

  test("a click on a highlight with nothing selected asks for it, and says where it is and on which document", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("quick brown fox", "The ", " jumps."))]);
    const pieces = root.querySelectorAll<HTMLElement>('[data-highlight-id="hl_1"]');
    const at = (left: number, top: number, right: number, bottom: number): DOMRect =>
      ({
        left,
        top,
        right,
        bottom,
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
      }) as DOMRect;
    pieces[0].getBoundingClientRect = () => at(40, 20, 90, 36);
    pieces[1].getBoundingClientRect = () => at(90, 20, 130, 36);
    pieces[2].getBoundingClientRect = () => at(10, 40, 30, 56);

    click(pieces[1]);

    expect(opened).toEqual([
      { doc: "/doc.md", id: "hl_1", rect: { left: 10, top: 20, right: 130, bottom: 56 } },
    ]);
  });

  test("a click that ends a selection leaves the selection alone", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);
    const range = document.createRange();
    range.selectNodeContents(root.querySelectorAll("p")[1]);
    window.getSelection()?.addRange(range);

    click(root.querySelector('[data-highlight-id="hl_1"]'));

    expect(opened).toEqual([]);
  });

  test("of nested highlights, the innermost clicked is the one opened", () => {
    const root = page(DOC);
    show("/doc.md", [
      highlight("hl_1", anchor("Lazy dogs", "", " sleep.")),
      highlight("hl_2", anchor("dogs", "Lazy ", " sleep.")),
    ]);

    click(root.querySelector('[data-highlight-id="hl_2"]'));

    expect(opened.map(({ id }) => id)).toEqual(["hl_2"]);
  });

  test("a click on a link in a highlight follows the link", () => {
    const root = page(`<p data-source-range="1:1-1:20">See <a href="#x">the docs</a> now.</p>`);
    show("/doc.md", [highlight("hl_1", anchor("the docs", "See ", " now."))]);

    click(root.querySelector('[data-highlight-id="hl_1"]'));

    expect(opened).toEqual([]);
  });

  test("a selection inside one highlight names that highlight", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("Lazy dogs sleep", "", "."))]);
    const range = document.createRange();
    const inner = root.querySelector('[data-highlight-id="hl_1"]')?.firstChild as Text;
    range.setStart(inner, 5);
    range.setEnd(inner, 9);
    window.getSelection()?.addRange(range);

    expect(describeSelection()).toMatchObject({ anchor: { exact: "dogs" }, within: "hl_1" });

    const outside = document.createRange();
    outside.selectNodeContents(root.querySelector("h1") as Element);
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(outside);
    expect(describeSelection()).toMatchObject({ anchor: { exact: "Intro" }, within: null });
  });

  test("a highlight is found where it is, or not at all", () => {
    page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);

    expect(rectOf("hl_1")).toEqual({ left: 0, top: 0, right: 0, bottom: 0 });
    expect(rectOf("hl_404")).toBeNull();
  });

  test("a highlight running past the window is measured by the part in it", () => {
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("quick brown fox", "The ", " jumps."))]);
    const pieces = root.querySelectorAll<HTMLElement>('[data-highlight-id="hl_1"]');
    const at =
      (top: number): (() => DOMRect) =>
      () =>
        ({ left: 10, top, right: 200, bottom: top + 16 }) as DOMRect;
    pieces[0].getBoundingClientRect = at(-400);
    pieces[1].getBoundingClientRect = at(100);
    pieces[2].getBoundingClientRect = at(window.innerHeight + 300);

    expect(rectOf("hl_1")).toEqual({ left: 10, top: 100, right: 200, bottom: 116 });
  });
});

describe("reveal", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  test("says where a highlight is once it has stopped moving", async () => {
    vi.useFakeTimers();
    const root = page(DOC);
    show("/doc.md", [highlight("hl_1", anchor("dogs", "Lazy ", " sleep."))]);
    const mark = root.querySelector('[data-highlight-id="hl_1"]') as HTMLElement;
    const tops = [400, 250, 120, 100, 100, 100, 100];
    mark.getBoundingClientRect = () => {
      const top = tops.length > 1 ? (tops.shift() ?? 0) : tops[0];
      return { left: 5, top, right: 45, bottom: top + 16 } as DOMRect;
    };

    const revealed = reveal("hl_1");
    await vi.advanceTimersByTimeAsync(2000);

    expect(await revealed).toEqual({ left: 5, top: 100, right: 45, bottom: 116 });
  });

  test("says nothing of a highlight not on the page", async () => {
    page(DOC);
    show("/doc.md", []);

    expect(await reveal("hl_1")).toBeNull();
  });
});
