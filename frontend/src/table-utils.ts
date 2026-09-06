/**
 * Shared table utility functions for extracting table data as delimited text or Markdown.
 * Used by both context-menu-handler (pre-capture) and content-cursor (keyboard actions).
 */

/**
 * Convert an HTML table to a delimited string (CSV or TSV).
 * Follows RFC 4180 for CSV escaping: fields containing the delimiter,
 * double quotes, or newlines are enclosed in double quotes, and internal
 * double quotes are escaped by doubling them.
 */
export function extractTableDelimited(table: HTMLTableElement, delimiter: string): string {
  const rows: string[] = [];
  for (const row of table.rows) {
    const cells: string[] = [];
    for (const cell of row.cells) {
      const text = cell.textContent?.trim() ?? "";
      cells.push(escapeDelimitedField(text, delimiter));
    }
    rows.push(cells.join(delimiter));
  }
  return rows.join("\n");
}

/**
 * Escape a field value for delimited output (CSV/TSV).
 *
 * In addition to RFC 4180 quoting (delimiter, quotes, newlines),
 * fields starting with `=`, `+`, `-`, or `@` are wrapped in quotes with
 * a leading tab character inside to prevent formula injection when pasted
 * into spreadsheets.
 */
export function escapeDelimitedField(value: string, delimiter: string): string {
  // Prevent spreadsheet formula injection (CSV Injection / DDE attacks).
  // Cells starting with these characters are interpreted as formulas by
  // Excel, Google Sheets, and LibreOffice Calc.
  const needsFormulaGuard =
    value.length > 0 &&
    (value[0] === "=" || value[0] === "+" || value[0] === "-" || value[0] === "@");

  if (
    needsFormulaGuard ||
    value.includes(delimiter) ||
    value.includes('"') ||
    value.includes("\n") ||
    value.includes("\r")
  ) {
    const escaped = value.replace(/"/g, '""');
    // Tab prefix neutralizes formula interpretation while preserving the value
    return needsFormulaGuard ? `"\t${escaped}"` : `"${escaped}"`;
  }
  return value;
}

/**
 * Convert an HTML table to a Markdown table string with aligned columns.
 */
export function formatTableAsMarkdown(table: HTMLTableElement): string {
  if (table.rows.length === 0) return "";

  // Collect all rows as arrays of cell text
  const allRows: string[][] = [];
  for (const row of table.rows) {
    const cells: string[] = [];
    for (const cell of row.cells) {
      cells.push(cell.textContent?.trim() ?? "");
    }
    allRows.push(cells);
  }

  if (allRows.length === 0) return "";

  // Calculate column widths for alignment
  const colCount = Math.max(...allRows.map((r) => r.length));
  const colWidths: number[] = Array(colCount).fill(3); // minimum 3 for "---"
  for (const row of allRows) {
    for (let i = 0; i < row.length; i++) {
      colWidths[i] = Math.max(colWidths[i], row[i].length);
    }
  }

  const formatRow = (cells: string[]): string => {
    const padded = colWidths.map((w, i) => (cells[i] ?? "").padEnd(w));
    return `| ${padded.join(" | ")} |`;
  };

  const lines: string[] = [];
  lines.push(formatRow(allRows[0]));
  lines.push(`| ${colWidths.map((w) => "-".repeat(w)).join(" | ")} |`);
  for (let i = 1; i < allRows.length; i++) {
    lines.push(formatRow(allRows[i]));
  }

  return lines.join("\n");
}
