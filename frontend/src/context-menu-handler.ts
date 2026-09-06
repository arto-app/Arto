/**
 * Context menu handler for markdown content viewer.
 * Detects the type of element that was right-clicked and reports to Rust.
 */

import { extractTableDelimited, formatTableAsMarkdown } from "./table-utils";

export type ContentContextType =
  | { type: "general" }
  | { type: "link"; href: string }
  | { type: "image"; src: string; alt: string | null }
  | {
      type: "code_block";
      content: string;
      language: string | null;
      source_line: number | null;
      source_line_end: number | null;
    }
  | { type: "mermaid"; source: string }
  | { type: "math_block"; source: string };

export interface ContextMenuData {
  context: ContentContextType;
  x: number;
  y: number;
  has_selection: boolean;
  selected_text: string;
  source_line: number | null;
  source_line_end: number | null;
  table_csv: string | null;
  table_tsv: string | null;
  table_markdown: string | null;
  table_source_line: number | null;
  table_source_line_end: number | null;
}

interface TableData {
  csv: string;
  tsv: string;
  markdown: string;
  sourceLine: number | null;
  sourceLineEnd: number | null;
}

/**
 * Explicit position within the DOM for code block line computation.
 */
interface CodePosition {
  container: Node;
  offset: number;
}

/**
 * Find the source line number by walking up the DOM tree to find
 * the nearest ancestor with a data-source-line attribute.
 *
 * For code blocks (<pre> with data-source-line-start), computes the
 * exact line by counting newlines from the start of the code content
 * to the given position. This is necessary because highlight.js
 * replaces <code> innerHTML, destroying any per-line annotations.
 *
 * @param position - Explicit position for code block offset calculation.
 *   Required for accurate line detection within code blocks.
 */
function findSourceLine(node: Node | null, position?: CodePosition): number | null {
  let current: Node | null = node;
  while (current && current !== document.body) {
    if (current instanceof HTMLElement) {
      // Check for code block with per-line start info
      const lineStart = current.dataset.sourceLineStart;
      if (lineStart !== undefined) {
        const startLine = parseInt(lineStart, 10);
        if (!isNaN(startLine)) {
          const offset = position ? computeCodeBlockLineOffset(current, position) : 0;
          return startLine + offset;
        }
      }

      const line = current.dataset.sourceLine;
      if (line !== undefined) {
        const parsed = parseInt(line, 10);
        if (!isNaN(parsed)) return parsed;
      }
      // Stop at markdown-body boundary
      if (current.classList.contains("markdown-body")) return null;
    }
    current = current.parentNode;
  }
  return null;
}

/**
 * Compute the line offset within a code block by counting newlines
 * from <code> start to the given position.
 */
function computeCodeBlockLineOffset(preElement: HTMLElement, position: CodePosition): number {
  const codeEl = preElement.querySelector("code");
  if (!codeEl || !codeEl.contains(position.container)) return 0;

  const range = document.createRange();
  range.setStart(codeEl, 0);
  range.setEnd(position.container, position.offset);

  const textBefore = range.toString();
  let count = 0;
  for (const ch of textBefore) {
    if (ch === "\n") count++;
  }
  return count;
}

/**
 * Find the deepest last descendant of a node.
 */
function deepestLastChild(node: Node): Node {
  let current = node;
  while (current.lastChild) {
    current = current.lastChild;
  }
  return current;
}

/**
 * Get source line range from the current selection or click target.
 * Returns [startLine, endLine] where both may be the same (single line)
 * or null if no source line could be determined.
 *
 * @param clientX - Mouse X coordinate for caret detection without selection
 * @param clientY - Mouse Y coordinate for caret detection without selection
 */
function getSourceLineRange(
  target: HTMLElement,
  clientX: number,
  clientY: number,
): {
  sourceLine: number | null;
  sourceLineEnd: number | null;
} {
  const selection = window.getSelection();
  if (selection && selection.rangeCount > 0 && !selection.isCollapsed) {
    const range = selection.getRangeAt(0);
    const startPosition: CodePosition = {
      container: range.startContainer,
      offset: range.startOffset,
    };
    const startLine = findSourceLine(range.startContainer, startPosition);

    // When endOffset is 0 and endContainer differs from startContainer,
    // the selection ends at the very start of endContainer — no content
    // from it is actually selected. This commonly happens with double-click
    // word selection where the browser extends the range to the start of
    // the next element. Walk backward to find the real end node.
    let endNode: Node = range.endContainer;
    let endPosition: CodePosition;
    if (range.endOffset === 0 && endNode !== range.startContainer) {
      const prev = endNode.previousSibling;
      if (prev) {
        endNode = deepestLastChild(prev);
        // Use end of the adjusted text node
        endPosition = {
          container: endNode,
          offset: endNode.nodeType === Node.TEXT_NODE ? (endNode.textContent?.length ?? 0) : 0,
        };
      } else if (endNode.parentNode) {
        endNode = endNode.parentNode;
        endPosition = { container: endNode, offset: 0 };
      } else {
        endPosition = { container: endNode, offset: 0 };
      }
    } else {
      endPosition = {
        container: range.endContainer,
        offset: range.endOffset,
      };
    }

    const endLine = findSourceLine(endNode, endPosition);
    return {
      sourceLine: startLine,
      sourceLineEnd: endLine,
    };
  }

  // No selection — use caret position from mouse coordinates
  const caretPosition = getCaretPositionFromPoint(clientX, clientY);
  const line = findSourceLine(target, caretPosition ?? undefined);
  return {
    sourceLine: line,
    sourceLineEnd: line,
  };
}

/**
 * Get caret position at the given screen coordinates.
 * Uses caretRangeFromPoint (WebKit) to convert mouse position to DOM position.
 */
function getCaretPositionFromPoint(clientX: number, clientY: number): CodePosition | null {
  const range = document.caretRangeFromPoint(clientX, clientY);
  if (!range) return null;
  return {
    container: range.startContainer,
    offset: range.startOffset,
  };
}

/**
 * Result of detecting the context of a right-click.
 * Block elements (mermaid, math, image) include their source line range
 * so that Copy Path items work for entire blocks.
 */
interface DetectedContext {
  context: ContentContextType;
  /** Block start line (null = use selection-based line detection) */
  sourceLine: number | null;
  /** Block end line */
  sourceLineEnd: number | null;
}

/**
 * Read data-source-line and data-source-line-end from an element.
 */
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

/**
 * Detect the context of a right-click by walking up the DOM tree.
 * For block elements, also returns the source line range from data attributes.
 */
function detectContext(target: HTMLElement): DetectedContext {
  let current: HTMLElement | null = target;

  while (current && !current.classList.contains("markdown-body")) {
    // Check for mermaid diagram
    if (current.classList.contains("preprocessed-mermaid")) {
      savedMermaidElement = current;
      const source = current.dataset.originalContent || "";
      const range = readSourceLineRange(current);
      return {
        context: { type: "mermaid", source },
        sourceLine: range.start,
        sourceLineEnd: range.end,
      };
    }

    // Check for math block (preprocessed-math code block or preprocessed-math-display)
    if (
      current.classList.contains("preprocessed-math") ||
      current.classList.contains("preprocessed-math-display")
    ) {
      savedMathElement = current;
      const source = current.dataset.originalContent || "";
      const range = readSourceLineRange(current);
      return {
        context: { type: "math_block", source },
        sourceLine: range.start,
        sourceLineEnd: range.end,
      };
    }

    // Check for code block (pre > code)
    if (current.tagName === "PRE" && current.querySelector("code")) {
      const codeEl = current.querySelector("code");
      const content = codeEl?.textContent || "";
      const language = extractLanguage(codeEl);
      const range = readSourceLineRange(current);
      return {
        context: {
          type: "code_block",
          content,
          language,
          source_line: range.start,
          source_line_end: range.end,
        },
        // Return null to let selection-based line detection handle Copy Path
        sourceLine: null,
        sourceLineEnd: null,
      };
    }

    // Check for inline code that's part of a code block
    if (current.tagName === "CODE" && current.parentElement?.tagName === "PRE") {
      const content = current.textContent || "";
      const language = extractLanguage(current);
      const range = readSourceLineRange(current.parentElement);
      return {
        context: {
          type: "code_block",
          content,
          language,
          source_line: range.start,
          source_line_end: range.end,
        },
        sourceLine: null,
        sourceLineEnd: null,
      };
    }

    // Check for image
    if (current.tagName === "IMG") {
      const img = current as HTMLImageElement;
      // Image is inline within <p data-source-line="N">, use parent's line
      const line = findSourceLine(current);
      return {
        context: { type: "image", src: img.src, alt: img.alt || null },
        sourceLine: line,
        sourceLineEnd: line,
      };
    }

    // Check for link (but not markdown-link which is handled differently)
    if (current.tagName === "A" && !current.classList.contains("markdown-link")) {
      const anchor = current as HTMLAnchorElement;
      return {
        context: { type: "link", href: anchor.getAttribute("href") || "" },
        sourceLine: null,
        sourceLineEnd: null,
      };
    }

    // Check for markdown-link (internal links converted by Rust)
    if (current.classList.contains("markdown-link")) {
      const href = current.getAttribute("data-path") || "";
      return {
        context: { type: "link", href },
        sourceLine: null,
        sourceLineEnd: null,
      };
    }

    current = current.parentElement;
  }

  return { context: { type: "general" }, sourceLine: null, sourceLineEnd: null };
}

/**
 * Detect if the right-click target is inside a table element.
 * Returns table data (CSV/TSV) and source line range, or null if not in a table.
 */
function detectTable(target: HTMLElement): TableData | null {
  const table = target.closest("table") as HTMLTableElement | null;
  if (!table) return null;

  const range = readSourceLineRange(table);
  return {
    csv: extractTableDelimited(table, ","),
    tsv: extractTableDelimited(table, "\t"),
    markdown: formatTableAsMarkdown(table),
    sourceLine: range.start,
    sourceLineEnd: range.end,
  };
}

/**
 * Extract language from code element's class
 */
function extractLanguage(codeEl: HTMLElement | null): string | null {
  if (!codeEl) return null;

  // Look for language-* class
  for (const cls of codeEl.classList) {
    if (cls.startsWith("language-")) {
      return cls.replace("language-", "");
    }
  }

  return null;
}

// Saved selection range for restoration after menu closes
let savedRange: Range | null = null;

// Saved element references for special blocks (Mermaid/Math)
let savedMermaidElement: HTMLElement | null = null;
let savedMathElement: HTMLElement | null = null;

/**
 * Get the current text selection and save the range for later restoration
 */
function getTextSelection(): { hasSelection: boolean; selectedText: string } {
  const selection = window.getSelection();
  const selectedText = selection?.toString() ?? "";

  // Save the range for restoration
  if (selection && selection.rangeCount > 0) {
    savedRange = selection.getRangeAt(0).cloneRange();
  } else {
    savedRange = null;
  }

  return {
    hasSelection: selectedText.length > 0,
    selectedText,
  };
}

/**
 * Restore the previously saved selection
 */
export function restoreSelection(): void {
  if (savedRange) {
    const selection = window.getSelection();
    if (selection) {
      selection.removeAllRanges();
      selection.addRange(savedRange);
    }
  }
}

const MENU_MARGIN = 8;

/**
 * Observe for context menu appearing and adjust its position to stay within viewport.
 * Uses MutationObserver to detect when menu is added to DOM, then measures and repositions.
 */
function setupMenuPositionAdjuster(): void {
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      for (const node of mutation.addedNodes) {
        if (node instanceof HTMLElement && node.classList.contains("content-context-menu")) {
          adjustMenuPosition(node);
          return;
        }
      }
    }
  });

  observer.observe(document.body, { childList: true, subtree: true });
}

/**
 * Adjust menu position based on its actual rendered size.
 */
function adjustMenuPosition(menu: HTMLElement): void {
  const rect = menu.getBoundingClientRect();
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  let newLeft: number | null = null;
  let newTop: number | null = null;

  // Flip horizontally if menu overflows right edge
  if (rect.right + MENU_MARGIN > vw) {
    // Move menu to open left of its current right edge
    newLeft = Math.max(MENU_MARGIN, rect.left - rect.width);
  }

  // Flip vertically if menu overflows bottom edge
  if (rect.bottom + MENU_MARGIN > vh) {
    // Move menu to open above its current bottom edge
    newTop = Math.max(MENU_MARGIN, rect.top - rect.height);
  }

  // Apply adjustments
  if (newLeft !== null) {
    menu.style.left = `${newLeft}px`;
  }
  if (newTop !== null) {
    menu.style.top = `${newTop}px`;
  }
}

// Initialize the position adjuster
setupMenuPositionAdjuster();
let clearCursorOnClickInitialized = false;

/**
 * Setup context menu event listener on the markdown viewer
 */
export function setup(sendToRust: (data: ContextMenuData) => void): void {
  // Clear keyboard content cursor when user switches back to mouse interaction.
  if (!clearCursorOnClickInitialized) {
    document.addEventListener(
      "click",
      (event) => {
        const target = event.target as HTMLElement | null;
        if (!target?.closest(".markdown-body")) return;
        window.Arto?.contentCursor?.clearCursor?.();
      },
      true,
    );
    clearCursorOnClickInitialized = true;
  }

  // Find the markdown body element
  const handler = (event: MouseEvent) => {
    const target = event.target as HTMLElement;

    // Only handle right-clicks within markdown-body
    const markdownBody = target.closest(".markdown-body");
    if (!markdownBody) return;

    // Prevent default browser context menu
    event.preventDefault();

    // Keep content cursor aligned with the context-menu target so that
    // context-menu actions and keyboard actions operate on the same element.
    window.Arto?.contentCursor?.setFromContextTarget?.(target);

    // Detect context and send to Rust
    // Position adjustment is handled by MutationObserver after menu renders
    const { context, sourceLine: blockLine, sourceLineEnd: blockLineEnd } = detectContext(target);
    const { hasSelection, selectedText } = getTextSelection();
    const tableData = detectTable(target);

    // Block elements override selection-based line detection
    let sourceLine: number | null;
    let sourceLineEnd: number | null;
    if (blockLine !== null) {
      sourceLine = blockLine;
      sourceLineEnd = blockLineEnd ?? blockLine;
    } else {
      ({ sourceLine, sourceLineEnd } = getSourceLineRange(target, event.clientX, event.clientY));
    }

    const data: ContextMenuData = {
      context,
      x: event.clientX,
      y: event.clientY,
      has_selection: hasSelection,
      selected_text: selectedText,
      source_line: sourceLine,
      source_line_end: sourceLineEnd,
      table_csv: tableData?.csv ?? null,
      table_tsv: tableData?.tsv ?? null,
      table_markdown: tableData?.markdown ?? null,
      table_source_line: tableData?.sourceLine ?? null,
      table_source_line_end: tableData?.sourceLineEnd ?? null,
    };

    sendToRust(data);
  };

  // Use capture phase to intercept before other handlers
  document.addEventListener("contextmenu", handler, true);
}

/**
 * Cleanup saved element references when context menu closes.
 */
export function cleanupElementReferences(): void {
  savedMermaidElement = null;
  savedMathElement = null;
}

/**
 * Get saved Mermaid element for rasterization.
 */
export function getSavedMermaidElement(): HTMLElement | null {
  return savedMermaidElement;
}

/**
 * Get saved Math element for rasterization.
 */
export function getSavedMathElement(): HTMLElement | null {
  return savedMathElement;
}

/** @internal - re-exported from table-utils for testing */
export { extractTableDelimited, escapeDelimitedField } from "./table-utils";
