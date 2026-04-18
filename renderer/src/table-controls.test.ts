import { beforeEach, describe, expect, test } from "vitest";
import { enhanceTables } from "./table-controls";

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("enhanceTables", () => {
  test("wraps markdown tables with controls and viewport", () => {
    document.body.innerHTML = `
      <div class="markdown-body">
        <table>
          <thead><tr><th>ID</th><th>Name</th></tr></thead>
          <tbody><tr><td>UC-1</td><td>Example</td></tr></tbody>
        </table>
      </div>
    `;

    const container = document.querySelector(".markdown-body")!;
    enhanceTables(container);

    const wrapper = container.querySelector(".table-viewer");
    const viewport = container.querySelector(".table-viewer-viewport");
    const buttons = Array.from(container.querySelectorAll<HTMLButtonElement>(".table-viewer-button"));

    expect(wrapper).not.toBeNull();
    expect(viewport).not.toBeNull();
    expect(viewport?.querySelector("table")).not.toBeNull();
    expect(wrapper?.getAttribute("data-table-mode")).toBe("full");
    expect(buttons.map((button) => button.textContent)).toEqual(["Fixed", "Full"]);
    expect(buttons[1]?.classList.contains("active")).toBe(true);
  });

  test("switches between fixed and full modes", () => {
    document.body.innerHTML = `
      <div class="markdown-body">
        <table>
          <tr><th>ID</th><th>Name</th></tr>
          <tr><td>UC-1</td><td>Example</td></tr>
        </table>
      </div>
    `;

    const container = document.querySelector(".markdown-body")!;
    enhanceTables(container);

    const wrapper = container.querySelector<HTMLElement>(".table-viewer")!;
    const fixedButton = container.querySelector<HTMLButtonElement>('.table-viewer-button[data-mode="fixed"]')!;
    const fullButton = container.querySelector<HTMLButtonElement>('.table-viewer-button[data-mode="full"]')!;

    fixedButton.click();
    expect(wrapper.dataset.tableMode).toBe("fixed");
    expect(fixedButton.getAttribute("aria-pressed")).toBe("true");
    expect(fullButton.getAttribute("aria-pressed")).toBe("false");

    fullButton.click();
    expect(wrapper.dataset.tableMode).toBe("full");
    expect(fullButton.getAttribute("aria-pressed")).toBe("true");
  });

  test("skips frontmatter tables", () => {
    document.body.innerHTML = `
      <div class="markdown-body">
        <details class="frontmatter">
          <table class="frontmatter-table">
            <tr><th>ID</th><td>UC-1</td></tr>
          </table>
        </details>
      </div>
    `;

    const container = document.querySelector(".markdown-body")!;
    enhanceTables(container);

    const table = container.querySelector("table")!;
    expect(container.querySelector(".table-viewer")).toBeNull();
    expect(table.getAttribute("data-table-controls-added")).toBe("skipped");
  });

  test("does not re-wrap a processed table", () => {
    document.body.innerHTML = `
      <div class="markdown-body">
        <table>
          <tr><th>ID</th><th>Name</th></tr>
          <tr><td>UC-1</td><td>Example</td></tr>
        </table>
      </div>
    `;

    const container = document.querySelector(".markdown-body")!;
    enhanceTables(container);
    enhanceTables(container);

    expect(container.querySelectorAll(".table-viewer")).toHaveLength(1);
  });
});
