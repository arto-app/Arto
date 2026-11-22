import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";

vi.mock("../style/embed.css", () => ({}));

import hljs from "highlight.js/lib/core";
import rust from "highlight.js/lib/languages/rust";

import * as embed from "./embed";
import * as viewportQueue from "./viewport-queue";

/** A host's element holding a rendered fragment, placed in its own page. */
function viewer(fragment: string): HTMLElement {
  const element = document.createElement("div");
  element.innerHTML = `<article class="markdown-body">${fragment}</article>`;
  document.body.append(element);
  return element;
}

const CODE = '<pre><code class="language-rust">fn main() {}</code></pre>';

beforeEach(() => {
  // Without an observer the queue runs every job at once, which is what a
  // test wants to see.
  vi.stubGlobal("IntersectionObserver", undefined);
  hljs.registerLanguage("rust", rust);
  embed.provideHljs(hljs);
});

afterEach(async () => {
  await embed.setTheme("light");
  document.body.replaceChildren();
  viewportQueue.reset();
  vi.unstubAllGlobals();
});

describe("render", () => {
  test("names the current theme on the element it is given", async () => {
    await embed.setTheme("dark_dimmed");
    const element = viewer("<p>hi</p>");

    embed.render(element);

    expect(element.getAttribute("data-theme")).toBe("dark_dimmed");
  });

  test("marks the element as the viewer the stylesheet is confined to", () => {
    const element = viewer("<p>hi</p>");

    embed.render(element);

    expect(element.classList.contains("markdown-viewer")).toBe(true);
  });

  test("leaves the host's own root alone", async () => {
    document.documentElement.setAttribute("data-theme", "host");
    try {
      await embed.setTheme("dark");
      embed.render(viewer("<p>hi</p>"));

      expect(document.documentElement.getAttribute("data-theme")).toBe("host");
    } finally {
      document.documentElement.removeAttribute("data-theme");
    }
  });

  test("colours the code inside the element and nothing outside it", () => {
    const element = viewer(CODE);
    const outside = viewer(CODE);

    embed.render(element);

    expect(element.querySelector("code")?.dataset.highlighted).toBe("yes");
    expect(outside.querySelector("code")?.dataset.highlighted).toBeUndefined();
  });

  test("draws what replaced the element's content when asked again", () => {
    const element = viewer(CODE);
    embed.render(element);

    element.innerHTML = `<article class="markdown-body">${CODE}</article>`;
    embed.render(element);

    expect(element.querySelector("code")?.dataset.highlighted).toBe("yes");
  });
});

describe("setTheme", () => {
  test("names the new theme on every element rendered so far", async () => {
    const first = viewer("<p>one</p>");
    const second = viewer("<p>two</p>");
    embed.render(first);
    embed.render(second);

    await embed.setTheme("dark");

    expect(first.getAttribute("data-theme")).toBe("dark");
    expect(second.getAttribute("data-theme")).toBe("dark");
  });

  test("forgets an element the host has taken out of its page", async () => {
    const element = viewer("<p>hi</p>");
    embed.render(element);
    element.remove();

    await embed.setTheme("dark");

    expect(element.getAttribute("data-theme")).toBe("light");
  });
});
