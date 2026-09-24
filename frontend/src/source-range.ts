/**
 * The `data-source-range` the Markdown pipeline puts on every block element.
 *
 * The attribute is `L:C-L:C`: 1-based lines of the whole file and 1-based
 * columns in code points, both ends inclusive. The contract lives in the
 * crate docs of `crates/arto-markdown/src/lib.rs`.
 */

export interface SourcePosition {
  line: number;
  column: number;
}

export interface SourceRange {
  start: SourcePosition;
  end: SourcePosition;
}

const RANGE = /^(\d+):(\d+)-(\d+):(\d+)$/;

/** Parse an `L:C-L:C` value, or `null` when it is not one. */
export function parseSourceRange(value: string | undefined): SourceRange | null {
  const match = value === undefined ? null : RANGE.exec(value);
  if (!match) return null;
  const numbers = match.slice(1).map(Number);
  if (!numbers.every((n) => Number.isSafeInteger(n) && n >= 1)) return null;
  const [startLine, startColumn, endLine, endColumn] = numbers;
  if (startLine > endLine || (startLine === endLine && startColumn > endColumn)) return null;
  return {
    start: { line: startLine, column: startColumn },
    end: { line: endLine, column: endColumn },
  };
}

/** The range an element was rendered from, or `null` when it names none. */
export function readSourceRange(el: HTMLElement): SourceRange | null {
  return parseSourceRange(el.dataset.sourceRange);
}
