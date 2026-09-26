import { describe, test, expect, beforeEach } from "vitest";

import { buildIndex, describe as describeRange, resolve } from "./text-anchor";

function page(html: string): HTMLElement {
  document.body.innerHTML = `<article class="markdown-body">${html}</article>`;
  return document.querySelector(".markdown-body") as HTMLElement;
}

/** A range over the first occurrence of `text` inside `el`'s only text node. */
function rangeOver(el: Element, text: string): Range {
  const node = el.firstChild as Text;
  const at = (node.textContent ?? "").indexOf(text);
  const range = document.createRange();
  range.setStart(node, at);
  range.setEnd(node, at + text.length);
  return range;
}

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("buildIndex", () => {
  test("reads the page's text in order, leaving out code blocks, diagrams and maths", () => {
    const root = page(
      `<p data-source-range="1:1-1:5">one</p>` +
        `<pre><code>code</code></pre>` +
        `<div class="mermaid">graph</div>` +
        `<p data-source-range="5:1-5:9">two <code>inline</code>` +
        `<span class="preprocessed-math-inline">x^2</span></p>`,
    );

    expect(buildIndex(root).text).toBe("onetwo inline");
  });
});

describe("describe then resolve", () => {
  test("finds a quote within one text node where it was", () => {
    const root = page(`<p data-source-range="3:1-3:30">The quick brown fox jumps.</p>`);
    const index = buildIndex(root);

    const anchor = describeRange(index, rangeOver(root.querySelector("p")!, "brown fox"));

    expect(anchor).toEqual({
      exact: "brown fox",
      prefix: "The quick ",
      suffix: " jumps.",
      start: 10,
      line: 3,
    });
    expect(resolve(index, anchor!)).toEqual({ start: 10, end: 19, line: 3 });
  });

  test("covers a selection across a bold word", () => {
    const root = page(`<p data-source-range="1:1-1:30">keep <strong>this</strong> safe</p>`);
    const p = root.querySelector("p")!;
    const range = document.createRange();
    range.setStart(p.firstChild!, 2); // "ep "
    range.setEnd(p.lastChild!, 3); // " sa"
    const index = buildIndex(root);

    const anchor = describeRange(index, range)!;

    expect(anchor.exact).toBe("ep this sa");
    const found = resolve(index, anchor)!;
    expect(index.text.slice(found.start, found.end)).toBe("ep this sa");
  });

  test("covers a selection across two paragraphs, starting on the first", () => {
    const root = page(
      `<p data-source-range="1:1-1:9">first one</p>\n<p data-source-range="3:1-3:10">second one</p>`,
    );
    const [first, second] = Array.from(root.querySelectorAll("p"));
    const range = document.createRange();
    range.setStart(first.firstChild!, 6);
    range.setEnd(second.firstChild!, 6);
    const index = buildIndex(root);

    const anchor = describeRange(index, range)!;

    expect(anchor.exact).toBe("one\nsecond");
    expect(anchor.line).toBe(1);
    expect(resolve(index, anchor)).toEqual({ start: 6, end: 16, line: 1 });
  });

  test("takes a boundary set on an element to the text around it", () => {
    const root = page(
      `<p data-source-range="1:1-1:9">alpha</p><p data-source-range="2:1-2:9">beta</p>`,
    );
    const range = document.createRange();
    range.setStart(root, 1); // before the second paragraph
    range.setEnd(root, 2); // after it
    const index = buildIndex(root);

    expect(describeRange(index, range)?.exact).toBe("beta");
  });

  test("gives nothing for a selection of only white space or only a code block", () => {
    const root = page(`<p data-source-range="1:1-1:9">a   b</p><pre><code>let x = 1;</code></pre>`);
    const index = buildIndex(root);

    expect(describeRange(index, rangeOver(root.querySelector("p")!, "   "))).toBeNull();
    expect(describeRange(index, rangeOver(root.querySelector("code")!, "x = 1"))).toBeNull();
  });

  test("never cuts a character outside the basic plane in half", () => {
    const text = `\u{1F600}${"a".repeat(31)}target${"b".repeat(31)}\u{1F600}`;
    const root = page(`<p data-source-range="1:1-1:99">${text}</p>`);

    const anchor = describeRange(buildIndex(root), rangeOver(root.querySelector("p")!, "target"))!;

    expect(anchor.prefix).toBe("a".repeat(31));
    expect(anchor.suffix).toBe("b".repeat(31));
  });

  test("trims white space off the ends of a selection", () => {
    const root = page(`<p data-source-range="1:1-1:9">say  hello  there</p>`);
    const index = buildIndex(root);

    expect(describeRange(index, rangeOver(root.querySelector("p")!, "  hello  "))?.exact).toBe(
      "hello",
    );
  });
});

describe("resolve after an edit", () => {
  const anchor = {
    exact: "brown fox",
    prefix: "The quick ",
    suffix: " jumps.",
    start: 10,
    line: 3,
  };

  test("follows the quote when a paragraph is inserted above it", () => {
    const root = page(
      `<p data-source-range="1:1-1:9">A new start.</p>` +
        `<p data-source-range="5:1-5:30">The quick brown fox jumps.</p>`,
    );

    expect(resolve(buildIndex(root), anchor)).toEqual({ start: 22, end: 31, line: 5 });
  });

  test("tells the same words apart by what surrounds them", () => {
    const root = page(
      `<p data-source-range="1:1-1:30">A slow brown fox sleeps.</p>` +
        `<p data-source-range="3:1-3:30">The quick brown fox jumps.</p>`,
    );
    const index = buildIndex(root);

    const found = resolve(index, { ...anchor, start: 0, line: 1 })!;

    expect(found.line).toBe(3);
    expect(index.text.slice(found.start - 10, found.start)).toBe("The quick ");
  });

  test("decides a tie by the line it was on, then by the nearest place", () => {
    const root = page(
      `<p data-source-range="1:1-1:9">x mark y</p>` +
        `<p data-source-range="2:1-2:9">x mark y</p>` +
        `<p data-source-range="3:1-3:9">x mark y</p>`,
    );
    const index = buildIndex(root);
    const tied = { exact: "mark", prefix: "x ", suffix: " y", start: 0, line: 3 };

    expect(resolve(index, tied)?.line).toBe(3);
    expect(resolve(index, { ...tied, line: 99, start: 9 })?.line).toBe(2);
  });

  test("loses a quote whose words were rewritten", () => {
    const root = page(`<p data-source-range="3:1-3:30">The quick brown cat jumps.</p>`);

    expect(resolve(buildIndex(root), anchor)).toBeNull();
  });

  test("loses a common word whose surroundings are all gone", () => {
    const root = page(
      `<p data-source-range="1:1-1:9">zzz brown fox qqq</p>` +
        `<p data-source-range="2:1-2:9">www brown fox vvv</p>`,
    );

    expect(resolve(buildIndex(root), anchor)).toBeNull();
  });

  test("takes the words on their old line only when they occur there once", () => {
    const lost = { ...anchor, prefix: "The quick ", suffix: " jumps." };
    const twice = page(
      `<p data-source-range="3:1-3:40">zz brown fox yy brown fox xx</p>` +
        `<p data-source-range="5:1-5:40">ww brown fox vv</p>`,
    );
    expect(resolve(buildIndex(twice), lost)).toBeNull();

    const once = page(
      `<p data-source-range="3:1-3:40">zz brown fox yy</p>` +
        `<p data-source-range="5:1-5:40">ww brown fox vv</p>`,
    );
    expect(resolve(buildIndex(once), lost)).toEqual({ start: 3, end: 12, line: 3 });
  });

  test("keeps a quote that is the only one of its words, whatever surrounds it", () => {
    const root = page(`<p data-source-range="8:1-8:9">zzz brown fox qqq</p>`);

    expect(resolve(buildIndex(root), anchor)).toEqual({ start: 4, end: 13, line: 8 });
  });
});
