type TableMode = "fixed" | "full";

const DEFAULT_MODE: TableMode = "full";

/**
 * Wrap markdown tables with a scroll viewport and a fixed/full mode toggle.
 */
export function enhanceTables(container: Element): void {
  const tables = container.querySelectorAll("table:not([data-table-controls-added])");
  if (tables.length === 0) {
    return;
  }

  tables.forEach((table) => {
    if (!(table instanceof HTMLTableElement)) {
      return;
    }

    if (shouldSkipTable(table)) {
      table.dataset.tableControlsAdded = "skipped";
      return;
    }

    addTableControls(table);
  });
}

function shouldSkipTable(table: HTMLTableElement): boolean {
  return (
    table.classList.contains("frontmatter-table") ||
    table.classList.contains("yaml-nested-table") ||
    table.closest(".frontmatter") !== null ||
    table.closest(".table-viewer") !== null
  );
}

function addTableControls(table: HTMLTableElement): void {
  const parent = table.parentElement;
  if (!parent) {
    return;
  }

  table.dataset.tableControlsAdded = "yes";

  const wrapper = document.createElement("div");
  wrapper.className = "table-viewer";

  const toolbar = document.createElement("div");
  toolbar.className = "table-viewer-toolbar";

  const segment = document.createElement("div");
  segment.className = "table-viewer-segment";
  segment.setAttribute("role", "group");
  segment.setAttribute("aria-label", "Table view mode");

  const fixedButton = createModeButton(wrapper, "fixed");
  const fullButton = createModeButton(wrapper, "full");
  segment.append(fixedButton, fullButton);
  toolbar.appendChild(segment);

  const viewport = document.createElement("div");
  viewport.className = "table-viewer-viewport";

  parent.insertBefore(wrapper, table);
  viewport.appendChild(table);
  wrapper.append(toolbar, viewport);

  setMode(wrapper, DEFAULT_MODE);
}

function createModeButton(wrapper: HTMLElement, mode: TableMode): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "table-viewer-button";
  button.dataset.mode = mode;
  button.textContent = mode === "fixed" ? "Fixed" : "Full";
  button.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setMode(wrapper, mode);
  });
  return button;
}

function setMode(wrapper: HTMLElement, mode: TableMode): void {
  wrapper.dataset.tableMode = mode;

  wrapper.querySelectorAll<HTMLButtonElement>(".table-viewer-button").forEach((button) => {
    const isActive = button.dataset.mode === mode;
    button.classList.toggle("active", isActive);
    button.setAttribute("aria-pressed", isActive ? "true" : "false");
  });
}

