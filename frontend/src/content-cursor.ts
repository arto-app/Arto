/// Content cursor for keyboard navigation of block elements in the markdown viewer.
///
/// Manages cursor state, navigation (next/prev, heading jump), highlight,
/// and content extraction for copy actions. Cursor state lives in JS because
/// it operates on DOM elements that change on tab switch / file navigation.
///
/// Lazy rescan pattern: before each navigation, verify the current element is
/// still in the DOM via document.contains(). If stale, rescan from .markdown-body.

import { extractTableDelimited, formatTableAsMarkdown } from "./table-utils";

const CURSOR_CLASS = "content-cursor-active";
const HOLD_CLASS = "content-cursor-hold";

const BLOCK_SELECTOR = [
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "p",
  "pre",
  "table",
  "blockquote",
  "ul",
  "ol",
  "img:not(a img)",
  "div.preprocessed-math-display",
].join(", ");

const HEADING_TAGS = new Set(["H1", "H2", "H3", "H4", "H5", "H6"]);

// Module-level cursor state
let elements: Element[] = [];
let currentIndex = -1;

function getMarkdownBody(): Element | null {
  return document.querySelector(".markdown-body");
}

function rescan(): void {
  const body = getMarkdownBody();
  if (!body) {
    elements = [];
    currentIndex = -1;
    return;
  }
  elements = Array.from(body.querySelectorAll(BLOCK_SELECTOR)).filter((el) => {
    // Skip elements nested inside rendered special blocks (keep the block itself)
    const mermaid = el.closest("pre.preprocessed-mermaid");
    if (mermaid && mermaid !== el) return false;
    const math = el.closest("div.preprocessed-math-display");
    if (math && math !== el) return false;

    // Skip empty elements (images are valid without text content)
    if (el.tagName !== "IMG" && !el.textContent?.trim()) return false;

    return true;
  });
}

// Ensure cursor is still pointing to a valid, in-DOM element. If not, rescan.
// Keeps default state as unselected (-1) until explicit cursor navigation.
function ensureValid(): void {
  if (
    currentIndex >= 0 &&
    currentIndex < elements.length &&
    document.contains(elements[currentIndex])
  ) {
    return;
  }
  // Current element is stale or out of range — rescan
  const oldEl = currentIndex >= 0 && currentIndex < elements.length ? elements[currentIndex] : null;
  rescan();
  if (oldEl && document.contains(oldEl)) {
    // Re-locate the element in the new list
    const idx = elements.indexOf(oldEl);
    currentIndex = idx >= 0 ? idx : -1;
  } else {
    currentIndex = -1;
  }
}

function removeHighlight(): void {
  if (currentIndex >= 0 && currentIndex < elements.length) {
    elements[currentIndex].classList.remove(CURSOR_CLASS);
    elements[currentIndex].classList.remove(HOLD_CLASS);
  }
}

function applyHighlight(scroll: boolean): void {
  if (currentIndex >= 0 && currentIndex < elements.length) {
    const el = elements[currentIndex] as HTMLElement;
    // Force animation restart: remove class, trigger reflow, re-add class
    el.classList.remove(CURSOR_CLASS);
    void el.offsetWidth;
    el.classList.add(CURSOR_CLASS);
    if (scroll) {
      el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }
}

function moveTo(newIndex: number, scroll = true): void {
  if (newIndex < 0 || newIndex >= elements.length) return;
  removeHighlight();
  currentIndex = newIndex;
  applyHighlight(scroll);
}

function isVisibleInContentViewport(el: Element): boolean {
  const container = document.querySelector(".content");
  if (!container) return true;
  const containerRect = container.getBoundingClientRect();
  const rect = el.getBoundingClientRect();
  return rect.bottom > containerRect.top && rect.top < containerRect.bottom;
}

function findFirstVisibleIndex(): number {
  for (let i = 0; i < elements.length; i++) {
    if (isVisibleInContentViewport(elements[i])) return i;
  }
  return 0;
}

function findLastVisibleIndex(): number {
  for (let i = elements.length - 1; i >= 0; i--) {
    if (isVisibleInContentViewport(elements[i])) return i;
  }
  return elements.length - 1;
}

// --- Navigation exports ---

export function next(): void {
  ensureValid();
  if (elements.length === 0) return;
  if (currentIndex < 0) {
    moveTo(findFirstVisibleIndex(), false);
    return;
  }
  if (currentIndex + 1 < elements.length) {
    moveTo(currentIndex + 1);
  }
}

export function prev(): void {
  ensureValid();
  if (elements.length === 0) return;
  if (currentIndex < 0) {
    moveTo(findLastVisibleIndex(), false);
    return;
  }
  if (currentIndex > 0) {
    moveTo(currentIndex - 1);
  }
}

export function nextHeading(): void {
  ensureValid();
  if (elements.length === 0) return;
  for (let i = currentIndex + 1; i < elements.length; i++) {
    if (HEADING_TAGS.has(elements[i].tagName)) {
      moveTo(i);
      return;
    }
  }
  // No heading found after current position: stay
}

export function prevHeading(): void {
  ensureValid();
  if (elements.length === 0) return;
  for (let i = currentIndex - 1; i >= 0; i--) {
    if (HEADING_TAGS.has(elements[i].tagName)) {
      moveTo(i);
      return;
    }
  }
  // No heading found before current position: stay
}

/// Re-show the cursor at its current position (restart fade animation).
/// Called when switching from mouse mode to keyboard mode.
export function show(): void {
  ensureValid();
  if (currentIndex >= 0 && currentIndex < elements.length) {
    applyHighlight(false);
  }
}

export function clearCursor(): void {
  removeHighlight();
  currentIndex = -1;
  if (scrollHoldTimer !== null) {
    clearTimeout(scrollHoldTimer);
    scrollHoldTimer = null;
  }
}

export function clearCursorDeferred(): void {
  setTimeout(() => {
    clearCursor();
  }, 0);
}

// --- Scroll sync ---

let pendingSyncTimer: ReturnType<typeof setTimeout> | null = null;
let pendingSyncHandler: (() => void) | null = null;
let scrollHoldTimer: ReturnType<typeof setTimeout> | null = null;

/// Hold cursor visible (suppress fade animation) during keyboard scrolling.
function holdCursor(): void {
  // Keep API surface for compatibility; fading is disabled by CSS.
  if (currentIndex >= 0 && currentIndex < elements.length) {
    elements[currentIndex].classList.add(HOLD_CLASS);
  }
  if (scrollHoldTimer !== null) clearTimeout(scrollHoldTimer);
  scrollHoldTimer = setTimeout(releaseHold, 800);
}

/// Release hold and restart fade animation.
function releaseHold(): void {
  scrollHoldTimer = null;
  if (currentIndex >= 0 && currentIndex < elements.length) {
    const el = elements[currentIndex] as HTMLElement;
    if (el.classList.contains(HOLD_CLASS)) {
      el.classList.remove(HOLD_CLASS);
      // Restart fade animation
      el.classList.remove(CURSOR_CLASS);
      void el.offsetWidth;
      el.classList.add(CURSOR_CLASS);
    }
  }
}

/** After a keyboard scroll action, keep cursor visible and track the viewport.
 *  Cursor stays held (no fade) during scrolling; fades out after scrolling stops.
 *  Walks sequentially from current position to find a visible element.
 *  Waits for smooth scroll to complete via `scrollend` event. */
export function syncToViewport(): void {
  const container = document.querySelector(".content");
  if (!container) return;

  // Cancel any pending sync from a previous rapid scroll
  if (pendingSyncTimer !== null) {
    clearTimeout(pendingSyncTimer);
    pendingSyncTimer = null;
  }
  if (pendingSyncHandler !== null) {
    container.removeEventListener("scrollend", pendingSyncHandler);
    pendingSyncHandler = null;
  }

  // Immediately hold cursor visible during scrolling
  holdCursor();

  const doSync = () => {
    // Cancel the other trigger to prevent double-fire.
    // Both scrollend and setTimeout can invoke doSync; whichever
    // fires first must cancel the other.
    if (pendingSyncTimer !== null) {
      clearTimeout(pendingSyncTimer);
      pendingSyncTimer = null;
    }
    if (pendingSyncHandler !== null) {
      container.removeEventListener("scrollend", pendingSyncHandler);
      pendingSyncHandler = null;
    }

    ensureValid();
    if (elements.length === 0 || currentIndex < 0 || currentIndex >= elements.length) return;

    const containerRect = container.getBoundingClientRect();

    // Already visible — no move needed
    const currentRect = elements[currentIndex].getBoundingClientRect();
    if (currentRect.bottom > containerRect.top && currentRect.top < containerRect.bottom) {
      return;
    }

    // Walk sequentially from current position toward the viewport
    if (currentRect.bottom <= containerRect.top) {
      // Current element is above viewport — scrolled down, walk forward
      // Select the first visible element from the top
      for (let i = currentIndex + 1; i < elements.length; i++) {
        const rect = elements[i].getBoundingClientRect();
        if (rect.bottom > containerRect.top && rect.top < containerRect.bottom) {
          moveTo(i, false);
          holdCursor();
          return;
        }
      }
    } else {
      // Current element is below viewport — scrolled up, walk backward
      // Select the last visible element from the bottom
      for (let i = currentIndex - 1; i >= 0; i--) {
        const rect = elements[i].getBoundingClientRect();
        if (rect.bottom > containerRect.top && rect.top < containerRect.bottom) {
          moveTo(i, false);
          holdCursor();
          return;
        }
      }
    }
  };

  pendingSyncHandler = doSync;
  container.addEventListener("scrollend", doSync, { once: true });
  // Fallback: if scrollend doesn't fire (already at target), use timeout
  pendingSyncTimer = setTimeout(doSync, 500);
}

// --- Content extraction exports ---

export function getCodeText(): string {
  const el = getCurrentElement();
  if (!el || el.tagName !== "PRE") return "";
  const code = el.querySelector("code");
  return code?.textContent ?? "";
}

export function getCodeAsMarkdown(): string {
  const el = getCurrentElement();
  if (!el || el.tagName !== "PRE") return "";
  const code = el.querySelector("code");
  if (!code) return "";
  const lang = extractLanguage(code) ?? "";
  const text = code.textContent ?? "";
  return `\`\`\`${lang}\n${text}\n\`\`\``;
}

export function getTableAsTsv(): string {
  const table = getTableFromCursor();
  if (!table) return "";
  return extractTableDelimited(table, "\t");
}

export function getTableAsCsv(): string {
  const table = getTableFromCursor();
  if (!table) return "";
  return extractTableDelimited(table, ",");
}

export function getTableAsMarkdown(): string {
  const table = getTableFromCursor();
  if (!table) return "";
  return formatTableAsMarkdown(table);
}

export function getImageSrc(): string {
  const el = getCurrentElement();
  if (!el || el.tagName !== "IMG") return "";
  return (el as HTMLImageElement).src;
}

export function getImageAsMarkdown(): string {
  const el = getCurrentElement();
  if (!el || el.tagName !== "IMG") return "";
  const img = el as HTMLImageElement;
  const alt = img.alt ?? "";
  return `![${alt}](${img.src})`;
}

export function getLinkHref(): string {
  const el = getCurrentElement();
  if (!el) return "";
  const anchor = el.closest("a[href]") ?? el.querySelector("a[href]");
  if (!(anchor instanceof HTMLAnchorElement)) return "";
  return anchor.getAttribute("href") ?? "";
}

export function getSourceLineRange(): [number, number] | null {
  const el = getCurrentElement();
  if (!el || !(el instanceof HTMLElement)) return null;
  const range = readSourceLineRange(el);
  if (range.start === null) return null;
  return [range.start, range.end ?? range.start];
}

export function getCurrentElement(): Element | null {
  ensureValid();
  if (currentIndex < 0 || currentIndex >= elements.length) return null;
  return elements[currentIndex];
}

export function setFromContextTarget(target: Element): void {
  if (!(target instanceof Element)) return;
  rescan();
  if (elements.length === 0) return;

  const index = elements.findIndex((el) => el === target || el.contains(target));
  if (index < 0) return;

  moveTo(index, false);
}

// --- Internal helpers ---

/// Find the parent table for cursor elements that may be TR or TABLE.
function getTableFromCursor(): HTMLTableElement | null {
  const el = getCurrentElement();
  if (!el) return null;
  if (el.tagName === "TABLE") return el as HTMLTableElement;
  if (el.tagName === "TR") {
    return el.closest("table") as HTMLTableElement | null;
  }
  return null;
}

function extractLanguage(codeEl: Element): string | null {
  for (const cls of codeEl.classList) {
    if (cls.startsWith("language-")) {
      return cls.replace("language-", "");
    }
  }
  return null;
}

function readSourceLineRange(el: HTMLElement): { start: number | null; end: number | null } {
  const s = el.dataset.sourceLine;
  const e = el.dataset.sourceLineEnd;
  const startVal = s !== undefined ? parseInt(s, 10) : NaN;
  const endVal = e !== undefined ? parseInt(e, 10) : NaN;
  return {
    start: !isNaN(startVal) ? startVal : null,
    end: !isNaN(endVal) ? endVal : null,
  };
}

// extractTableDelimited, escapeDelimitedField, formatTableAsMarkdown
// are imported from table-utils.ts (shared with context-menu-handler)
