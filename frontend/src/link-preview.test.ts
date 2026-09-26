import { describe, test, expect, beforeEach, afterEach, vi } from "vitest";
import {
  HOVER_DELAY_MS,
  LEAVE_GRACE_MS,
  _internal,
  footnoteOf,
  hideIfDetached,
  leadOf,
  onRequest,
  resolve,
  sectionOf,
  setup,
  showAtCursor,
} from "./link-preview";

const cursor = vi.hoisted(() => ({ element: null as Element | null }));
vi.mock("./content-cursor", () => ({ getCurrentElement: () => cursor.element }));

function page(html: string): HTMLElement {
  document.body.innerHTML = `<div class="content"><article class="markdown-body">${html}</article></div>`;
  return document.querySelector(".markdown-body") as HTMLElement;
}

function replaceDocument(): void {
  (document.querySelector(".markdown-body") as HTMLElement).innerHTML = "<p>another document</p>";
}

function fragment(html: string): DocumentFragment {
  const template = document.createElement("template");
  template.innerHTML = html;
  return template.content;
}

function texts(nodes: Node[]): string[] {
  return nodes.map((node) => (node.textContent ?? "").trim());
}

function hover(el: Element): void {
  el.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
}

function shown(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".link-preview.is-visible");
}

describe("sectionOf", () => {
  test("runs to the next heading of the same level or above, taking deeper ones along", () => {
    const root = fragment(`
      <h2 id="a">A</h2><p>one</p><h3>A.1</h3><p>two</p>
      <h2>B</h2><p>three</p>`);
    const excerpt = sectionOf(root.querySelector("h2") as Element, 12);
    expect(texts(excerpt.nodes)).toEqual(["one", "A.1", "two"]);
    expect(excerpt.truncated).toBe(false);
  });

  test("stops at a heading above its own level", () => {
    const root = fragment(`<h2>A</h2><p>one</p><h1>Top</h1><p>two</p>`);
    expect(texts(sectionOf(root.querySelector("h2") as Element, 12).nodes)).toEqual(["one"]);
  });

  test("is cut at the block limit and says so", () => {
    const root = fragment(`<h2>A</h2><p>1</p><p>2</p><p>3</p>`);
    const excerpt = sectionOf(root.querySelector("h2") as Element, 2);
    expect(texts(excerpt.nodes)).toEqual(["1", "2"]);
    expect(excerpt.truncated).toBe(true);
  });

  test("leaves out the ids and source ranges of what it copies", () => {
    const root = fragment(
      `<h2>A</h2><p id="x" data-source-range="1:1-1:3"><span id="y">one</span></p>`,
    );
    const [node] = sectionOf(root.querySelector("h2") as Element, 12).nodes as Element[];
    expect(node.hasAttribute("id")).toBe(false);
    expect(node.hasAttribute("data-source-range")).toBe(false);
    expect(node.querySelector("[id]")).toBeNull();
  });
});

describe("leadOf", () => {
  test("takes what precedes the first heading and that heading's section", () => {
    const root = fragment(`
      <details class="frontmatter"><summary>meta</summary></details>
      <p>intro</p><h1>Title</h1><p>body</p><h2>Sub</h2><p>more</p><h1>Next</h1><p>not this</p>`);
    const excerpt = leadOf(root, 12);
    expect(texts(excerpt.nodes)).toEqual(["intro", "Title", "body", "Sub", "more"]);
    expect(excerpt.truncated).toBe(false);
  });

  test("does not run into the footnotes", () => {
    const root = fragment(`<p>body</p><section class="footnotes"><ol><li>note</li></ol></section>`);
    expect(texts(leadOf(root, 12).nodes)).toEqual(["body"]);
  });

  test("is cut at the block limit", () => {
    const root = fragment(`<p>1</p><p>2</p><p>3</p>`);
    const excerpt = leadOf(root, 2);
    expect(texts(excerpt.nodes)).toEqual(["1", "2"]);
    expect(excerpt.truncated).toBe(true);
  });
});

describe("footnoteOf", () => {
  test("is the definition without the links back to its references", () => {
    const root = fragment(`
      <li id="fn-1" data-source-range="3:1-3:9"><p data-source-range="3:7-3:9">Note</p>
      <a href="#fnref-1" aria-label="Back to reference 1">↩</a></li>`);
    const excerpt = footnoteOf(root.querySelector("li") as Element);
    expect(texts(excerpt.nodes)).toEqual(["Note"]);
    const [paragraph] = excerpt.nodes as Element[];
    expect(paragraph.hasAttribute("data-source-range")).toBe(false);
  });
});

describe("hover", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    _internal.reset();
    setup();
  });

  afterEach(() => {
    _internal.reset();
    vi.useRealTimers();
  });

  const withFootnote = `
    <p>Text<sup><a href="#fn-1" id="fnref-1">1</a></sup> and <a href="#later">later</a>.</p>
    <h2 id="later">Later</h2><p>what later says</p>
    <section class="footnotes"><ol><li id="fn-1"><p>The note.</p>
    <a href="#fnref-1" aria-label="Back to reference 1">↩</a></li></ol></section>`;

  test("a footnote reference shows the note after a rest", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fn-1"]') as Element);

    vi.advanceTimersByTime(HOVER_DELAY_MS - 1);
    expect(shown()).toBeNull();
    vi.advanceTimersByTime(1);

    const popover = shown();
    expect(popover?.getAttribute("role")).toBe("tooltip");
    expect(popover?.querySelector(".link-preview-body")?.textContent?.trim()).toBe("The note.");
    expect(popover?.textContent).not.toContain("↩");
    expect(popover?.querySelector("[id]")).toBeNull();
    expect(document.querySelector('a[href="#fn-1"]')?.getAttribute("aria-describedby")).toBe(
      popover?.id,
    );
  });

  test("a link to a heading on the page shows that heading's section", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#later"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);

    expect(shown()?.querySelector(".link-preview-title")?.textContent).toBe("Later");
    expect(shown()?.querySelector(".link-preview-body")?.textContent?.trim()).toBe(
      "what later says",
    );
  });

  test("a link that names neither a heading nor a footnote shows nothing", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fnref-1"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    expect(shown()).toBeNull();
  });

  test("moving onto the popover keeps it, moving elsewhere lets it go", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fn-1"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);

    hover(document.querySelector(".markdown-body > p") as Element);
    vi.advanceTimersByTime(LEAVE_GRACE_MS - 1);
    hover(shown()?.querySelector(".link-preview-body") as Element);
    vi.advanceTimersByTime(LEAVE_GRACE_MS);
    expect(shown()).not.toBeNull();

    hover(document.querySelector("h2") as Element);
    vi.advanceTimersByTime(LEAVE_GRACE_MS);
    expect(shown()).toBeNull();
    expect(document.querySelector('a[href="#fn-1"]')?.hasAttribute("aria-describedby")).toBe(false);
  });

  test("leaving before the rest is over shows nothing", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fn-1"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS / 2);
    hover(document.querySelector("h2") as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    expect(shown()).toBeNull();
  });

  test("a scroll of the page or Escape puts it away", () => {
    page(withFootnote);
    const link = document.querySelector('a[href="#fn-1"]') as Element;
    hover(link);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    document.querySelector(".content")?.dispatchEvent(new Event("scroll"));
    expect(shown()).toBeNull();

    hover(document.querySelector("h2") as Element);
    hover(link);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    expect(shown()).not.toBeNull();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(shown()).toBeNull();
  });

  test("a preview whose link the page no longer shows is put away", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fn-1"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    expect(shown()).not.toBeNull();

    replaceDocument();
    hideIfDetached();
    expect(shown()).toBeNull();
  });

  test("the pointer leaving the window takes a waiting or shown preview with it", () => {
    page(withFootnote);
    const link = document.querySelector('a[href="#fn-1"]') as Element;
    hover(link);
    link.dispatchEvent(new MouseEvent("mouseout", { bubbles: true, relatedTarget: null }));
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    expect(shown()).toBeNull();

    hover(document.querySelector("h2") as Element);
    hover(link);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    expect(shown()).not.toBeNull();
    link.dispatchEvent(new MouseEvent("mouseout", { bubbles: true, relatedTarget: null }));
    vi.advanceTimersByTime(LEAVE_GRACE_MS);
    expect(shown()).toBeNull();
  });

  test("the document hearing the pointer leave takes the preview with it", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fn-1"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    document.dispatchEvent(new MouseEvent("mouseleave"));
    vi.advanceTimersByTime(LEAVE_GRACE_MS);
    expect(shown()).toBeNull();
  });

  test("a scroll inside the popover leaves it up", () => {
    page(withFootnote);
    hover(document.querySelector('a[href="#fn-1"]') as Element);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    shown()?.dispatchEvent(new Event("scroll"));
    expect(shown()).not.toBeNull();
  });

  describe("another document", () => {
    const docLink = (link: string, extra = "") =>
      `<p><span class="md-link${extra}" data-md-link="${link}">other</span></p>`;

    test("is asked for after the rest and shown when the answer comes", () => {
      page(docLink("./other.md"));
      const requests: { seq: number; link: string }[] = [];
      onRequest((request) => requests.push(request));

      hover(document.querySelector(".md-link") as Element);
      vi.advanceTimersByTime(HOVER_DELAY_MS - 1);
      expect(requests).toHaveLength(0);
      vi.advanceTimersByTime(1);
      expect(requests).toHaveLength(1);
      expect(requests[0].link).toBe("./other.md");
      expect(shown()).toBeNull();

      resolve(requests[0].seq, '<h1 id="t">Other</h1><p>body</p>');

      expect(shown()?.querySelector(".link-preview-title")?.textContent).toBe("other.md");
      expect(shown()?.querySelector(".link-preview-body")?.textContent).toContain("body");
      expect(shown()?.querySelector("[id]")).toBeNull();
    });

    test("passing over a link without resting asks for nothing", () => {
      page(docLink("./a.md") + docLink("./b.md"));
      const requests: unknown[] = [];
      onRequest((request) => requests.push(request));
      const [a, b] = Array.from(document.querySelectorAll(".md-link"));
      hover(a);
      vi.advanceTimersByTime(HOVER_DELAY_MS / 2);
      hover(b);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      expect(requests).toEqual([expect.objectContaining({ link: "./b.md" })]);
    });

    test("an answer for a link the page no longer shows is dropped", () => {
      page(docLink("./other.md"));
      let seq = -1;
      onRequest((request) => (seq = request.seq));
      hover(document.querySelector(".md-link") as Element);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      replaceDocument();
      resolve(seq, "<p>body</p>");
      expect(shown()).toBeNull();
    });

    test("with a fragment shows the section it names", () => {
      page(docLink("./other.md#sub%20part"));
      let seq = -1;
      onRequest((request) => (seq = request.seq));
      hover(document.querySelector(".md-link") as Element);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      resolve(seq, `<p>intro</p><h2 id="sub part">Sub</h2><p>inside</p><h2>Next</h2>`);

      expect(shown()?.querySelector(".link-preview-title")?.textContent).toBe("other.md › Sub");
      expect(shown()?.querySelector(".link-preview-body")?.textContent?.trim()).toBe("inside");
    });

    test("an answer for a link no longer pointed at is dropped", () => {
      page(docLink("./a.md") + docLink("./b.md"));
      const seqs: number[] = [];
      onRequest((request) => seqs.push(request.seq));
      const [a, b] = Array.from(document.querySelectorAll(".md-link"));
      hover(a);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      hover(b);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      resolve(seqs[0], "<p>from a</p>");
      expect(shown()).toBeNull();
      resolve(seqs[1], "<p>from b</p>");
      expect(shown()?.textContent).toContain("from b");
    });

    test("an answer that comes while another link is being rested on is not taken for it", () => {
      page(docLink("./a.md") + docLink("./b.md"));
      const seqs: number[] = [];
      onRequest((request) => seqs.push(request.seq));
      const [a, b] = Array.from(document.querySelectorAll(".md-link"));
      hover(a);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      hover(b);
      resolve(seqs[0], "<p>from a</p>");
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      expect(shown()).toBeNull();
      resolve(seqs[1], "<p>from b</p>");
      expect(shown()?.textContent).toContain("from b");
    });

    test("a document that cannot be read says so", () => {
      page(docLink("./other.md"));
      let seq = -1;
      onRequest((request) => (seq = request.seq));
      hover(document.querySelector(".md-link") as Element);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      resolve(seq, null);
      expect(shown()?.textContent).toContain("No preview available");
    });

    test("a link to a missing or non-Markdown file is not previewed", () => {
      page(docLink("./gone.md", " md-link-missing") + docLink("./a.txt", " md-link-invalid"));
      const requests: unknown[] = [];
      onRequest((request) => requests.push(request));
      for (const link of Array.from(document.querySelectorAll(".md-link"))) hover(link);
      vi.advanceTimersByTime(HOVER_DELAY_MS);
      expect(requests).toHaveLength(0);
      expect(shown()).toBeNull();
    });
  });

  describe("at the keyboard cursor", () => {
    test("previews the link in the block under the cursor at once", () => {
      page(`<p id="block">See<sup><a href="#fn-1">1</a></sup></p>
        <section class="footnotes"><ol><li id="fn-1"><p>The note.</p></li></ol></section>`);
      cursor.element = document.getElementById("block");

      expect(showAtCursor()).toBe(true);
      vi.advanceTimersByTime(0);

      expect(shown()?.textContent).toContain("The note.");
    });

    test("says so when there is no link to preview", () => {
      page(`<p id="block">No link here.</p>`);
      cursor.element = document.getElementById("block");

      expect(showAtCursor()).toBe(false);
      cursor.element = null;
      expect(showAtCursor()).toBe(false);
    });
  });
});
