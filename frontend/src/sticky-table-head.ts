/**
 * A long table's header row, kept at the top of the view while it is read.
 *
 * Primer draws every table as a block that scrolls on both axes, so the
 * table itself is the scroll container a `position: sticky` header would
 * hold against — and a table never scrolls vertically, so the header would
 * never move. The stylesheet (`sticky-table-head.css`) takes that box away
 * from the tables marked here, which makes the header hold against the
 * document's own scroller instead.
 *
 * Only a table that fits across the page can give up its box: one that is
 * wider needs it to scroll sideways, and its header stays where it is.
 */

import { renderCoordinator } from "./render-coordinator";
import { scrollContainer } from "./scroll-destination";

/** The attribute the stylesheet reads, on each table whose header sticks. */
export const STICKY_HEAD = "data-sticky-head";

/** What deciding a table needs to know about it, measured. */
export interface TableGeometry {
  /**
   * Rows in the header. Only a header of one row is pinned: every row would
   * hold at the same top, one over the other.
   */
  headRows: number;
  /** Inside the frontmatter block, which has a table layout of its own. */
  inFrontmatter: boolean;
  scrollWidth: number;
  clientWidth: number;
  /** As drawn, so that zoom counts. */
  height: number;
  /** The scroller's visible height. */
  viewportHeight: number;
}

/**
 * Whether a table's header row should stay in view while it is read.
 *
 * A table that fits in the view is never read without its header on screen,
 * and pinning one would only make its header shift for the moment it takes
 * to scroll past.
 */
export function shouldStickHead(table: TableGeometry): boolean {
  return (
    table.headRows === 1 &&
    !table.inFrontmatter &&
    table.scrollWidth <= table.clientWidth &&
    table.height > table.viewportHeight
  );
}

/**
 * Mark or unmark every table under `root` for a view `viewportHeight` tall.
 *
 * Returns the tables it looked at.
 */
export function classifyTables(root: ParentNode, viewportHeight: number): HTMLTableElement[] {
  const tables = [...root.querySelectorAll<HTMLTableElement>("table")];
  for (const table of tables) {
    const sticks = shouldStickHead({
      // Every row the stylesheet pins, so a second `thead` counts as well.
      headRows: table.querySelectorAll(":scope > thead > tr").length,
      inFrontmatter: table.closest(".frontmatter") !== null,
      scrollWidth: table.scrollWidth,
      clientWidth: table.clientWidth,
      height: table.getBoundingClientRect().height,
      viewportHeight,
    });
    table.toggleAttribute(STICKY_HEAD, sticks);
  }
  return tables;
}

/**
 * What a resize can change the answer for: the scroller (the view's height),
 * the document and each table (how wide and tall they are drawn).
 */
let observer: ResizeObserver | null = null;
let observed = new Set<Element>();

/**
 * Watch exactly `elements`.
 *
 * Only the difference is (un)observed: observing an element reports its size
 * at once, and re-observing all of them on every report would never settle.
 */
function watch(elements: Element[]): void {
  if (typeof ResizeObserver === "undefined") {
    return;
  }
  observer ??= new ResizeObserver(() => refresh());
  const next = new Set(elements);
  for (const element of observed) {
    if (!next.has(element)) {
      observer.unobserve(element);
    }
  }
  for (const element of next) {
    if (!observed.has(element)) {
      observer.observe(element);
    }
  }
  observed = next;
}

let scheduled = false;

function update(): void {
  scheduled = false;
  const scroller = scrollContainer();
  // The one under the scroller: a lens's answer in the header is rendered
  // Markdown too, and comes first in the document.
  const body = scroller?.querySelector<HTMLElement>(".markdown-body");
  if (!scroller || !body) {
    watch([]);
    return;
  }
  const tables = classifyTables(body, scroller.clientHeight);
  watch([scroller, body, ...tables]);
}

/**
 * Decide again which tables keep their header in view, once per frame.
 *
 * A render, a resize and a table growing ask for this on their own. Zoom does
 * not — it changes how tall a table is drawn without changing its layout
 * size, which is what an observer sees — so the app asks when it zooms.
 */
export function refresh(): void {
  if (scheduled) {
    return;
  }
  scheduled = true;
  requestAnimationFrame(update);
}

export function setup(): void {
  // Marking a table only changes how it is drawn, and it happens after a
  // render; counted as new content, it would ask for another.
  renderCoordinator.ignoreAttribute(STICKY_HEAD);
  // A new document brings new tables. The callback fires once, so it re-arms
  // itself for the render after this one.
  const afterRender = (): void => {
    refresh();
    renderCoordinator.onRenderComplete(afterRender);
  };
  renderCoordinator.onRenderComplete(afterRender);
  // The observers see the scroller's box, which in a page `arto page` wrote
  // is the document as tall as it is, not the window it is seen through.
  window.addEventListener("resize", refresh);
  refresh();
}
