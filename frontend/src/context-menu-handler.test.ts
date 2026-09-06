import { describe, test, expect } from "vitest";
import { extractTableDelimited, escapeDelimitedField, setup } from "./context-menu-handler";

// ============================================================================
// escapeDelimitedField
// ============================================================================

describe("escapeDelimitedField", () => {
  test("returns plain value when no special characters", () => {
    expect(escapeDelimitedField("hello", ",")).toBe("hello");
    expect(escapeDelimitedField("hello", "\t")).toBe("hello");
  });

  test("quotes field containing delimiter", () => {
    expect(escapeDelimitedField("a,b", ",")).toBe('"a,b"');
    expect(escapeDelimitedField("a\tb", "\t")).toBe('"a\tb"');
  });

  test("quotes and escapes double quotes", () => {
    expect(escapeDelimitedField('say "hi"', ",")).toBe('"say ""hi"""');
  });

  test("quotes field containing newline", () => {
    expect(escapeDelimitedField("line1\nline2", ",")).toBe('"line1\nline2"');
  });

  test("quotes field containing carriage return", () => {
    expect(escapeDelimitedField("a\rb", ",")).toBe('"a\rb"');
  });

  test("handles empty string", () => {
    expect(escapeDelimitedField("", ",")).toBe("");
  });

  test("does not quote when delimiter is not present", () => {
    expect(escapeDelimitedField("a,b", "\t")).toBe("a,b");
    expect(escapeDelimitedField("a\tb", ",")).toBe("a\tb");
  });

  test("handles combined special characters", () => {
    expect(escapeDelimitedField('a,"b\n', ",")).toBe('"a,""b\n"');
  });

  test("guards against formula injection with = prefix", () => {
    expect(escapeDelimitedField("=SUM(A1)", ",")).toBe('"\t=SUM(A1)"');
  });

  test("guards against formula injection with + prefix", () => {
    expect(escapeDelimitedField("+1234", ",")).toBe('"\t+1234"');
  });

  test("guards against formula injection with - prefix", () => {
    expect(escapeDelimitedField("-1234", ",")).toBe('"\t-1234"');
  });

  test("guards against formula injection with @ prefix", () => {
    expect(escapeDelimitedField("@mention", ",")).toBe('"\t@mention"');
  });

  test("formula guard also escapes double quotes", () => {
    expect(escapeDelimitedField('=A1+"B"', ",")).toBe('"\t=A1+""B"""');
  });
});

// ============================================================================
// extractTableDelimited
// ============================================================================

function createTable(rows: string[][]): HTMLTableElement {
  const table = document.createElement("table");
  for (const cells of rows) {
    const tr = document.createElement("tr");
    for (const text of cells) {
      const td = document.createElement("td");
      td.textContent = text;
      tr.appendChild(td);
    }
    table.appendChild(tr);
  }
  return table;
}

describe("extractTableDelimited", () => {
  test("converts simple table to CSV", () => {
    const table = createTable([
      ["a", "b", "c"],
      ["1", "2", "3"],
    ]);
    expect(extractTableDelimited(table, ",")).toBe("a,b,c\n1,2,3");
  });

  test("converts simple table to TSV", () => {
    const table = createTable([
      ["a", "b"],
      ["1", "2"],
    ]);
    expect(extractTableDelimited(table, "\t")).toBe("a\tb\n1\t2");
  });

  test("escapes fields containing comma in CSV", () => {
    const table = createTable([["a,b", "c"]]);
    expect(extractTableDelimited(table, ",")).toBe('"a,b",c');
  });

  test("escapes fields containing tab in TSV", () => {
    const table = createTable([["a\tb", "c"]]);
    expect(extractTableDelimited(table, "\t")).toBe('"a\tb"\tc');
  });

  test("escapes fields containing double quotes", () => {
    const table = createTable([['say "hi"', "ok"]]);
    expect(extractTableDelimited(table, ",")).toBe('"say ""hi""",ok');
  });

  test("handles empty cells", () => {
    const table = createTable([
      ["a", "", "c"],
      ["", "b", ""],
    ]);
    expect(extractTableDelimited(table, ",")).toBe("a,,c\n,b,");
  });

  test("handles single cell table", () => {
    const table = createTable([["only"]]);
    expect(extractTableDelimited(table, ",")).toBe("only");
  });

  test("handles table with thead and tbody", () => {
    const table = document.createElement("table");
    const thead = document.createElement("thead");
    const tbody = document.createElement("tbody");

    const headerRow = document.createElement("tr");
    const th1 = document.createElement("th");
    th1.textContent = "Name";
    const th2 = document.createElement("th");
    th2.textContent = "Value";
    headerRow.appendChild(th1);
    headerRow.appendChild(th2);
    thead.appendChild(headerRow);

    const bodyRow = document.createElement("tr");
    const td1 = document.createElement("td");
    td1.textContent = "foo";
    const td2 = document.createElement("td");
    td2.textContent = "bar";
    bodyRow.appendChild(td1);
    bodyRow.appendChild(td2);
    tbody.appendChild(bodyRow);

    table.appendChild(thead);
    table.appendChild(tbody);

    expect(extractTableDelimited(table, ",")).toBe("Name,Value\nfoo,bar");
  });

  test("trims cell whitespace", () => {
    const table = createTable([["  a  ", "  b  "]]);
    expect(extractTableDelimited(table, ",")).toBe("a,b");
  });

  test("handles multibyte characters", () => {
    const table = createTable([
      ["名前", "値"],
      ["太郎", "100"],
    ]);
    expect(extractTableDelimited(table, ",")).toBe("名前,値\n太郎,100");
  });
});

// ============================================================================
// context menu link capture
// ============================================================================

describe("setup", () => {
  test("preserves relative href for normal links", () => {
    document.body.innerHTML = `
      <div class="markdown-body">
        <p><a href="./guide/page.md">Guide</a></p>
      </div>
    `;
    document.caretRangeFromPoint = () => null;

    window.Arto = {
      ...window.Arto,
      contentCursor: {
        ...window.Arto?.contentCursor,
        clearCursor: () => {},
        setFromContextTarget: () => {},
      },
    };

    let captured: unknown = null;
    setup((data) => {
      captured = data;
    });

    const link = document.querySelector("a");
    expect(link).not.toBeNull();

    link?.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: 10,
        clientY: 20,
      }),
    );

    expect(captured).toMatchObject({
      context: { type: "link", href: "./guide/page.md" },
    });
  });
});
