/**
 * The whole of a name a row had to cut, floating beside it.
 *
 * Only when it *was* cut: a note repeating what the row already shows in full
 * is one more thing appearing under the pointer for nothing. And only the
 * name — the row's own text, on one line, as wide as it needs — because that
 * is what the reader was reading when it ran out of room.
 *
 * It is drawn at the top of the document rather than inside the list, because
 * the list scrolls and a scrolling box clips what is positioned in it — which
 * would be the rows nearest its edges, where a cut-off note is least use.
 */

/** Room kept between the note and the edge of the window. */
const MARGIN = 8;

/** The rows that can carry one, and the element holding the name in each. */
const ROW = ".left-sidebar-tree-node-content";
const LABEL = ".left-sidebar-tree-label";

let note: HTMLElement | null = null;
let shown: Element | null = null;

function element(): HTMLElement {
  if (note) {
    return note;
  }
  const created = document.createElement("div");
  created.className = "row-hover";
  created.hidden = true;
  document.body.appendChild(created);
  note = created;
  return created;
}

/** Whether the row had to cut this label to fit. */
function truncated(label: HTMLElement): boolean {
  return label.scrollWidth > label.clientWidth + 1;
}

function place(row: HTMLElement, label: HTMLElement): void {
  const tip = element();
  tip.textContent = label.textContent;
  tip.hidden = false;

  const rect = label.getBoundingClientRect();
  const width = Math.min(tip.offsetWidth, window.innerWidth - MARGIN * 2);
  tip.style.maxWidth = `${window.innerWidth - MARGIN * 2}px`;

  // Over the name it completes, so the eye does not have to travel: same left
  // edge, and below the row unless there is no room there.
  const height = tip.offsetHeight;
  const below = row.getBoundingClientRect().bottom + 4;
  const top = below + height + MARGIN > window.innerHeight ? rect.top - height - 4 : below;
  const left = Math.max(MARGIN, Math.min(rect.left, window.innerWidth - width - MARGIN));

  tip.style.top = `${Math.max(MARGIN, top)}px`;
  tip.style.left = `${left}px`;
}

function hide(): void {
  shown = null;
  if (note) {
    note.hidden = true;
  }
}

function onOver(event: MouseEvent): void {
  const target = event.target;
  if (!(target instanceof Element)) {
    return;
  }
  const row = target.closest<HTMLElement>(ROW);
  const label = row?.querySelector<HTMLElement>(LABEL);
  if (!row || !label || !truncated(label)) {
    hide();
    return;
  }
  if (row === shown) {
    return;
  }
  shown = row;
  place(row, label);
}

export function setupRowHover(): void {
  document.addEventListener("mouseover", onOver);
  document.addEventListener("mouseleave", hide);
  // A row that moves out from under the note takes the note with it.
  document.addEventListener("scroll", hide, { capture: true, passive: true });
  window.addEventListener("resize", hide, { passive: true });
}
