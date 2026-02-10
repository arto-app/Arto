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

/**
 * A single fuzzy match entry (one per block).
 * Range background is rendered via CSS Custom Highlight API.
 * Char-level bold uses DOM <mark> elements (font-weight not supported by ::highlight).
 */
interface FuzzyMatchEntry {
  /** Range spanning the match region (for Highlight API background) */
  range: Range;
  /** First char mark element (for scrollIntoView navigation) */
  scrollTarget: HTMLElement;
  /** All char mark elements (for cleanup) */
  charMarks: HTMLElement[];
  /** Code elements within the range whose background is temporarily hidden */
  codeElements: HTMLElement[];
}

interface SearchState {
  query: string;
  currentIndex: number;
  /** Entries are invalidated on DOM re-render; call reapply() to rebuild. */
  fuzzyMatches: FuzzyMatchEntry[];
  /** Last Rust-computed matches for reapply after DOM re-render */
  lastHighlightMatches: HighlightMatch[];
  /**
   * Pending match index to navigate to on the next applyHighlights call.
   * Set by Enter key jump; consumed by applyHighlights after highlights are placed.
   * Survives across calls where entries are empty (e.g., file switching).
   */
  pendingNavigateIndex: number | null;
  // Pinned search state
  pinnedSearches: PinnedSearchDef[];
  pinnedHighlights: Map<string, HTMLElement[]>;
}

/**
 * Result of a fuzzy subsequence match against a block's text.
 */
interface FuzzyBlockMatch {
  /** Character positions where query chars matched (in block's concatenated text) */
  charIndices: number[];
  /** Start of the match range (min of charIndices) */
  rangeStart: number;
  /** End of the match range, exclusive (max of charIndices + 1) */
  rangeEnd: number;
}

const state: SearchState = {
  query: "",
  currentIndex: 0,
  fuzzyMatches: [],
  lastHighlightMatches: [],
  pendingNavigateIndex: null,
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
 * Apply exact string highlights for a search query (used for pinned searches).
 * Returns the highlight elements created.
 */
function applyExactStringHighlights(
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
      if (parent?.closest("pre, .mermaid, .pinned-highlight, .pinned-highlight-disabled")) {
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

function clearSearchHighlights(): void {
  // Clear CSS Custom Highlight API entries
  CSS.highlights.delete("search-range");
  CSS.highlights.delete("search-range-active");

  // Restore code element backgrounds and remove char mark DOM elements
  for (const entry of state.fuzzyMatches) {
    for (const el of entry.codeElements) {
      el.classList.remove("code-in-search-highlight");
    }
    for (const mark of entry.charMarks) {
      const parent = mark.parentNode;
      if (parent) {
        const textNode = document.createTextNode(mark.textContent || "");
        parent.replaceChild(textNode, mark);
        parent.normalize();
      }
    }
  }
  state.fuzzyMatches = [];
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

/**
 * Update the CSS Custom Highlight API registrations from current fuzzy match state.
 */
function updateHighlightAPI(): void {
  if (state.fuzzyMatches.length > 0) {
    const ranges = state.fuzzyMatches.map((m) => m.range);
    const rangeHighlight = new Highlight(...ranges);
    rangeHighlight.priority = 0;
    CSS.highlights.set("search-range", rangeHighlight);
  } else {
    CSS.highlights.delete("search-range");
  }

  // Active highlight (current match only, higher priority to layer on top)
  if (state.currentIndex >= 0 && state.currentIndex < state.fuzzyMatches.length) {
    const activeRange = state.fuzzyMatches[state.currentIndex].range;
    const activeHighlight = new Highlight(activeRange);
    activeHighlight.priority = 1;
    CSS.highlights.set("search-range-active", activeHighlight);
  } else {
    CSS.highlights.delete("search-range-active");
  }
}

function navigateToMatch(direction: "next" | "prev"): number {
  if (state.fuzzyMatches.length === 0) return 0;

  // Calculate new index
  if (direction === "next") {
    state.currentIndex = (state.currentIndex + 1) % state.fuzzyMatches.length;
  } else {
    state.currentIndex =
      (state.currentIndex - 1 + state.fuzzyMatches.length) % state.fuzzyMatches.length;
  }

  // Update active highlight via Highlight API and scroll into view
  updateHighlightAPI();
  const entry = state.fuzzyMatches[state.currentIndex];
  entry.scrollTarget.scrollIntoView({ behavior: "smooth", block: "center" });

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
    const elements = applyExactStringHighlights(
      container as HTMLElement,
      pinned.pattern,
      pinned.caseSensitive,
      className,
      { "data-color": pinned.color, "data-pinned-id": pinned.id },
    );
    state.pinnedHighlights.set(pinned.id, elements);
  }
}

interface TextNodeInfo {
  node: Text;
  offset: number;
}

/**
 * Collect searchable text nodes from a block element with their offsets.
 * Skips text inside pre, mermaid, and existing pinned highlight marks to avoid
 * nesting highlights and to keep offsets consistent with the search text.
 */
function collectTextNodes(block: Element): { nodes: TextNodeInfo[]; text: string } {
  const nodes: TextNodeInfo[] = [];
  const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => {
      const parent = node.parentElement;
      if (parent?.closest("pre, .mermaid, .pinned-highlight, .pinned-highlight-disabled")) {
        return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });

  let offset = 0;
  let node: Node | null;
  while ((node = walker.nextNode())) {
    nodes.push({ node: node as Text, offset });
    offset += (node.textContent || "").length;
  }

  const text = nodes.map((n) => n.node.textContent || "").join("");
  return { nodes, text };
}

/**
 * Apply fuzzy highlighting to a single block element.
 *
 * Creates char-level DOM marks (.fuzzy-highlight-char) for matched characters
 * and builds a Range spanning the match region for CSS Custom Highlight API.
 *
 * Returns a FuzzyMatchEntry for navigation and cleanup, or null if no match.
 */
function highlightBlock(
  textNodeInfos: TextNodeInfo[],
  match: FuzzyBlockMatch,
): FuzzyMatchEntry | null {
  const charIndexSet = new Set(match.charIndices);
  const charMarks: HTMLElement[] = [];
  const codeElements: HTMLElement[] = [];

  // Track range boundary nodes in the modified DOM
  let rangeStartNode: Node | null = null;
  let rangeStartOffset = 0;
  let rangeEndNode: Node | null = null;
  let rangeEndOffset = 0;

  // Process each text node that overlaps with the match range
  for (const { node: textNode, offset: nodeOffset } of textNodeInfos) {
    const text = textNode.textContent || "";
    const nodeEnd = nodeOffset + text.length;

    // Check overlap with range
    const overlapStart = Math.max(match.rangeStart, nodeOffset);
    const overlapEnd = Math.min(match.rangeEnd, nodeEnd);

    if (overlapStart >= overlapEnd) continue;

    // Relative positions within this text node
    const relStart = overlapStart - nodeOffset;
    const relEnd = overlapEnd - nodeOffset;

    const parent = textNode.parentNode;
    if (!parent) continue;

    const fragment = document.createDocumentFragment();

    // Text before range
    if (relStart > 0) {
      fragment.appendChild(document.createTextNode(text.slice(0, relStart)));
    }

    // Range portion: create char marks for matched characters, plain text for others
    const rangeText = text.slice(relStart, relEnd);
    const rangeContentNodes: Node[] = [];
    let segStart = 0;

    for (let i = 0; i < rangeText.length; i++) {
      const globalIdx = nodeOffset + relStart + i;
      if (charIndexSet.has(globalIdx)) {
        // Add text before this char mark
        if (i > segStart) {
          const textBefore = document.createTextNode(rangeText.slice(segStart, i));
          fragment.appendChild(textBefore);
          rangeContentNodes.push(textBefore);
        }
        // Add char mark
        const charMark = document.createElement("mark");
        charMark.className = "fuzzy-highlight-char";
        charMark.textContent = rangeText[i];
        fragment.appendChild(charMark);
        rangeContentNodes.push(charMark);
        charMarks.push(charMark);
        segStart = i + 1;
      }
    }
    // Remaining text in range
    if (segStart < rangeText.length) {
      const textAfterChars = document.createTextNode(rangeText.slice(segStart));
      fragment.appendChild(textAfterChars);
      rangeContentNodes.push(textAfterChars);
    }

    // Text after range
    if (relEnd < text.length) {
      fragment.appendChild(document.createTextNode(text.slice(relEnd)));
    }

    // Hide code element background so highlight shows through continuously
    if (parent instanceof HTMLElement && parent.tagName === "CODE") {
      if (!parent.classList.contains("code-in-search-highlight")) {
        parent.classList.add("code-in-search-highlight");
        codeElements.push(parent);
      }
    }

    // Replace the original text node with the fragment
    parent.replaceChild(fragment, textNode);

    // Track range boundaries from the first and last overlapping text nodes
    if (rangeContentNodes.length > 0) {
      if (!rangeStartNode) {
        const first = rangeContentNodes[0];
        if (first.nodeType === Node.TEXT_NODE) {
          rangeStartNode = first;
          rangeStartOffset = 0;
        } else {
          // Element (char mark) - set start before it
          rangeStartNode = first.parentNode;
          rangeStartOffset = indexOf(first);
        }
      }

      const last = rangeContentNodes[rangeContentNodes.length - 1];
      if (last.nodeType === Node.TEXT_NODE) {
        rangeEndNode = last;
        rangeEndOffset = (last.textContent || "").length;
      } else {
        // Element (char mark) - set end after it
        rangeEndNode = last.parentNode;
        rangeEndOffset = indexOf(last) + 1;
      }
    }
  }

  if (!rangeStartNode || !rangeEndNode || charMarks.length === 0) return null;

  // Build the Range for CSS Custom Highlight API
  const range = document.createRange();
  range.setStart(rangeStartNode, rangeStartOffset);
  range.setEnd(rangeEndNode, rangeEndOffset);

  return {
    range,
    scrollTarget: charMarks[0],
    charMarks,
    codeElements,
  };
}

/**
 * Get the index of a child node within its parent's childNodes.
 */
function indexOf(node: Node): number {
  let index = 0;
  let sibling: Node | null = node.previousSibling;
  while (sibling) {
    index++;
    sibling = sibling.previousSibling;
  }
  return index;
}

export function navigate(direction: "next" | "prev"): void {
  const current = navigateToMatch(direction);
  const matches = collectSearchMatches();
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: state.fuzzyMatches.length,
    current,
    query: state.query,
    matches,
    pinnedMatches,
  });
}

export function clear(): void {
  state.query = "";
  state.lastHighlightMatches = [];
  state.pendingNavigateIndex = null;
  clearSearchHighlights();
  const pinnedMatches = collectPinnedMatches();
  callback?.({ count: 0, current: 0, query: "", matches: [], pinnedMatches });
}

/**
 * Set a pending navigate index for the next applyHighlights call.
 * Used by the finder Enter key to navigate to a specific match after
 * highlights are (re-)applied, handling both same-file and cross-file jumps.
 */
export function setNavigateOnApply(index: number): void {
  state.pendingNavigateIndex = index;
}

export function setup(cb: SearchCallback): void {
  callback = cb;
}

/**
 * Re-apply the current search query and pinned searches after DOM changes (e.g., tab switch).
 * This preserves highlights across tab navigation.
 *
 * Uses stored Rust-computed match data (lastHighlightMatches) to ensure
 * highlights appear at the same positions nucleo computed, rather than
 * re-matching with a different algorithm.
 */
export function reapply(): void {
  // Re-apply pinned highlights first
  applyPinnedHighlights();

  // Re-apply search highlights using stored Rust-computed match data.
  // Preserve current position by setting it as pending (only if no pending already set,
  // e.g., from an Enter key jump that hasn't been consumed yet).
  if (state.query && state.lastHighlightMatches.length > 0) {
    if (state.pendingNavigateIndex === null && state.currentIndex >= 0) {
      state.pendingNavigateIndex = state.currentIndex;
    }
    applyHighlights(state.query, state.lastHighlightMatches);
  } else {
    // Just notify with pinned matches
    const pinnedMatches = collectPinnedMatches();
    callback?.({ count: 0, current: 0, query: "", matches: [], pinnedMatches });
  }
}

/**
 * Navigate directly to a specific match by index.
 * Used by the Search tab for clicking on match items.
 */
export function navigateTo(index: number): void {
  if (index < 0 || index >= state.fuzzyMatches.length) {
    return;
  }

  // Update index, refresh Highlight API active state, and scroll
  state.currentIndex = index;
  updateHighlightAPI();
  const entry = state.fuzzyMatches[index];
  entry.scrollTarget.scrollIntoView({ behavior: "smooth", block: "center" });

  // Notify callback with unified format
  const newCurrent = index + 1;
  const matches = collectSearchMatches();
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: state.fuzzyMatches.length,
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
  applyPinnedHighlights();

  // Notify with updated matches
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: state.fuzzyMatches.length,
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
 * Collect context around a Range (text before and after).
 */
function getContextFromRange(
  range: Range,
  maxChars: number,
): { text: string; matchStart: number; matchEnd: number } {
  const matchText = range.toString();

  const before = getTextBeforePosition(range.startContainer, range.startOffset, maxChars);
  const after = getTextAfterPosition(range.endContainer, range.endOffset, maxChars);

  const text = before + matchText + after;
  const matchStart = before.length;
  const matchEnd = matchStart + matchText.length;

  return { text, matchStart, matchEnd };
}

/**
 * Get text content before a (container, offset) position, stopping at newlines.
 */
function getTextBeforePosition(container: Node, offset: number, maxChars: number): string {
  let text = "";

  // Text before offset in start container
  if (container.nodeType === Node.TEXT_NODE) {
    const nodeText = container.textContent || "";
    const beforeText = nodeText.slice(0, offset);
    const newlineIdx = beforeText.lastIndexOf("\n");
    if (newlineIdx !== -1) {
      return beforeText.slice(newlineIdx + 1);
    }
    text = beforeText;
  }

  // Walk backwards through siblings and parent's previous siblings
  let node: Node | null = container;
  outer: while (node && text.length < maxChars) {
    if (node.previousSibling) {
      node = node.previousSibling;
      const content = getNodeTextContent(node);
      const newlineIdx = content.lastIndexOf("\n");
      if (newlineIdx !== -1) {
        text = content.slice(newlineIdx + 1) + text;
        break outer;
      }
      text = content + text;
    } else {
      node = node.parentElement;
      if (node && node.closest(".markdown-body")) {
        continue;
      }
      break;
    }
  }

  if (text.length > maxChars) {
    text = text.slice(-maxChars);
  }

  return text;
}

/**
 * Get text content after a (container, offset) position, stopping at newlines.
 */
function getTextAfterPosition(container: Node, offset: number, maxChars: number): string {
  let text = "";

  // Text after offset in end container
  if (container.nodeType === Node.TEXT_NODE) {
    const nodeText = container.textContent || "";
    const afterText = nodeText.slice(offset);
    const newlineIdx = afterText.indexOf("\n");
    if (newlineIdx !== -1) {
      return afterText.slice(0, newlineIdx);
    }
    text = afterText;
  }

  // Walk forwards through siblings and parent's next siblings
  let node: Node | null = container;
  outer: while (node && text.length < maxChars) {
    if (node.nextSibling) {
      node = node.nextSibling;
      const content = getNodeTextContent(node);
      const newlineIdx = content.indexOf("\n");
      if (newlineIdx !== -1) {
        text = text + content.slice(0, newlineIdx);
        break outer;
      }
      text = text + content;
    } else {
      node = node.parentElement;
      if (node && node.closest(".markdown-body")) {
        continue;
      }
      break;
    }
  }

  if (text.length > maxChars) {
    text = text.slice(0, maxChars);
  }

  return text;
}

/**
 * Collect context around an element (text before and after).
 * Used by pinned search matches.
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
  return node.textContent || "";
}

/**
 * Collect all search match information for the Search tab.
 */
function collectSearchMatches(): SearchMatch[] {
  const matches: SearchMatch[] = [];
  const contextChars = 30;

  for (let i = 0; i < state.fuzzyMatches.length; i++) {
    const entry = state.fuzzyMatches[i];
    const text = entry.range.toString();
    const context = getContextFromRange(entry.range, contextChars);

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

// ── New Rust-driven highlight API ───────────────────────────

/**
 * A single highlight match from Rust search results.
 * Rust computes content indices; JS only renders them.
 */
export interface HighlightMatch {
  /** Source line number (matches data-source-line attribute on DOM blocks) */
  lineNumber: number;
  /** Character indices in rendered content text (pre-converted by Rust) */
  contentIndices: number[];
}

/**
 * Apply highlights based on Rust search results.
 *
 * Receives pre-computed match positions from Rust and places highlights
 * at exact character positions in the DOM.
 *
 * Flow:
 * 1. Find DOM block via data-source-line attribute
 * 2. Collect text nodes from the block
 * 3. Place char-level marks at contentIndices positions
 * 4. Build Range for CSS Custom Highlight API background
 *
 * Also stores the matches in state for reapply() after DOM re-renders.
 */
export function applyHighlights(query: string, matches: HighlightMatch[]): void {
  state.query = query;
  state.lastHighlightMatches = matches;
  clearSearchHighlights();

  const container = document.querySelector(".markdown-body");
  if (!container || !query || matches.length === 0) {
    const pinnedMatches = collectPinnedMatches();
    callback?.({ count: 0, current: 0, query: query || "", matches: [], pinnedMatches });
    return;
  }

  const entries: FuzzyMatchEntry[] = [];

  for (const match of matches) {
    // Find DOM block by data-source-line attribute
    const block = container.querySelector(`[data-source-line="${match.lineNumber}"]`);
    if (!block) continue;

    const { nodes } = collectTextNodes(block as Element);
    if (nodes.length === 0) continue;

    const charIndices = match.contentIndices;
    if (charIndices.length === 0) continue;

    // Build FuzzyBlockMatch compatible with highlightBlock
    const rangeStart = Math.min(...charIndices);
    const rangeEnd = Math.max(...charIndices) + 1;

    const blockMatch: FuzzyBlockMatch = { charIndices, rangeStart, rangeEnd };
    const entry = highlightBlock(nodes, blockMatch);
    if (entry) {
      entries.push(entry);
    }
  }

  state.fuzzyMatches = entries;

  // Determine active match index: use pending navigate if available
  let shouldScroll = false;
  if (state.pendingNavigateIndex !== null) {
    if (state.pendingNavigateIndex < entries.length) {
      // Pending index is valid — consume it and scroll
      state.currentIndex = state.pendingNavigateIndex;
      state.pendingNavigateIndex = null;
      shouldScroll = true;
    } else if (entries.length === 0) {
      // No entries yet (e.g., file switching) — keep pending for next call
    } else {
      // Pending index out of range — fall back to 0, clear pending
      state.currentIndex = 0;
      state.pendingNavigateIndex = null;
    }
  } else {
    state.currentIndex = entries.length > 0 ? 0 : -1;
  }

  updateHighlightAPI();

  // Scroll to active match when navigating from finder Enter key
  if (shouldScroll && state.currentIndex >= 0 && state.currentIndex < entries.length) {
    entries[state.currentIndex].scrollTarget.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
  }

  const searchMatches = collectSearchMatches();
  const pinnedMatches = collectPinnedMatches();
  callback?.({
    count: entries.length,
    current: state.currentIndex >= 0 ? state.currentIndex + 1 : 0,
    query: state.query,
    matches: searchMatches,
    pinnedMatches,
  });
}
