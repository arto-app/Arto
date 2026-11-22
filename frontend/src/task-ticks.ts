/**
 * Task list checkboxes the reader can tick, written back to the file by the
 * app.
 *
 * The renderer draws every task's checkbox `disabled`, which is what a page
 * written by `arto page` keeps. The app asks for ticks with `onTick` and
 * turns them on with `setEnabled`; only then are the checkboxes of the
 * document's own body made clickable. A tick names where its list item
 * starts in the source, and the app checks the file still holds the task
 * there before writing.
 */

import { readSourceRange } from "./source-range";
import type { SourcePosition } from "./source-range";

export interface Tick extends SourcePosition {
  checked: boolean;
}

/** A tick, and the render of the page it was made on (`data-render-generation`). */
export interface TickRequest extends Tick {
  generation: number;
}

/** The article the document itself is drawn in, not a preview or an answer in it. */
const DOCUMENT_BODY = "article.markdown-body[data-arto-loaded]";
/** Marks the checkboxes this module made clickable, to put back when turned off. */
const TICKABLE = "data-arto-tickable";
/**
 * What a lens shows in place of a block (`PAIRED` in `lenses.ts`): its items
 * carry the lines of the answer, not of the file.
 */
const LENS_ANSWER = "[data-lens-paired]";

/**
 * The attributes making a task tickable writes, none of which is new content
 * for the render coordinator to render again.
 */
export const ATTRIBUTES = ["disabled", TICKABLE] as const;

let send: ((tick: TickRequest) => void) | null = null;
let enabled = false;
let listening = false;

/** Register how a tick is handed to the app. */
export function onTick(callback: (tick: TickRequest) => void): void {
  send = callback;
  if (!listening) {
    document.addEventListener("change", handleChange);
    listening = true;
  }
  refresh(document);
}

/** Turn ticking on or off, for what is drawn now and what is drawn next. */
export function setEnabled(on: boolean): void {
  enabled = on;
  refresh(document);
}

/** Make the tasks under `root` clickable, or inert again, as ticking is now. */
export function refresh(root: ParentNode): void {
  const active = enabled && send !== null;
  for (const input of root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')) {
    // Only what changes is written: a render pass calls this, and an
    // attribute written again, even to the value it has, reads to the render
    // coordinator as new content to render.
    if (active && positionOf(input)) {
      if (input.hasAttribute(TICKABLE)) continue;
      input.disabled = false;
      input.setAttribute(TICKABLE, "");
    } else if (input.hasAttribute(TICKABLE)) {
      input.disabled = true;
      input.removeAttribute(TICKABLE);
    }
  }
}

/**
 * Show the task whose item starts at `at` as `checked` — what the file says —
 * on the page of render `generation`, if that page is still the one drawn.
 */
export function show(generation: number, at: SourcePosition, checked: boolean): void {
  const body = document.querySelector<HTMLElement>(DOCUMENT_BODY);
  if (!body || body.dataset.renderGeneration !== String(generation)) return;
  for (const input of body.querySelectorAll<HTMLInputElement>(`input[${TICKABLE}]`)) {
    const position = positionOf(input);
    if (position && position.line === at.line && position.column === at.column) {
      input.checked = checked;
    }
  }
}

function handleChange(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement) || !input.hasAttribute(TICKABLE)) return;
  const position = enabled && send ? positionOf(input) : null;
  const generation = Number(input.closest<HTMLElement>(DOCUMENT_BODY)?.dataset.renderGeneration);
  if (!position || !send || !Number.isSafeInteger(generation)) return;
  send({ generation, ...position, checked: input.checked });
}

/**
 * Where the list item `input` is the task marker of starts, or `null` when
 * it is not one in the document's own body.
 *
 * The renderer writes the marker's checkbox as the item's first node, so a
 * checkbox anywhere else is the document's own HTML.
 */
function positionOf(input: HTMLInputElement): SourcePosition | null {
  if (input.type !== "checkbox") return null;
  const item = input.parentElement;
  if (!(item instanceof HTMLLIElement) || item.firstChild !== input) return null;
  const body = item.closest(".markdown-body");
  if (!body || !body.matches(DOCUMENT_BODY)) return null;
  if (item.closest(LENS_ANSWER)) return null;
  return readSourceRange(item)?.start ?? null;
}

/** @internal */
export const _internal = {
  reset(): void {
    if (listening) {
      document.removeEventListener("change", handleChange);
    }
    send = null;
    enabled = false;
    listening = false;
  },
};
