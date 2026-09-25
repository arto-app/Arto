import { describe, test, expect, beforeEach, afterEach, vi } from "vitest";
import {
  annotate,
  annotateAll,
  beginPage,
  collect,
  fail,
  markPending,
  restore,
  restoreMarks,
  restorePage,
  settle,
  setup,
  showAnswers,
  showBlock,
  showPage,
} from "./lenses";

function page(html: string, generation = "7"): HTMLElement {
  document.body.innerHTML = `<article class="markdown-body" data-render-generation="${generation}">${html}</article>`;
  return document.querySelector(".markdown-body") as HTMLElement;
}

beforeEach(() => {
  restore();
  document.body.innerHTML = "";
});

describe("collect", () => {
  test("offers every block with text of its own, in document order", () => {
    page(`
      <h1 data-source-range="1:1-1:7">Title</h1>
      <p data-source-range="3:1-3:4">Text</p>
      <pre data-source-range="5:1-7:3"><code data-source-range="6:1-6:4">code</code></pre>
      <table data-source-range="9:1-11:9"><tr data-source-range="9:1-9:9">
        <th data-source-range="9:3-9:3">A</th></tr></table>
      <dl data-source-range="13:1-14:10"><dt data-source-range="13:1-13:4">Term</dt>
        <dd data-source-range="14:1-14:10">: Meaning</dd></dl>
      <div class="frontmatter"><table><tr><td>not source</td></tr></table></div>
    `);

    const offered = collect(1, "document");

    expect(offered.generation).toBe(7);
    expect(offered.blocks.map((block) => [block.kind, block.range, block.target])).toEqual([
      ["heading", "1:1-1:7", true],
      ["paragraph", "3:1-3:4", true],
      ["table-cell", "9:3-9:3", true],
      ["definition-term", "13:1-13:4", true],
      ["definition", "14:1-14:10", true],
    ]);
    expect(new Set(offered.blocks.map((block) => block.id)).size).toBe(5);
  });

  test("offers the paragraphs of a loose item instead of the item", () => {
    page(`<ul data-source-range="1:1-3:5">
      <li data-source-range="1:1-1:5"><p data-source-range="1:3-1:5">one</p></li>
      <li data-source-range="3:1-3:5"><p data-source-range="3:3-3:5">two</p></li></ul>`);

    expect(collect(1, "document").blocks.map((block) => [block.kind, block.range])).toEqual([
      ["paragraph", "1:3-1:5"],
      ["paragraph", "3:3-3:5"],
    ]);
  });

  test("ends a tight item where its nested block starts", () => {
    page(`<ul data-source-range="1:1-2:9">
      <li data-source-range="1:1-2:9">parent<ul data-source-range="2:3-2:9">
        <li data-source-range="2:3-2:9">child</li></ul></li></ul>`);

    expect(collect(1, "document").blocks).toMatchObject([
      { kind: "list-item", range: "1:1-2:9", until: "2:3" },
      { kind: "list-item", range: "2:3-2:9", until: undefined },
    ]);
  });

  test("targets only the blocks inside the cursor's block, offering the rest as context", () => {
    const body = page(`
      <p data-source-range="1:1-1:3">one</p>
      <blockquote data-source-range="3:1-4:5"><p data-source-range="3:3-4:5">two</p></blockquote>
      <p data-source-range="6:1-6:5">three</p>
    `);

    const offered = collect(1, "cursor", "", body.querySelector("blockquote"));

    expect(offered.blocks.map((block) => block.target)).toEqual([false, true, false]);
  });

  test("offers nothing without a page", () => {
    expect(collect(1, "document")).toEqual({ generation: null, blocks: [] });
  });
});

describe("annotations", () => {
  test("an answer marks its block, and closing takes the mark away", () => {
    page(`<p data-source-range="1:1-1:5">Hello</p>`);
    const [block] = collect(1, "document").blocks;
    markPending(1, [block.id]);
    const el = document.querySelector("p") as HTMLElement;
    expect(el.classList.contains("lens-pending")).toBe(true);

    annotate(1, block.id, "<p>A greeting.</p>");

    expect(el.classList.contains("lens-pending")).toBe(false);
    expect(el.classList.contains("lens-annotated")).toBe(true);
    expect(el.querySelector(".lens-marker")).not.toBeNull();
    expect(el.firstChild?.textContent).toBe("Hello");

    restore();
    expect(el.outerHTML).toBe(`<p data-source-range="1:1-1:5">Hello</p>`);
  });

  test("a failure is marked, and settling clears what is still pending", () => {
    page(`<p data-source-range="1:1-1:3">one</p><p data-source-range="3:1-3:3">two</p>`);
    const [first, second] = collect(1, "document").blocks;
    markPending(1, [first.id, second.id]);

    fail(1, first.id, "the command failed");
    settle();

    const [one, two] = Array.from(document.querySelectorAll("p"));
    expect(one.classList.contains("lens-failed")).toBe(true);
    expect(two.classList.contains("lens-pending")).toBe(false);
  });

  test("an answer for a closed run is ignored", () => {
    page(`<p data-source-range="1:1-1:3">one</p>`);
    const [closed] = collect(1, "document").blocks;
    restoreMarks(1);

    annotate(1, closed.id, "<p>stale</p>");

    expect(document.querySelector(".lens-marker")).toBeNull();
  });

  test("two runs mark one block with one mark, and closing one keeps the other's", () => {
    page(`<p data-source-range="1:1-1:3">one</p>`);
    const [gloss] = collect(1, "document", "Gloss").blocks;
    const [explain] = collect(2, "document", "Explain").blocks;
    annotate(1, gloss.id, "<p>a gloss</p>");
    annotate(2, explain.id, "<p>an explanation</p>");

    expect(document.querySelectorAll(".lens-marker")).toHaveLength(1);

    restoreMarks(1);
    expect(document.querySelectorAll(".lens-marker")).toHaveLength(1);

    restoreMarks(2);
    expect(document.querySelector("p")?.outerHTML).toBe(`<p data-source-range="1:1-1:3">one</p>`);
  });

  test("an answer about text that has since changed is marked outdated", () => {
    page(`<p data-source-range="1:1-1:3">one</p>`);
    const [block] = collect(1, "document").blocks;
    const el = document.querySelector("p") as HTMLElement;

    annotate(1, block.id, "<p>old</p>", true);
    expect(el.classList.contains("lens-outdated")).toBe(true);
    expect(el.querySelector(".lens-marker.lens-marker-outdated")).not.toBeNull();

    annotate(1, block.id, "<p>new</p>");
    expect(el.classList.contains("lens-outdated")).toBe(false);
    expect(el.querySelector(".lens-marker-outdated")).toBeNull();
  });

  test("answers kept from before are put on the page at once", () => {
    page(`<p data-source-range="1:1-1:3">one</p><p data-source-range="3:1-3:3">two</p>`);
    const [first, second] = collect(1, "document").blocks;

    annotateAll(1, [
      [first.id, "<p>a</p>", false],
      [second.id, "<p>b</p>", true],
    ]);

    const [one, two] = Array.from(document.querySelectorAll("p"));
    expect(one.classList.contains("lens-annotated")).toBe(true);
    expect(one.classList.contains("lens-outdated")).toBe(false);
    expect(two.classList.contains("lens-outdated")).toBe(true);
  });

  test("an empty answer leaves its block unmarked", () => {
    page(`<p data-source-range="1:1-1:3">one</p>`);
    const [block] = collect(1, "document").blocks;
    markPending(1, [block.id]);

    annotate(1, block.id, "  \n");

    expect(document.querySelector("p")?.outerHTML).toBe(`<p data-source-range="1:1-1:3">one</p>`);
  });

  test("a block stays pending while any run still waits on it", () => {
    page(`<p data-source-range="1:1-1:3">one</p>`);
    const [first] = collect(1, "document").blocks;
    const [second] = collect(2, "document").blocks;
    markPending(1, [first.id]);
    markPending(2, [second.id]);
    const el = document.querySelector("p") as HTMLElement;

    annotate(1, first.id, "<p>done</p>");
    expect(el.classList.contains("lens-pending")).toBe(true);

    settle(2);
    expect(el.classList.contains("lens-pending")).toBe(false);
    expect(el.classList.contains("lens-annotated")).toBe(true);
  });

  test("an answer for a block that is gone is ignored", () => {
    page(`<p data-source-range="1:1-1:3">one</p>`);
    const [block] = collect(1, "document").blocks;
    page(`<p data-source-range="1:1-1:3">rebuilt</p>`);

    annotate(1, block.id, "<p>answer</p>");

    expect(document.querySelector(".lens-marker")).toBeNull();
  });

  test("the document's own look-alike markup is left alone", () => {
    page(`
      <div data-lens-id="1:0" class="lens-marker lens-annotated">authored</div>
      <p data-source-range="3:1-3:3">one</p>
    `);
    const [block] = collect(1, "document").blocks;

    annotate(1, block.id, "<p>answer</p>");
    const authored = document.querySelector("div") as HTMLElement;
    expect(authored.querySelector(".lens-marker")).toBeNull();
    expect(document.querySelector("p .lens-marker")).not.toBeNull();

    restore();
    expect(authored.outerHTML).toBe(
      `<div data-lens-id="1:0" class="lens-marker lens-annotated">authored</div>`,
    );
  });
});

describe("page", () => {
  const document_ = `
    <h1 data-source-range="1:1-1:7">Title</h1>
    <p data-source-range="3:1-3:5">First</p>
    <p data-source-range="5:1-5:6">Second</p>
  `;

  function texts(): string[] {
    return Array.from(document.querySelector(".markdown-body")?.children ?? []).map(
      (el) => el.textContent ?? "",
    );
  }

  test("the answer takes the document's places from the top as it arrives", () => {
    page(document_);

    expect(beginPage(1)).toMatchObject({ generation: 7, total: 3 });
    expect(showPage(1, "<h1>題</h1>")).toBe(1);
    expect(texts()).toEqual(["題", "First", "Second"]);

    expect(showPage(1, "<h1>題</h1><p>一つ目</p><p>二つ目</p>")).toBe(3);
    expect(texts()).toEqual(["題", "一つ目", "二つ目"]);
  });

  test("the document's raw HTML keeps its place, since the answer shows it escaped as text", () => {
    page(`
      <p align="center"><img alt="Logo" src="logo.png"></p>
      <h1 data-source-range="3:1-3:7">Title</h1>
      <div align="center">Badges</div>
      <p data-source-range="7:1-7:5">First</p>
    `);

    expect(beginPage(1)).toMatchObject({ total: 2 });
    expect(
      showPage(
        1,
        `&lt;p align="center"&gt;…&lt;/p&gt;<h1>題</h1>&lt;div align="center"&gt;…&lt;/div&gt;<p>一つ目</p>`,
      ),
    ).toBe(2);
    expect(texts()).toEqual(["", "題", "Badges", "一つ目"]);
    expect(document.querySelector(".markdown-body > p > img")).not.toBeNull();
  });

  test("frontmatter and footnotes are places the answer takes, though they name no lines", () => {
    page(`
      <details class="frontmatter"><summary>F</summary></details>
      <p data-source-range="5:1-5:5">First</p>
      <section class="footnotes"><ol><li>Note</li></ol></section>
    `);

    expect(beginPage(1)).toMatchObject({ total: 3 });
    showPage(
      1,
      `<details class="frontmatter"><summary>訳F</summary></details><p>一つ目</p><section class="footnotes"><ol><li>注</li></ol></section>`,
    );
    expect(texts()).toEqual(["訳F", "一つ目", "注"]);
  });

  test("Markdown inside the document's raw HTML stays as written, and the blocks after it are still paired", () => {
    page(`
      <details><summary>More</summary>
        <p data-source-range="3:1-3:6">Inside</p>
        <ul data-source-range="5:1-5:8"><li data-source-range="5:1-5:8">Item</li></ul>
      </details>
      <p data-source-range="9:1-9:5">After</p>
    `);

    expect(beginPage(1)).toMatchObject({ total: 1 });
    expect(
      showPage(
        1,
        `&lt;details&gt;&lt;summary&gt;More&lt;/summary&gt;<p>中</p><ul><li>項目</li></ul>&lt;/details&gt;<p>後</p>`,
      ),
    ).toBe(1);
    expect(texts().map((text) => text.replace(/\s+/g, " ").trim())).toEqual([
      "More Inside Item",
      "後",
    ]);
  });

  test("an answer longer than the document follows it", () => {
    page(`
      <div align="center">Badges</div>
      <p data-source-range="3:1-3:5">First</p>
    `);
    beginPage(1);

    showPage(1, "<p>一つ目</p><p>余り</p>");

    expect(texts()).toEqual(["Badges", "一つ目", "余り"]);
  });

  test("a block already answered stays the same element as more arrives", () => {
    page(document_);
    beginPage(1);
    showPage(1, "<h1>題</h1>");
    const heading = document.querySelector(".markdown-body > h1");

    showPage(1, "<h1>題</h1><p>一つ目</p>");

    expect(document.querySelector(".markdown-body > h1")).toBe(heading);
  });

  test("an answered block stands for the lines of the block it replaced", () => {
    page(document_);
    beginPage(1);
    showPage(1, "<h1>題</h1><p>一つ目</p>");

    const answered = document.querySelector(".markdown-body > p") as HTMLElement;
    expect(answered.classList.contains("lens-answered")).toBe(true);
    expect(answered.getAttribute("data-source-range")).toBe("3:1-3:5");
  });

  test("taking the page lens off brings the document as written back", () => {
    page(document_);
    beginPage(1);
    showPage(1, "<h1>題</h1><p>一つ目</p>");
    expect(texts()).toEqual(["題", "一つ目", "Second"]);

    restorePage();
    expect(texts()).toEqual(["Title", "First", "Second"]);
  });

  test("a document rebuilt under the lens is left as it was rebuilt", () => {
    page(document_);
    beginPage(1);
    showPage(1, "<h1>題</h1>");

    const root = document.querySelector(".markdown-body") as HTMLElement;
    root.innerHTML = `<p data-source-range="1:1-1:6">Edited</p>`;
    expect(showPage(1, "<h1>題</h1><p>一つ目</p>")).toBe(0);
    restore();

    expect(texts()).toEqual(["Edited"]);
  });

  test("the page names each block and whether it is prose to hand over", () => {
    page(`
      <h1 data-source-range="1:1-1:7">Title</h1>
      <pre data-source-range="3:1-5:3"><code>x</code></pre>
      <div class="preprocessed-math-display" data-source-range="7:1-9:2">x</div>
      <hr data-source-range="11:1-11:3">
      <details class="frontmatter"><summary>F</summary></details>
    `);

    expect(beginPage(1).blocks).toEqual([
      { range: "1:1-1:7", translatable: true },
      { range: "3:1-5:3", translatable: false },
      { range: "7:1-9:2", translatable: false },
      { range: "11:1-11:3", translatable: false },
      { range: null, translatable: false },
    ]);
  });

  test("a block answered on its own takes its own place", () => {
    page(document_);
    beginPage(1);

    expect(showBlock(1, 1, "<p>一つ目</p>")).toBe(1);
    expect(texts()).toEqual(["Title", "一つ目", "Second"]);
    expect(showBlock(1, 0, "<h1>題</h1>")).toBe(2);
    expect(texts()).toEqual(["題", "一つ目", "Second"]);

    const answered = document.querySelector(".markdown-body > p") as HTMLElement;
    expect(answered.getAttribute("data-source-range")).toBe("3:1-3:5");
    restore();
    expect(texts()).toEqual(["Title", "First", "Second"]);
  });

  test("a block answered with two takes its place with both", () => {
    page(document_);
    beginPage(1);

    showBlock(1, 1, "<p>一つ目。</p><p>続き。</p>");

    expect(texts()).toEqual(["Title", "一つ目。", "続き。", "Second"]);
  });

  test("a block answered with nothing keeps the document's own", () => {
    page(document_);
    beginPage(1);

    showBlock(1, 1, "<p>一つ目</p>");
    showBlock(1, 1, "  ");

    expect(texts()).toEqual(["Title", "First", "Second"]);
  });

  test("a lens that marks blocks leaves the page's answer standing", () => {
    page(document_);
    beginPage(1);
    showPage(1, "<h1>題</h1><p>一つ目</p><p>二つ目</p>");

    const [block] = collect(2, "document").blocks;
    annotate(2, block.id, "<p>note</p>");
    restoreMarks();

    expect(texts()).toEqual(["題", "一つ目", "二つ目"]);
  });

  test("an answer for a block the page's answer stands in for is kept for its return", () => {
    page(document_);
    const [block] = collect(2, "document").blocks;
    beginPage(1);
    showPage(1, "<h1>題</h1><p>一つ目</p><p>二つ目</p>");

    annotate(2, block.id, "<p>note</p>");
    restorePage();

    expect(document.querySelectorAll(".lens-annotated")).toHaveLength(1);
  });

  test("a block the page's answer stands in for keeps its notes on the answer", () => {
    page(document_);
    const [, first] = collect(2, "document", "Gloss").blocks;
    annotate(2, first.id, "<p>a gloss</p>");
    beginPage(1);

    showPage(1, "<h1>題</h1><p>一つ目</p><p>二つ目</p>");

    const answered = Array.from(document.querySelectorAll(".markdown-body > *"));
    expect(answered[1].querySelector(".lens-marker")).not.toBeNull();
    expect(answered[0].querySelector(".lens-marker")).toBeNull();
    expect(answered[2].querySelector(".lens-marker")).toBeNull();
  });

  test("a note that arrives while the page shows its answer is marked on the answer", () => {
    page(document_);
    const [, , second] = collect(2, "document", "Gloss").blocks;
    beginPage(1);
    showPage(1, "<h1>題</h1><p>一つ目</p><p>二つ目</p>");

    annotate(2, second.id, "<p>late</p>", true);

    const answered = Array.from(document.querySelectorAll(".markdown-body > *"));
    expect(answered[2].querySelector(".lens-marker.lens-marker-outdated")).not.toBeNull();
  });

  test("a lens opened over a translation looks at the document's own blocks", () => {
    page(document_);
    beginPage(1);
    showPage(1, "<h1>題</h1><p>一つ目</p><p>二つ目</p>");

    const blocks = collect(2, "document", "Gloss").blocks;
    expect(blocks.map((block) => block.range)).toEqual(["1:1-1:7", "3:1-3:5", "5:1-5:6"]);
    annotate(2, blocks[1].id, "<p>a gloss</p>");
    expect(
      Array.from(document.querySelectorAll(".markdown-body > *"))[1].querySelector(".lens-marker"),
    ).not.toBeNull();

    restorePage();
    expect(
      document.querySelectorAll(".markdown-body > p")[0].querySelector(".lens-marker"),
    ).not.toBeNull();
  });

  test("closing the page leaves another lens's marks standing", () => {
    page(document_);
    const [block] = collect(2, "document").blocks;
    annotate(2, block.id, "<p>note</p>");
    beginPage(1);
    showPage(1, "<h1>題</h1>");

    restorePage();

    expect(texts()).toEqual(["Title", "First", "Second"]);
    expect(document.querySelectorAll(".lens-annotated")).toHaveLength(1);
  });

  test("a block answered from an older text is marked outdated until answered again", () => {
    page(document_);
    beginPage(1);

    showBlock(1, 1, "<p>古い</p>", true);
    const stale = document.querySelector(".markdown-body > .lens-answered") as HTMLElement;
    expect(stale.classList.contains("lens-outdated")).toBe(true);

    showBlock(1, 1, "<p>新しい</p>");
    const fresh = document.querySelector(".markdown-body > .lens-answered") as HTMLElement;
    expect(fresh.classList.contains("lens-outdated")).toBe(false);
  });

  test("several blocks' answers take their places in one call", () => {
    page(document_);
    beginPage(1);

    expect(
      showAnswers(1, [
        ["0", "<h1>題</h1>", false],
        ["2", "<p>二つ目</p>", true],
      ]),
    ).toBe(2);

    expect(texts()).toEqual(["題", "First", "二つ目"]);
    expect(document.querySelectorAll(".markdown-body > .lens-outdated")).toHaveLength(1);
  });

  test("an answer for an earlier run changes nothing", () => {
    page(document_);
    beginPage(1);
    beginPage(2);

    expect(showPage(1, "<h1>題</h1>")).toBe(0);
    expect(texts()).toEqual(["Title", "First", "Second"]);
  });
});

describe("hover", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setup();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function hover(el: Element, altKey = false): HTMLElement | null {
    el.dispatchEvent(new MouseEvent("mouseover", { bubbles: true, altKey }));
    vi.advanceTimersByTime(1000);
    return document.querySelector(".lens-popover.is-visible");
  }

  function key(type: "keydown" | "keyup", altKey: boolean): HTMLElement | null {
    document.dispatchEvent(new KeyboardEvent(type, { key: "Alt", altKey }));
    vi.advanceTimersByTime(1000);
    return document.querySelector(".lens-popover.is-visible");
  }

  function answeredPage(): Element {
    page(`<p data-source-range="1:1-1:5">First</p>`);
    beginPage(1);
    showPage(1, "<p>一つ目</p>");
    return document.querySelector(".markdown-body > p") as Element;
  }

  test("an answered block keeps its original to itself under a bare pointer", () => {
    expect(hover(answeredPage())).toBeNull();
  });

  test("an answered block under the pointer offers its original in the margin", () => {
    hover(answeredPage());
    const handle = document.querySelector(".lens-original-handle.is-visible");
    expect(handle).not.toBeNull();

    const popover = hover(handle as Element);

    expect(popover?.querySelector(".lens-popover-caption")?.textContent).toBe("Original");
    expect(popover?.querySelector(".markdown-body")?.textContent).toBe("First");
  });

  test("the margin offer goes away with the pointer, and with the page lens", () => {
    const block = answeredPage();
    hover(block);

    hover(document.body);
    expect(document.querySelector(".lens-original-handle.is-visible")).toBeNull();

    hover(block);
    restorePage();
    expect(document.querySelector(".lens-original-handle.is-visible")).toBeNull();
  });

  test("an answered block shows its original under a caption while Option is held", () => {
    const popover = hover(answeredPage(), true);

    expect(popover?.querySelector(".lens-popover-caption")?.textContent).toBe("Original");
    expect(popover?.querySelector(".markdown-body")?.textContent).toBe("First");
  });

  test("pressing Option over an answered block shows its original, and letting go hides it", () => {
    hover(answeredPage());

    expect(key("keydown", true)?.textContent).toContain("First");
    expect(key("keyup", false)).toBeNull();
  });

  test("a mark shows every open lens's answer, each under its lens's name", () => {
    page(`<p data-source-range="1:1-1:5">First</p>`);
    const [gloss] = collect(1, "document", "Gloss").blocks;
    const [explain] = collect(2, "document", "Explain").blocks;
    annotate(1, gloss.id, "<p>a gloss</p>");
    annotate(2, explain.id, "<p>an explanation</p>");

    const popover = hover(document.querySelector(".lens-marker") as Element);

    const captions = Array.from(popover?.querySelectorAll(".lens-popover-caption") ?? []);
    expect(captions.map((caption) => caption.textContent)).toEqual(["Gloss", "Explain"]);
    expect(popover?.querySelectorAll(":scope > .lens-popover-section")).toHaveLength(2);
    expect(popover?.textContent).toContain("an explanation");
  });

  test("a note on a block a translation stands in for shows from the translation's mark", () => {
    page(`<p data-source-range="1:1-1:5">First</p>`);
    const [block] = collect(2, "document", "Gloss").blocks;
    annotate(2, block.id, "<p>a gloss</p>");
    beginPage(1);
    showPage(1, "<p>一つ目</p>");

    const popover = hover(document.querySelector(".lens-answered .lens-marker") as Element);

    expect(popover?.querySelector(".lens-popover-caption")?.textContent).toBe("Gloss");
    expect(popover?.textContent).toContain("a gloss");
  });

  test("an outdated answer is badged as such beside its lens's name", () => {
    page(`<p data-source-range="1:1-1:5">First</p>`);
    const [block] = collect(1, "document", "Gloss").blocks;
    annotate(1, block.id, "<p>a gloss</p>", true);

    const popover = hover(document.querySelector(".lens-marker") as Element);

    expect(popover?.querySelector(".lens-popover-caption")?.textContent).toBe("Gloss");
    expect(popover?.querySelector(".lens-popover-badge.is-warning")?.textContent).toBe("Outdated");
  });

  test("a popover is drawn at the zoom the document is drawn at", () => {
    document.body.innerHTML = `<div style="zoom: 1.5;"><article class="markdown-body" data-render-generation="7"><p data-source-range="1:1-1:5">First</p></article></div>`;
    const [block] = collect(1, "document").blocks;
    annotate(1, block.id, "<p>a gloss</p>");

    const popover = hover(document.querySelector(".lens-marker") as Element);

    expect(popover?.style.zoom).toBe("1.5");
  });

  test("a failed block shows why under its lens's name, badged as failed", () => {
    page(`<p data-source-range="1:1-1:5">First</p>`);
    const [block] = collect(1, "document", "Gloss").blocks;
    fail(1, block.id, "timed out");

    const popover = hover(document.querySelector(".markdown-body > p") as Element);

    expect(popover?.querySelector(".lens-popover-caption")?.textContent).toBe("Gloss");
    expect(popover?.querySelector(".lens-popover-badge.is-danger")?.textContent).toBe("Failed");
    expect(popover?.querySelector(".markdown-body")?.textContent).toBe("timed out");
  });
});
