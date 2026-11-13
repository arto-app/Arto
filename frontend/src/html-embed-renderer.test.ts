import { describe, test, expect, afterEach } from "vitest";

import { FRAME_SCRIPT } from "./html-embed-frame";
import {
  _internal,
  MAX_HEIGHT,
  renderHtmlEmbeds,
  renderHtmlEmbedsElsewhere,
  setHtmlEmbedTheme,
} from "./html-embed-renderer";
import * as viewportQueue from "./viewport-queue";

const { sanitize, frameDocument, frameHeight } = _internal;

function page(html: string): HTMLElement {
  document.body.innerHTML = `<article class="markdown-body">${html}</article>`;
  return document.querySelector(".markdown-body") as HTMLElement;
}

function embed(source: string): string {
  const block = document.createElement("pre");
  block.className = "preprocessed-html-embed";
  block.dataset.originalContent = source;
  block.textContent = source;
  return block.outerHTML;
}

/**
 * The markup `sanitize` leaves, parsed back so it can be queried — by a
 * parser whose document loads nothing, should a test's markup survive that
 * ought not to.
 */
function sanitized(markup: string): HTMLTemplateElement {
  const template = document.createElement("template");
  template.innerHTML = sanitize(markup);
  return template;
}

afterEach(() => {
  viewportQueue.reset();
  document.body.innerHTML = "";
});

describe("sanitize", () => {
  test("keeps the markup and the styles a figure is drawn with", () => {
    const holder = sanitized(
      `<div class="flow"><div class="box">入力</div></div><style>.box { color: red; }</style>`,
    );

    expect(holder.content.querySelector(".flow .box")?.textContent).toBe("入力");
    expect(holder.content.querySelector("style")?.textContent).toBe(".box { color: red; }");
  });

  test("takes out script and everything that runs it", () => {
    const holder = sanitized(
      `<script>alert(1)</script><img src="x.png" onerror="alert(2)"><div onclick="alert(3)">x</div>`,
    );

    expect(holder.content.querySelector("script")).toBeNull();
    expect(holder.content.querySelector("img")?.hasAttribute("onerror")).toBe(false);
    expect(holder.content.querySelector("div")?.hasAttribute("onclick")).toBe(false);
  });

  test("a link keeps its text and loses where it goes", () => {
    const holder = sanitized(
      `<a href="https://example.com" target="_top" ping="https://example.com">here</a>` +
        `<svg><a href="https://example.com"><text>there</text></a></svg>`,
    );

    const [html, svg] = Array.from(holder.content.querySelectorAll("a"));
    expect(html.textContent).toBe("here");
    expect(html.getAttributeNames()).toEqual([]);
    expect(svg.hasAttribute("href")).toBe(false);
  });

  test("nothing the sanitizer cannot see into is kept", () => {
    const holder = sanitized(
      `<div><template shadowrootmode="open"><a href="https://example.com">go</a><input type="text"></template></div>`,
    );

    expect(holder.innerHTML).toBe("<div></div>");
  });

  test("an SVG animation cannot give a link back the address it lost", () => {
    const holder = sanitized(
      `<svg><a><set attributeName="href" to="https://example.com"/>` +
        `<animate attributeName="xlink:href" values="https://example.com"/>` +
        `<animate attributeName="opacity" values="0;1"/><text>x</text></a></svg>`,
    );

    const animations = Array.from(holder.content.querySelectorAll("set, animate"), (element) =>
      element.getAttribute("attributeName"),
    );
    expect(animations).toEqual(["opacity"]);
  });

  test("a MathML element is not a link either", () => {
    const holder = sanitized(`<math><mi href="https://example.com">x</mi></math>`);

    expect(holder.content.querySelector("mi")?.hasAttribute("href")).toBe(false);
  });

  test("takes out forms and their contents, but keeps checkboxes and radios", () => {
    const holder = sanitized(
      `<form action="/x"><input name="q"><button>go</button></form>` +
        `<textarea>t</textarea><select><option>o</option></select>` +
        `<input type="checkbox" id="tab"><input type="radio" name="r">`,
    );

    expect(holder.content.querySelector("form")).toBeNull();
    expect(holder.content.querySelector("button")).toBeNull();
    expect(holder.content.querySelector("textarea")).toBeNull();
    expect(holder.content.querySelector("select")).toBeNull();
    expect(Array.from(holder.content.querySelectorAll("input"), (input) => input.type)).toEqual([
      "checkbox",
      "radio",
    ]);
  });

  test("takes out what loads another document or changes this one's address", () => {
    const holder = sanitized(
      `<iframe src="https://example.com"></iframe><object data="x"></object><embed src="x">` +
        `<link rel="stylesheet" href="https://example.com/a.css">` +
        `<meta http-equiv="refresh" content="0;url=https://example.com"><base href="https://example.com/">`,
    );

    expect(holder.innerHTML).toBe("");
  });

  test("nothing is left editable", () => {
    const holder = sanitized(`<div contenteditable="true">x</div>`);

    expect(holder.content.querySelector("div")?.hasAttribute("contenteditable")).toBe(false);
  });
});

describe("frameDocument", () => {
  function parse(markup: string, theme: "light" | "dark" = "light"): Document {
    return new DOMParser().parseFromString(frameDocument(markup, theme, "n0nce"), "text/html");
  }

  test("runs only its own script, and loads nothing but images", () => {
    const doc = parse("<p>x</p>");

    const policy = doc
      .querySelector('meta[http-equiv="Content-Security-Policy"]')
      ?.getAttribute("content");
    expect(policy).toContain("default-src 'none'");
    expect(policy).toContain("script-src 'nonce-n0nce'");
    expect(policy).toContain("img-src");
    // The policy has to be in force before anything the document wrote.
    expect(doc.head.firstElementChild?.nextElementSibling?.getAttribute("http-equiv")).toBe(
      "Content-Security-Policy",
    );

    const scripts = Array.from(doc.querySelectorAll("script"));
    expect(scripts).toHaveLength(1);
    expect(scripts[0].getAttribute("nonce")).toBe("n0nce");
    expect(scripts[0].textContent).toBe(FRAME_SCRIPT);
  });

  test("names the theme the way Zenn does, on <html>", () => {
    expect(parse("", "dark").documentElement.getAttribute("data-theme")).toBe("dark");
    expect(parse("", "light").documentElement.getAttribute("data-theme")).toBe("light");
  });

  test("a style written for the other theme starts out of force", () => {
    const doc = parse(
      `<style data-theme="light">.a{}</style><style data-theme="dark">.b{}</style>`,
      "dark",
    );

    expect(doc.querySelector('style[data-theme="light"]')?.getAttribute("media")).toBe("not all");
    expect(doc.querySelector('style[data-theme="dark"]')?.getAttribute("media")).toBe("all");
  });

  test("the document's markup is sanitized on the way in", () => {
    const doc = parse(`<script>alert(1)</script><p>x</p>`);

    expect(doc.body.querySelector("p")?.textContent).toBe("x");
    expect(Array.from(doc.querySelectorAll("script"), (s) => s.textContent)).toEqual([
      FRAME_SCRIPT,
    ]);
  });
});

describe("frameHeight", () => {
  test("follows the content, up to the cap", () => {
    expect(frameHeight(120.2)).toBe(121);
    expect(frameHeight(MAX_HEIGHT + 300)).toBe(MAX_HEIGHT);
  });

  test("refuses what is not a height", () => {
    expect(frameHeight(-1)).toBeNull();
    expect(frameHeight(Number.NaN)).toBeNull();
    expect(frameHeight("100")).toBeNull();
  });
});

describe("renderHtmlEmbeds", () => {
  test("an embed is drawn in a sandboxed frame once the reader comes near it", async () => {
    const body = page(embed(`<div class="box">x</div>`));

    renderHtmlEmbeds(body, () => "light");
    await viewportQueue.flush();

    const block = body.querySelector("pre.preprocessed-html-embed") as HTMLElement;
    const frame = block.querySelector("iframe") as HTMLIFrameElement;
    expect(frame).not.toBeNull();
    // Scripts, so that the frame can say how tall it is; never the page's
    // origin, so that nothing in it can reach the page.
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame.getAttribute("srcdoc")).toContain(`<div class="box">x</div>`);
    expect(block.dataset.rendered).toBe("true");
    // Said to be the author's, not the reader's application's.
    expect(block.querySelector(".html-embed-label")?.textContent).toBe("User-created content");
  });

  test("a dark theme is painted as Zenn's dark", async () => {
    const body = page(embed(`<p>x</p>`));

    renderHtmlEmbeds(body, () => "dark_dimmed");
    await viewportQueue.flush();

    expect(body.querySelector("iframe")?.getAttribute("srcdoc")).toContain('data-theme="dark"');
  });

  test("an embed over the size limit says so instead of being drawn", async () => {
    const body = page(embed(`<p>${"あ".repeat(20 * 1024)}</p>`));

    renderHtmlEmbeds(body, () => "light");
    await viewportQueue.flush();

    const block = body.querySelector("pre.preprocessed-html-embed") as HTMLElement;
    expect(block.querySelector("iframe")).toBeNull();
    expect(block.querySelector(".html-embed-error")).not.toBeNull();
  });

  test("a block already drawn is left as it is", async () => {
    const body = page(embed(`<p>x</p>`));

    renderHtmlEmbeds(body, () => "light");
    await viewportQueue.flush();
    const frame = body.querySelector("iframe");
    renderHtmlEmbeds(body, () => "light");
    await viewportQueue.flush();

    expect(body.querySelector("iframe")).toBe(frame);
  });

  test("an embed is drawn in the theme painted when the reader comes near it", async () => {
    const body = page(embed(`<p>x</p>`));
    let theme = "light";

    renderHtmlEmbeds(body, () => theme);
    theme = "dark";
    await viewportQueue.flush();

    expect(body.querySelector("iframe")?.getAttribute("srcdoc")).toContain('data-theme="dark"');
  });
});

describe("renderHtmlEmbedsElsewhere", () => {
  test("an embed outside the page is drawn at once, in the page's theme", () => {
    document.documentElement.setAttribute("data-theme", "dark");
    const card = page(embed(`<p>x</p>`));

    renderHtmlEmbedsElsewhere(card);
    document.documentElement.removeAttribute("data-theme");

    const frame = card.querySelector("iframe");
    expect(frame?.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame?.getAttribute("srcdoc")).toContain('data-theme="dark"');
  });

  test("an embed outside the page is only looked at: the pointer and focus stay out", () => {
    const card = page(embed(`<input type="checkbox">`));

    renderHtmlEmbedsElsewhere(card);

    expect(card.querySelector("pre.preprocessed-html-embed")?.hasAttribute("inert")).toBe(true);
  });
});

describe("setHtmlEmbedTheme", () => {
  test("tells every frame drawn in the container the new theme", async () => {
    const body = page(embed(`<p>x</p>`));
    renderHtmlEmbeds(body, () => "light");
    await viewportQueue.flush();

    const frame = body.querySelector("iframe") as HTMLIFrameElement;
    const received: unknown[] = [];
    frame.contentWindow?.addEventListener("message", (event) => received.push(event.data));

    setHtmlEmbedTheme(body, "dark_high_contrast");
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(received).toContainEqual({ artoHtmlEmbed: "theme", theme: "dark" });
  });
});
