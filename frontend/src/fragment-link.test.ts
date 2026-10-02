import { describe, test, expect, beforeAll, beforeEach, vi } from "vitest";
import { setup } from "./fragment-link";

const scroll = vi.hoisted(() => ({ toElement: vi.fn() }));
vi.mock("./scroll-controller", () => scroll);

function page(html: string): void {
  document.body.innerHTML = `<div class="content"><article class="markdown-body">${html}</article></div>`;
}

function click(selector: string, init: MouseEventInit = {}): MouseEvent {
  const event = new MouseEvent("click", { bubbles: true, cancelable: true, button: 0, ...init });
  document.querySelector(selector)?.dispatchEvent(event);
  return event;
}

describe("fragment links", () => {
  beforeAll(() => {
    setup();
  });

  beforeEach(() => {
    scroll.toElement.mockClear();
  });

  test("a footnote's link back goes to its reference through the scroll controller", () => {
    page(`
      <p>Text<sup><a href="#fn-1" id="fnref-1">1</a></sup></p>
      <section class="footnotes"><ol><li id="fn-1"><p>Note</p>
      <a href="#fnref-1" aria-label="Back to reference 1">↩</a></li></ol></section>`);

    const event = click('a[href="#fnref-1"]');

    expect(event.defaultPrevented).toBe(true);
    expect(scroll.toElement).toHaveBeenCalledWith(document.getElementById("fnref-1"), "center");
  });

  test("a footnote reference goes to its note", () => {
    page(`
      <p>Text<sup><a href="#fn-1" id="fnref-1">1</a></sup></p>
      <section class="footnotes"><ol><li id="fn-1"><p>Note</p></li></ol></section>`);

    click('a[href="#fn-1"]');

    expect(scroll.toElement).toHaveBeenCalledWith(document.getElementById("fn-1"), "center");
  });

  test("a heading is arrived at from its top", () => {
    page(`<h2 id="見出し">見出し</h2><p><a href="#%E8%A6%8B%E5%87%BA%E3%81%97">link</a></p>`);

    click("p a");

    expect(scroll.toElement).toHaveBeenCalledWith(document.getElementById("見出し"), "start");
  });

  test("a fragment naming nothing on the page is left alone", () => {
    page(`<p><a href="#missing">link</a></p>`);

    const event = click("p a");

    expect(event.defaultPrevented).toBe(false);
    expect(scroll.toElement).not.toHaveBeenCalled();
  });

  test("a click with a modifier or another button is left alone", () => {
    page(`<h2 id="a">A</h2><p><a href="#a">link</a></p>`);

    click("p a", { metaKey: true });
    click("p a", { button: 1 });

    expect(scroll.toElement).not.toHaveBeenCalled();
  });

  test("a link in another rendered body, such as a preview, is left alone", () => {
    page(`<h2 id="a">A</h2>`);
    document.body.insertAdjacentHTML(
      "beforeend",
      `<div class="markdown-body" data-arto-apart><p><a href="#a">link</a></p></div>`,
    );

    const event = click("[data-arto-apart] a");

    expect(event.defaultPrevented).toBe(false);
    expect(scroll.toElement).not.toHaveBeenCalled();
  });

  test("a body finds its own place even when the page has one of the same name", () => {
    page(`<h2 id="a">A</h2>`);
    document.body.insertAdjacentHTML(
      "beforeend",
      `<div class="markdown-body answer"><h2 id="a">A</h2><p><a href="#a">link</a></p></div>`,
    );

    click(".answer a");

    expect(scroll.toElement).toHaveBeenCalledWith(document.querySelector(".answer h2"), "start");
  });

  test("a link outside the document is left alone", () => {
    page(`<h2 id="a">A</h2>`);
    document.body.insertAdjacentHTML("beforeend", `<nav><a href="#a">link</a></nav>`);

    click("nav a");

    expect(scroll.toElement).not.toHaveBeenCalled();
  });
});
