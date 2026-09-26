/**
 * What a link points at, shown beside it on a rest of the pointer, so the
 * reader can check a footnote or a reference without leaving their place.
 *
 * A footnote reference shows its note and a link to a heading on the page
 * shows that heading's section, both copied out of the page itself. A link
 * to another document (`span.md-link`, see the HTML contract in
 * `crates/arto-markdown/src/lib.rs`) is asked of the app, which renders
 * that document and answers through `resolve`; the part shown is cut out
 * of the answer here, by the same rules, so the heading ids it is matched
 * against are the ones the renderer wrote rather than a second guess at them.
 *
 * A click is left alone: following the link still works the way it always
 * did.
 */

import { getCurrentElement } from "./content-cursor";
import { placePopover } from "./popover";

/** A little quicker than a lens's note: links are pointed at all the time. */
export const HOVER_DELAY_MS = 350;
/** Time to cross the gap between a link and its preview without losing it. */
export const LEAVE_GRACE_MS = 150;
/** How much of a section is shown before the rest is left to opening it. */
export const MAX_BLOCKS = 12;

const POPOVER = "link-preview";
const POPOVER_ID = "arto-link-preview";
const HEAD = "link-preview-head";
const TITLE = "link-preview-title";
const BODY = "link-preview-body";
const MORE = "link-preview-more";
const UNAVAILABLE = "link-preview-unavailable";
const LINK_SELECTOR = "a[href], span.md-link[data-md-link]";
const HEADING = /^H[1-6]$/;

/** Part of a document, copied, and whether there was more of it. */
export interface Excerpt {
  nodes: Node[];
  truncated: boolean;
}

/** What the app is asked for: the document `link` points at. */
export interface PreviewRequest {
  seq: number;
  link: string;
}

type Preview =
  | { anchor: HTMLElement; kind: "footnote"; target: Element }
  | { anchor: HTMLElement; kind: "heading"; target: Element }
  | { anchor: HTMLElement; kind: "document"; link: string };

/** A copy of `node` that carries no id and no source range. */
function detached(node: Node): Node {
  const copy = node.cloneNode(true);
  if (copy instanceof Element) {
    for (const el of [copy, ...Array.from(copy.querySelectorAll("[id], [data-source-range]"))]) {
      el.removeAttribute("id");
      el.removeAttribute("data-source-range");
    }
  }
  return copy;
}

function headingLevel(el: Element): number | null {
  return HEADING.test(el.tagName) ? Number(el.tagName[1]) : null;
}

/**
 * The blocks under `heading`: its following siblings up to the next heading
 * of its own level or above, the heading itself left out.
 */
export function sectionOf(heading: Element, maxBlocks: number): Excerpt {
  const level = headingLevel(heading) ?? 6;
  const nodes: Node[] = [];
  for (let el = heading.nextElementSibling; el; el = el.nextElementSibling) {
    const other = headingLevel(el);
    if (other !== null && other <= level) break;
    if (el.matches("section.footnotes")) break;
    if (nodes.length >= maxBlocks) return { nodes, truncated: true };
    nodes.push(detached(el));
  }
  return { nodes, truncated: false };
}

/**
 * How a document opens: what precedes its first heading, then that
 * heading and its section. The frontmatter is not what a reader means by
 * the start of a document, and the footnotes are its end.
 */
export function leadOf(root: ParentNode, maxBlocks: number): Excerpt {
  const nodes: Node[] = [];
  let first: number | null = null;
  for (const el of Array.from(root.children)) {
    if (el.matches("details.frontmatter")) continue;
    if (el.matches("section.footnotes")) break;
    const level = headingLevel(el);
    if (level !== null) {
      if (first !== null && level <= first) break;
      first ??= level;
    }
    if (nodes.length >= maxBlocks) return { nodes, truncated: true };
    nodes.push(detached(el));
  }
  return { nodes, truncated: false };
}

/** A footnote's definition, without the links back to where it is referenced. */
export function footnoteOf(item: Element): Excerpt {
  const copy = detached(item) as Element;
  for (const back of Array.from(copy.querySelectorAll('a[href^="#fnref"]'))) back.remove();
  const nodes = Array.from(copy.childNodes).filter(
    (node) => node instanceof Element || (node.textContent ?? "").trim() !== "",
  );
  return { nodes, truncated: false };
}

function decodeFragment(fragment: string): string {
  try {
    return decodeURIComponent(fragment);
  } catch {
    return fragment;
  }
}

/** The link as written, split into its path and its decoded fragment. */
function splitLink(link: string): { path: string; fragment: string | null } {
  const hash = link.indexOf("#");
  if (hash < 0) return { path: link, fragment: null };
  const fragment = link.slice(hash + 1);
  return { path: link.slice(0, hash), fragment: fragment ? decodeFragment(fragment) : null };
}

function fileName(path: string): string {
  const name = path.split(/[/\\]/).filter(Boolean).pop() ?? path;
  return decodeFragment(name);
}

/** The heading with `id` under `root`, matched on the attribute rather than a selector. */
function headingById(root: ParentNode, id: string): Element | null {
  for (const el of Array.from(
    root.querySelectorAll("h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]"),
  )) {
    if (el.id === id) return el;
  }
  return null;
}

// ----------------------------------------------------------------------
// State
// ----------------------------------------------------------------------

let popover: HTMLElement | null = null;
/** The preview being waited for or shown. */
let current: Preview | null = null;
let showTimer: ReturnType<typeof setTimeout> | null = null;
let hideTimer: ReturnType<typeof setTimeout> | null = null;
/** The rest is over, so the preview shows as soon as it has something to show. */
let due = false;
let seq = 0;
/** The app's answer for the current document preview; `undefined` until it comes. */
let answer: string | null | undefined;
let requestHandler: ((request: PreviewRequest) => void) | null = null;
let initialized = false;

/** The preview a hover over `el` asks for, or `null` when it asks for none. */
function previewOf(el: Element): Preview | null {
  if (popover?.contains(el)) return null;
  const anchor = el.closest<HTMLElement>(LINK_SELECTOR);
  if (!anchor || !anchor.closest(".markdown-body")) return null;
  if (anchor.matches("span.md-link")) {
    if (anchor.classList.contains("md-link-missing")) return null;
    if (anchor.classList.contains("md-link-invalid")) return null;
    const link = anchor.dataset.mdLink ?? "";
    return link ? { anchor, kind: "document", link } : null;
  }
  const href = anchor.getAttribute("href") ?? "";
  if (!href.startsWith("#") || href.length < 2) return null;
  const target = document.getElementById(decodeFragment(href.slice(1)));
  if (!target) return null;
  if (target.matches("section.footnotes li")) return { anchor, kind: "footnote", target };
  if (headingLevel(target) !== null) return { anchor, kind: "heading", target };
  return null;
}

function clearTimers(): void {
  if (showTimer) clearTimeout(showTimer);
  if (hideTimer) clearTimeout(hideTimer);
  showTimer = null;
  hideTimer = null;
}

/** Put the preview away and forget what it was for. */
export function hide(): void {
  clearTimers();
  current?.anchor.removeAttribute("aria-describedby");
  current = null;
  due = false;
  answer = undefined;
  popover?.classList.remove("is-visible");
}

function hideLater(): void {
  if (hideTimer) return;
  if (!popover?.classList.contains("is-visible")) {
    hide();
    return;
  }
  hideTimer = setTimeout(hide, LEAVE_GRACE_MS);
}

function keep(): void {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = null;
}

/**
 * Start on `preview`, and show it after `delay`.
 *
 * Another document is asked for only once the rest is over: the app reads
 * and renders it for each request, and a pointer on its way across the
 * page passes over far more links than it stops at.
 */
function begin(preview: Preview, delay: number): void {
  hide();
  if (preview.kind === "document" && !requestHandler) return;
  current = preview;
  // Advanced now rather than when the request goes out, so an answer still
  // on its way for the link before this one no longer matches.
  seq += 1;
  const requestSeq = seq;
  showTimer = setTimeout(() => {
    showTimer = null;
    due = true;
    if (preview.kind === "document") requestHandler?.({ seq: requestSeq, link: preview.link });
    tryShow();
  }, delay);
}

/**
 * The app's answer to request `requestSeq`: the rendered document, or
 * `null` when it cannot be previewed. An answer to any request but the
 * latest is for a link the pointer has left, and is dropped.
 */
export function resolve(requestSeq: number, html: string | null): void {
  if (requestSeq !== seq || current?.kind !== "document") return;
  answer = html;
  tryShow();
}

/** Register the function that asks the app for another document. */
export function onRequest(handler: (request: PreviewRequest) => void): void {
  requestHandler = handler;
}

interface Shown {
  title: string;
  excerpt: Excerpt | null;
  open: (() => void) | null;
}

function openDocument(link: string): () => void {
  return () => window.handleMarkdownLinkClick?.(link, 0);
}

function openHeading(heading: Element): () => void {
  return () => {
    if (window.Arto?.scroll?.toHeading) window.Arto.scroll.toHeading(heading.id);
    else heading.scrollIntoView();
  };
}

function contentFor(preview: Preview): Shown | null {
  switch (preview.kind) {
    case "footnote":
      return { title: "Footnote", excerpt: footnoteOf(preview.target), open: null };
    case "heading":
      return {
        title: (preview.target.textContent ?? "").trim(),
        excerpt: sectionOf(preview.target, MAX_BLOCKS),
        open: openHeading(preview.target),
      };
    case "document": {
      if (answer === undefined) return null;
      const { path, fragment } = splitLink(preview.link);
      const open = openDocument(preview.link);
      const name = fileName(path);
      if (answer === null) return { title: name, excerpt: null, open };
      const template = document.createElement("template");
      template.innerHTML = answer;
      const heading = fragment ? headingById(template.content, fragment) : null;
      if (heading) {
        const title = `${name} › ${(heading.textContent ?? "").trim()}`;
        return { title, excerpt: sectionOf(heading, MAX_BLOCKS), open };
      }
      return { title: name, excerpt: leadOf(template.content, MAX_BLOCKS), open };
    }
  }
}

/**
 * Put away a preview whose link has left the page — the document was
 * replaced under it — rather than leave it floating over the next one.
 */
export function hideIfDetached(): void {
  if (current && !current.anchor.isConnected) hide();
}

function tryShow(): void {
  hideIfDetached();
  if (!due || !current) return;
  const content = contentFor(current);
  if (!content) return;
  show(current.anchor, content);
}

function show(anchor: HTMLElement, { title, excerpt, open }: Shown): void {
  if (!popover?.isConnected) {
    popover = document.createElement("div");
    popover.className = POPOVER;
    popover.id = POPOVER_ID;
    popover.setAttribute("role", "tooltip");
    document.body.appendChild(popover);
  }
  const followThen = (action: () => void) => () => {
    hide();
    action();
  };

  const head = document.createElement("div");
  head.className = HEAD;
  const name = document.createElement(open ? "button" : "span");
  name.className = TITLE;
  name.textContent = title;
  if (open) {
    (name as HTMLButtonElement).type = "button";
    name.addEventListener("click", followThen(open));
  }
  head.append(name);

  const body = document.createElement("div");
  body.className = `markdown-body ${BODY}`;
  if (excerpt) {
    body.append(...excerpt.nodes);
  } else {
    body.classList.add(UNAVAILABLE);
    body.textContent = "No preview available";
  }

  const parts: HTMLElement[] = [head, body];
  if (excerpt?.truncated && open) {
    const more = document.createElement("button");
    more.type = "button";
    more.className = MORE;
    more.textContent = "Open to read more…";
    more.addEventListener("click", followThen(open));
    parts.push(more);
  }
  popover.replaceChildren(...parts);
  anchor.setAttribute("aria-describedby", POPOVER_ID);
  popover.classList.add("is-visible");
  placePopover(popover, anchor);
}

/**
 * Preview the link under the keyboard cursor, right away. Returns whether
 * there was one to preview.
 */
export function showAtCursor(): boolean {
  const el = getCurrentElement();
  if (!el) return false;
  const link = el.closest(LINK_SELECTOR) ?? el.querySelector(LINK_SELECTOR);
  const preview = link ? previewOf(link) : null;
  if (!preview) return false;
  begin(preview, 0);
  return true;
}

function onMouseOver(event: MouseEvent): void {
  const target = event.target;
  if (!(target instanceof Element)) return;
  if (popover?.contains(target)) {
    keep();
    return;
  }
  const preview = previewOf(target);
  if (preview && preview.anchor === current?.anchor) {
    keep();
    return;
  }
  if (!preview) {
    if (current) hideLater();
    return;
  }
  begin(preview, HOVER_DELAY_MS);
}

/** Watch for rests on links, and for whatever puts a preview away. */
export function setup(): void {
  if (initialized) return;
  initialized = true;
  document.addEventListener("mouseover", onMouseOver);
  // Leaving the window raises no `mouseover` to say the link was left.
  // WebViews differ in which of these they send for it, as
  // `scrollbar-reach.ts` found, so both are heard.
  const away = (): void => {
    if (current) hideLater();
  };
  document.addEventListener("mouseleave", away);
  document.addEventListener("mouseout", (event) => {
    if (event.relatedTarget === null) away();
  });
  window.addEventListener("blur", () => {
    if (current) hide();
  });
  document.addEventListener(
    "mousedown",
    (event) => {
      if (event.target instanceof Node && popover?.contains(event.target)) return;
      if (current) hide();
    },
    true,
  );
  // A link followed from inside the preview leaves the preview behind.
  document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element) || !popover?.contains(event.target)) return;
    if (event.target.closest("a, .md-link")) hide();
  });
  document.addEventListener(
    "keydown",
    (event) => {
      if (event.key === "Escape" && current) hide();
    },
    true,
  );
  document.addEventListener(
    "scroll",
    (event) => {
      if (event.target instanceof Node && popover?.contains(event.target)) return;
      if (current) hide();
    },
    true,
  );
}

/** @internal */
export const _internal = {
  reset(): void {
    hide();
    requestHandler = null;
    popover?.remove();
    popover = null;
  },
};
