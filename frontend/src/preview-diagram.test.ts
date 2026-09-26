import { describe, test, expect } from "vitest";

import { inertSvg } from "./preview-diagram";

function svg(inner: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" id="mermaid-1">${inner}</svg>`;
}

describe("inertSvg", () => {
  test("keeps a diagram as it was drawn, images and all", () => {
    const result = inertSvg(
      svg(`<g class="node"><image href="https://example.com/p.png"/><text>A</text></g>`),
    );
    expect(result?.querySelector("image")?.getAttribute("href")).toBe("https://example.com/p.png");
    expect(result?.textContent).toContain("A");
  });

  test("drops what would run something", () => {
    const result = inertSvg(
      svg(
        `<text>A</text><script>alert(1)</script><foreignObject><iframe src="x"></iframe>` +
          `<object data="x"></object><embed src="x"></foreignObject>`,
      ),
    );
    expect(result?.querySelector("script, iframe, object, embed")).toBeNull();
    expect(result?.textContent).toContain("A");
  });

  test("drops every event handler", () => {
    const result = inertSvg(svg(`<g onclick="alert(1)" onmouseover="alert(2)"><text>A</text></g>`));
    const names = result?.querySelector("g")?.getAttributeNames() ?? [];
    expect(names.filter((name) => name.startsWith("on"))).toEqual([]);
  });

  test.each([
    ["javascript:alert(1)"],
    [" JavaScript:alert(1)"],
    ["java\tscript:alert(1)"],
    ["vbscript:msgbox(1)"],
    ["data:text/html,<script>alert(1)</script>"],
    // Followed from the page, a path would be read against the document on
    // screen rather than the one the diagram is from.
    ["other.md"],
  ])("drops a link that cannot be followed from here: %s", (href) => {
    const result = inertSvg(svg(`<a href="${href}"><text>go</text></a>`));
    expect(result?.querySelector("a")?.hasAttribute("href")).toBe(false);
    expect(result?.textContent).toContain("go");
  });

  test("keeps a link to a page, a mail address or a place in the diagram", () => {
    const result = inertSvg(
      svg(
        `<a href="http://example.com/"></a><a href="mailto:a@example.com"></a>` +
          `<a href="#node"></a>`,
      ),
    );
    const hrefs = Array.from(result?.querySelectorAll("a") ?? []).map((el) =>
      el.getAttribute("href"),
    );
    expect(hrefs).toEqual(["http://example.com/", "mailto:a@example.com", "#node"]);
  });

  test("is nothing when the markup holds no diagram", () => {
    expect(inertSvg("<p>not a diagram</p>")).toBeNull();
  });
});
