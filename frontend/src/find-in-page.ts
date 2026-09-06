/**
 * Pinned search definition from Rust.
 */
export interface PinnedSearchDef {
  /** Unique identifier */
  id: string;
  /** Search pattern (plain text) */
  pattern: string;
  /** Highlight color: green, blue, pink, orange, purple */
  color: "green" | "blue" | "pink" | "orange" | "purple";
  /** Case-sensitive matching */
  caseSensitive: boolean;
  /** Whether this search is disabled (still collect matches, but don't highlight) */
  disabled: boolean;
}

interface SearchState {
  query: string;
  currentIndex: number;
  highlightElements: HTMLElement[];
  // Pinned search state
  pinnedSearches: PinnedSearchDef[];
  pinnedHighlights: Map<string, HTMLElement[]>;
}

const state: SearchState = {
  query: "",
  currentIndex: 0,
  highlightElements: [],
  pinnedSearches: [],
  pinnedHighlights: new Map(),
};

/**
 * Match information for displaying in the Search tab.
 */
export interface SearchMatch {
  /** 0-based index of this match */
  index: number;
  /** The matched text itself */
  text: string;
  /** Surrounding context including the match */
  context: string;
  /** Start position of match within context */
  contextStart: number;
  /** End position of match within context */
  contextEnd: number;
}

/**
 * Full search result including all match details.
 */
export interface SearchResult {
  query: string;
  total: number;
  current: number;
  matches: SearchMatch[];
  /** Pinned search matches keyed by pinned search ID */
  pinnedMatches: Record<string, SearchMatch[]>;
}

type SearchCallback = (data: {
  count: number;
  current: number;
  query: string;
  matches: SearchMatch[];
  pinnedMatches: Record<string, SearchMatch[]>;
}) => void;

let callback: SearchCallback | null = null;

/**
 * Apply highlights for a search query.
 * Returns the highlight elements created.
 */
function applyHighlights(
  container: HTMLElement,
  query: string,
  caseSensitive: boolean,
  className: string,
  dataAttributes?: Record<string, string>,
): HTMLElement[] {
  if (!query || query.length === 0) {
    return [];
  }

  const textNodes: Text[] = [];
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => {
      const parent = node.parentElement;
      // Exclude code blocks (pre), mermaid diagrams, and already highlighted text
      // Note: inline <code> tags are intentionally searchable
      if (
        parent?.closest(
          "pre, .mermaid, .search-highlight, .pinned-highlight, .pinned-highlight-disabled",
        )
      ) {
        return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });

  let node: Node | null;
  while ((node = walker.nextNode())) {
    textNodes.push(node as Text);
  }

  const queryToMatch = caseSensitive ? query : query.toLowerCase();
  const elements: HTMLElement[] = [];

  // Process each text node
  for (const textNode of textNodes) {
    const text = textNode.textContent || "";
    const textToSearch = caseSensitive ? text : text.toLowerCase();
    let startIndex = 0;

    // Find all matches in this text node
    const matches: { start: number; end: number }[] = [];
    while (true) {
      const index = textToSearch.indexOf(queryToMatch, startIndex);
      if (index === -1) break;
      matches.push({ start: index, end: index + query.length });
      startIndex = index + 1;
    }

    if (matches.length === 0) continue;

    // Replace text node with highlighted fragments
    const parent = textNode.parentNode;
    if (!parent) continue;

    const fragment = document.createDocumentFragment();
    let lastEnd = 0;

    for (const match of matches) {
      // Text before match
      if (match.start > lastEnd) {
        fragment.appendChild(document.createTextNode(text.slice(lastEnd, match.start)));
      }

      // Highlighted match
      const mark = document.createElement("mark");
      mark.className = className;
      mark.textContent = text.slice(match.start, match.end);

      // Add data attributes if provided
      if (dataAttributes) {
        for (const [key, value] of Object.entries(dataAttributes)) {
          mark.setAttribute(key, value);
        }
      }

      fragment.appendChild(mark);
      elements.push(mark);

      lastEnd = match.end;
    }

    // Text after last match
    if (lastEnd < text.length) {
      fragment.appendChild(document.createTextNode(text.slice(lastEnd)));
    }

    parent.replaceChild(fragment, textNode);
  }

  return elements;
}

function highlightMatches(container: HTMLElement, query: string): number {
  // Clear existing search highlights first (not pinned)
  clearSearchHighlights();

  state.highlightElements = applyHighlights(container, query, false, "search-highlight");

  return state.highlightElements.length;
}

function clearSearchHighlights(): void {
  // Remove all search highlight marks and restore original text
  for (const mark of state.highlightElements) {
    const parent = mark.parentNode;
    if (parent) {
      // Replace mark with its text content
      const textNode = document.createTextNode(mark.textContent || "");
      parent.replaceChild(textNode, mark);
      // Normalize to merge adjacent text nodes
      parent.normalize();
    }
  }
  state.highlightElements = [];
  state.currentIndex = 0;
}

function clearPinnedHighlights(): void {
  // Remove all pinned highlight marks (including disabled ones)
  for (const elements of state.pinnedHighlights.values()) {
    for (const mark of elements) {
      const parent = mark.parentNode;
      if (parent) {
        const textNode = document.createTextNode(mark.textContent || "");
        parent.replaceChild(textNode, mark);
        parent.normalize();
      }
    }
  }
  state.pinnedHighlights.clear();
}

function navigateToMatch(direction: "next" | "prev"): number {
  if (state.highlightElements.length === 0) return 0;

  // Remove active class from current
  const current = state.highlightElements[state.currentIndex];
  current?.classList.remove("search-highlight-active");

  // Calculate new index
  if (direction === "next") {
    state.currentIndex = (state.currentIndex + 1) % state.highlightElements.length;
  } else {
    state.currentIndex =
      (state.currentIndex - 1 + state.highlightElements.length) % state.highlightElements.length;
  }

  // Add active class to new current and scroll into view
  const next = state.highlightElements[state.currentIndex];
  next?.classList.add("search-highlight-active");
  next?.scrollIntoView({ behavior: "smooth", block: "center" });

  return state.currentIndex + 1; // 1-based for display
}

/**
 * Apply pinned search highlights.
 * This should be called after DOM content changes to re-apply all pinned highlights.
 * Disabled searches still create DOM elements but with invisible styling.
 */
function applyPinnedHighlights(): void {
  const container = document.querySelector(".markdown-body");
  if (!container) return;

  // Clear existing pinned highlights
  clearPinnedHighlights();

  // Apply highlights for each pinned search
  for (const pinned of state.pinnedSearches) {
    // Use invisible class for disabled searches (DOM exists, but no visual highlight)
    const className = pinned.disabled ? "pinned-highlight-disabled" : "pinned-highlight";
    const elements = applyHighlights(
      container as HTMLElement,
      pinned.pattern,
      pinned.caseSensitive,
      className,
      { "data-color": pinned.color, "data-pinned-id": pinned.id },
    );
    state.pinnedHighlights.set(pinned.id, elements);
  }
}

export function find(query: string): void {
  state.query = query;
  const container = document.querySelector(".markdown-body");
  if (!container) {
    callback?.({ count: 0, current: 0, query: "", matches: [], pinnedMatches: {} });
    return;
  }

  // Ensure search highlights take priority over pinned highlights
  clearPinnedHighlights();
  const count = highlightMatches(container as HTMLElement, query);
  state.currentIndex = count > 0 ? 0 : -1;

  // Activate first match (no auto-scroll to avoid focus issues with IME)
  if (count > 0) {
    state.highlightElements[0]?.classList.add("search-highlight-active");
  }

  // Re-apply pinned highlights after search so search wins on overlap
  applyPinnedHighlights();

  const matches = collectSearchMatches();
  const pinnedMatches = collectPinnedMatches();
  callback?.({ count, current: count > 0 ? 1 : 0, query: state.query, matches, pinnedMatches });
}

export function navigate(direction: "next" | "prev"): void {
  const current = navigateToMatch(direction);
  const matches = collectSearchMatches();
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: state.highlightElements.length,
    current,
    query: state.query,
    matches,
    pinnedMatches,
  });
}

export function clear(): void {
  state.query = "";
  clearSearchHighlights();
  const pinnedMatches = collectPinnedMatches();
  callback?.({ count: 0, current: 0, query: "", matches: [], pinnedMatches });
}

export function setup(cb: SearchCallback): void {
  callback = cb;
}

/**
 * Re-apply the current search query and pinned searches after DOM changes (e.g., tab switch).
 * This preserves highlights across tab navigation.
 */
export function reapply(): void {
  // Re-apply search first so it wins on overlap
  if (state.query) {
    find(state.query);
    return;
  }

  // No search query; just re-apply pinned highlights
  applyPinnedHighlights();
  const pinnedMatches = collectPinnedMatches();
  callback?.({ count: 0, current: 0, query: "", matches: [], pinnedMatches });
}

/**
 * Navigate directly to a specific match by index.
 * Used by the Search tab for clicking on match items.
 */
export function navigateTo(index: number): void {
  if (index < 0 || index >= state.highlightElements.length) {
    return;
  }

  // Remove active class from current match
  const current = state.highlightElements[state.currentIndex];
  current?.classList.remove("search-highlight-active");

  // Update index and activate new match
  state.currentIndex = index;
  const target = state.highlightElements[index];
  target?.classList.add("search-highlight-active");
  target?.scrollIntoView({ behavior: "smooth", block: "center" });

  // Notify callback with unified format
  const newCurrent = index + 1;
  const matches = collectSearchMatches();
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: state.highlightElements.length,
    current: newCurrent,
    query: state.query,
    matches,
    pinnedMatches,
  });
}

/**
 * Set the list of pinned searches and re-apply highlights.
 */
export function setPinned(pinned: PinnedSearchDef[]): void {
  state.pinnedSearches = pinned;

  // Keep search highlights prioritized if a query is active
  if (state.query) {
    find(state.query);
    return;
  }

  applyPinnedHighlights();
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: state.highlightElements.length,
    current: state.currentIndex >= 0 ? state.currentIndex + 1 : 0,
    query: state.query,
    matches: collectSearchMatches(),
    pinnedMatches,
  });
}

/**
 * Scroll to a pinned search match.
 */
export function scrollToPinnedMatch(pinnedId: string, index: number): void {
  const elements = state.pinnedHighlights.get(pinnedId);
  if (!elements || index < 0 || index >= elements.length) {
    return;
  }

  const target = elements[index];
  target?.scrollIntoView({ behavior: "smooth", block: "center" });

  // Brief highlight effect
  target?.classList.add("pinned-highlight-flash");
  setTimeout(() => {
    target?.classList.remove("pinned-highlight-flash");
  }, 500);
}

/**
 * Collect context around an element (text before and after).
 */
function getContext(
  element: HTMLElement,
  maxChars: number,
): { text: string; matchStart: number; matchEnd: number } {
  const matchText = element.textContent || "";

  // Get text from siblings and parent text nodes
  const before = getTextBefore(element, maxChars);
  const after = getTextAfter(element, maxChars);

  const text = before + matchText + after;
  const matchStart = before.length;
  const matchEnd = matchStart + matchText.length;

  return { text, matchStart, matchEnd };
}

/**
 * Get text content before an element, stopping at newlines.
 */
function getTextBefore(element: HTMLElement, maxChars: number): string {
  let text = "";
  let node: Node | null = element;

  // Walk backwards through siblings and parent's previous siblings
  outer: while (node && text.length < maxChars) {
    if (node.previousSibling) {
      node = node.previousSibling;
      const content = getNodeTextContent(node);
      // Stop at newline
      const newlineIdx = content.lastIndexOf("\n");
      if (newlineIdx !== -1) {
        text = content.slice(newlineIdx + 1) + text;
        break outer;
      }
      text = content + text;
    } else {
      // Move up to parent and continue
      node = node.parentElement;
      if (node && node.closest(".markdown-body")) {
        continue;
      }
      break;
    }
  }

  // Trim to maxChars from the end (no ellipsis since we stop at line boundary)
  if (text.length > maxChars) {
    text = text.slice(-maxChars);
  }

  return text;
}

/**
 * Get text content after an element, stopping at newlines.
 */
function getTextAfter(element: HTMLElement, maxChars: number): string {
  let text = "";
  let node: Node | null = element;

  // Walk forwards through siblings and parent's next siblings
  outer: while (node && text.length < maxChars) {
    if (node.nextSibling) {
      node = node.nextSibling;
      const content = getNodeTextContent(node);
      // Stop at newline
      const newlineIdx = content.indexOf("\n");
      if (newlineIdx !== -1) {
        text = text + content.slice(0, newlineIdx);
        break outer;
      }
      text = text + content;
    } else {
      // Move up to parent and continue
      node = node.parentElement;
      if (node && node.closest(".markdown-body")) {
        continue;
      }
      break;
    }
  }

  // Trim to maxChars from the start (no ellipsis since we stop at line boundary)
  if (text.length > maxChars) {
    text = text.slice(0, maxChars);
  }

  return text;
}

/**
 * Get text content of a node, handling different node types.
 */
function getNodeTextContent(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.textContent || "";
  }
  if (node.nodeType === Node.ELEMENT_NODE) {
    const el = node as HTMLElement;
    // Skip highlight marks to get actual text
    if (el.classList.contains("search-highlight") || el.classList.contains("pinned-highlight")) {
      return el.textContent || "";
    }
    return el.textContent || "";
  }
  return "";
}

/**
 * Collect all search match information for the Search tab.
 */
function collectSearchMatches(): SearchMatch[] {
  const matches: SearchMatch[] = [];
  const contextChars = 30;

  for (let i = 0; i < state.highlightElements.length; i++) {
    const el = state.highlightElements[i];
    const text = el.textContent || "";
    const context = getContext(el, contextChars);

    matches.push({
      index: i,
      text,
      context: context.text,
      contextStart: context.matchStart,
      contextEnd: context.matchEnd,
    });
  }

  return matches;
}

/**
 * Collect all pinned search matches.
 */
function collectPinnedMatches(): Record<string, SearchMatch[]> {
  const result: Record<string, SearchMatch[]> = {};
  const contextChars = 30;

  for (const [pinnedId, elements] of state.pinnedHighlights) {
    const matches: SearchMatch[] = [];
    for (let i = 0; i < elements.length; i++) {
      const el = elements[i];
      const text = el.textContent || "";
      const context = getContext(el, contextChars);

      matches.push({
        index: i,
        text,
        context: context.text,
        contextStart: context.matchStart,
        contextEnd: context.matchEnd,
      });
    }
    result[pinnedId] = matches;
  }

  return result;
}
