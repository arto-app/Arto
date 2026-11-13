/**
 * Zenn's HTML embeds: a ` ```html-embed ` fence drawn as the figure its HTML
 * and CSS describe.
 *
 * The markup is the document's, so it is never let into the page. It is
 * drawn in an `<iframe srcdoc>` sandboxed without the page's origin — nothing
 * in it can touch the page, the bridge to Rust, or storage — and the frame's
 * own Content-Security-Policy lets it run no script but the one Arto writes
 * into it and load nothing but images. The markup is also sanitized on the
 * way in, to what Zenn says an embed may use: no script, links that show as
 * text, no form controls but checkboxes and radios, nothing that loads
 * another document. The sandbox and the policy hold even where the
 * sanitizer misses something; the sanitizer is what makes the figure look the
 * way it does on Zenn.
 */

import { FRAME_SCRIPT } from "./html-embed-frame";
import { t } from "./i18n";
import { restoreCopyButton } from "./code-copy";
import { type Theme, currentTheme, isDarkTheme } from "./theme";
import { whenNearViewport } from "./viewport-queue";

/** The largest embed Zenn draws, counted in bytes of UTF-8 as Zenn does. */
export const MAX_BYTES = 50 * 1024;

/** The tallest a frame is shown; a taller figure scrolls inside it. */
export const MAX_HEIGHT = 500;

const BLOCKS = "pre.preprocessed-html-embed:not([data-rendered])";
const FRAME_CLASS = "html-embed-frame";

/**
 * The frames this module draws. Sizing one to its figure changes its `style`,
 * which is layout rather than new content.
 */
export const FRAME_SELECTOR = `iframe.${FRAME_CLASS}`;

/** The two themes Zenn names; Arto's are mapped onto them by darkness. */
type EmbedTheme = "light" | "dark";

function embedTheme(theme: Theme): EmbedTheme {
  return isDarkTheme(theme) ? "dark" : "light";
}

/** Elements that run script, load another document, or change the address. */
const REMOVED = [
  "script",
  "iframe",
  "frame",
  "frameset",
  "object",
  "embed",
  "applet",
  "portal",
  "link",
  "meta",
  "base",
  "form",
  "button",
  "textarea",
  "select",
  "datalist",
].join(",");

/** The attributes a link is followed by. */
const LINK_ATTRIBUTES = [
  "href",
  "xlink:href",
  "target",
  "ping",
  "download",
  "rel",
  "referrerpolicy",
];

/**
 * `markup` parsed where nothing in it loads or runs: a template's content
 * belongs to a document without a browsing context.
 */
function inert(markup: string): HTMLTemplateElement {
  const template = document.createElement("template");
  template.innerHTML = markup;
  return template;
}

/** `markup` cut down to what an embed may use; see the module comment. */
function sanitize(markup: string): string {
  const template = inert(markup);
  const body = template.content;

  for (const element of Array.from(body.querySelectorAll(REMOVED))) {
    element.remove();
  }
  for (const input of Array.from(body.querySelectorAll("input"))) {
    const type = input.getAttribute("type")?.toLowerCase();
    if (type !== "checkbox" && type !== "radio") {
      input.remove();
    }
  }
  for (const element of Array.from(body.querySelectorAll("*"))) {
    for (const name of element.getAttributeNames()) {
      if (/^on/i.test(name) || name.toLowerCase() === "contenteditable") {
        element.removeAttribute(name);
      }
    }
  }
  for (const link of Array.from(body.querySelectorAll("a, area"))) {
    for (const name of LINK_ATTRIBUTES) {
      link.removeAttribute(name);
    }
  }
  return template.innerHTML;
}

/**
 * The policy the frame runs under. A `srcdoc` frame also inherits the page's,
 * so where the page has one — `arto page` — both apply.
 */
function policy(nonce: string): string {
  return [
    "default-src 'none'",
    `script-src 'nonce-${nonce}'`,
    "style-src 'unsafe-inline'",
    "img-src data: http: https:",
    "base-uri 'none'",
    "form-action 'none'",
  ].join("; ");
}

/**
 * The frame's base style. GitHub's font stack, so a figure's text is set like
 * the document's; `flow-root` so that the body's height takes in its first
 * and last child's margins, which is the height the frame reports.
 */
const BASE_STYLE = [
  "html { background: transparent; }",
  "body { margin: 0; display: flow-root; font-size: 16px; line-height: 1.5;",
  ' font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji"; }',
].join("");

/** The whole document an embed's frame shows. */
function frameDocument(markup: string, theme: EmbedTheme, nonce: string): string {
  const content = inert(sanitize(markup));
  // In force from the first paint; the frame's script keeps it so after.
  for (const style of Array.from(content.content.querySelectorAll("style[data-theme]"))) {
    style.setAttribute("media", style.getAttribute("data-theme") === theme ? "all" : "not all");
  }
  return [
    "<!doctype html>",
    `<html data-theme="${theme}" style="color-scheme: ${theme}">`,
    '<head><meta charset="utf-8">',
    `<meta http-equiv="Content-Security-Policy" content="${policy(nonce)}">`,
    `<style>${BASE_STYLE}</style></head>`,
    `<body>${content.innerHTML}<script nonce="${nonce}">${FRAME_SCRIPT}</script></body>`,
    "</html>",
  ].join("");
}

/**
 * A fresh nonce for one frame. `getRandomValues` rather than `randomUUID`:
 * the latter is exposed in a secure context alone, which neither the app's
 * WebView on Windows nor Quick Look's is.
 */
function nonce(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** The height to give a frame that reported `reported`, or null for nonsense. */
function frameHeight(reported: unknown): number | null {
  if (typeof reported !== "number" || !Number.isFinite(reported) || reported < 0) {
    return null;
  }
  return Math.min(Math.ceil(reported), MAX_HEIGHT);
}

let listening = false;

/**
 * Size each frame to what it reports. Only a frame of ours is listened to,
 * found by the window that sent the message — the one thing about a message
 * from an opaque origin that cannot be forged.
 */
function listen(): void {
  if (listening) return;
  listening = true;
  window.addEventListener("message", (event) => {
    const data: unknown = event.data;
    if (typeof data !== "object" || data === null || !("artoHtmlEmbed" in data)) return;
    if (data.artoHtmlEmbed !== "height" || !("height" in data)) return;
    const height = frameHeight(data.height);
    if (height === null) return;
    for (const frame of Array.from(document.querySelectorAll(FRAME_SELECTOR))) {
      if (frame instanceof HTMLIFrameElement && frame.contentWindow === event.source) {
        frame.style.height = `${height}px`;
        return;
      }
    }
  });
}

/**
 * Draw the embeds in `container`, each as the reader comes near, in the theme
 * `theme` names then. Asked for when drawn rather than handed over now: a
 * theme change reaches only the frames already drawn.
 */
export function renderHtmlEmbeds(container: Element, theme: () => Theme): void {
  const blocks = Array.from(container.querySelectorAll<HTMLElement>(BLOCKS));
  if (blocks.length === 0) return;
  listen();
  for (const block of blocks) {
    whenNearViewport(block, () => {
      draw(block, theme());
      restoreCopyButton(block as HTMLPreElement);
    });
  }
}

/**
 * Draw the embeds in `container`, which is not the page — a link's preview —
 * at once and in the page's theme: the viewport queue watches the page's
 * scroller and would never see them come near.
 *
 * Inert, as a preview's diagrams are: a pointer or focus that went into a
 * frame would leave the page's document, which the preview takes for the
 * reader having left the window.
 */
export function renderHtmlEmbedsElsewhere(container: Element): void {
  const blocks = Array.from(container.querySelectorAll<HTMLElement>(BLOCKS));
  if (blocks.length === 0) return;
  listen();
  for (const block of blocks) {
    draw(block, currentTheme());
    block.setAttribute("inert", "");
  }
}

function draw(block: HTMLElement, theme: Theme): void {
  if (block.dataset.rendered === "true") return;
  const markup = block.dataset.originalContent ?? "";
  block.dataset.rendered = "true";

  if (new TextEncoder().encode(markup).length > MAX_BYTES) {
    const error = document.createElement("div");
    error.className = "html-embed-error";
    error.textContent = t("frontend.html_embed.too_large", { limit: `${MAX_BYTES / 1024} KB` });
    block.replaceChildren(error);
    return;
  }

  const frame = document.createElement("iframe");
  frame.className = FRAME_CLASS;
  frame.setAttribute("sandbox", "allow-scripts");
  frame.setAttribute("referrerpolicy", "no-referrer");
  frame.title = t("frontend.html_embed.label");
  frame.srcdoc = frameDocument(markup, embedTheme(theme), nonce());

  const label = document.createElement("span");
  label.className = "html-embed-label";
  label.textContent = t("frontend.html_embed.label");

  block.replaceChildren(frame, label);
}

/** Tell every frame drawn in `container` that the theme is now `theme`. */
export function setHtmlEmbedTheme(container: ParentNode, theme: Theme): void {
  const message = { artoHtmlEmbed: "theme", theme: embedTheme(theme) };
  for (const frame of Array.from(container.querySelectorAll(FRAME_SELECTOR))) {
    if (frame instanceof HTMLIFrameElement) {
      frame.contentWindow?.postMessage(message, "*");
    }
  }
}

/** @internal */
export const _internal = { sanitize, frameDocument, frameHeight };
