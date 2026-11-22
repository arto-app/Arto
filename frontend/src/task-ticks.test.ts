import { describe, test, expect, beforeEach, vi } from "vitest";
import { onTick, refresh, setEnabled, show, _internal } from "./task-ticks";
import type { TickRequest } from "./task-ticks";

function page(html: string): HTMLElement {
  document.body.innerHTML = `<div class="markdown-viewer"><article class="markdown-body" data-arto-loaded="/notes/a.md" data-render-generation="7">${html}</article></div>`;
  return document.querySelector("article") as HTMLElement;
}

const TASKS = `<ul>
<li data-source-range="1:1-1:9"><input type="checkbox" disabled> open</li>
<li data-source-range="2:1-2:9"><input type="checkbox" checked disabled> done</li>
</ul>`;

function boxes(): HTMLInputElement[] {
  return Array.from(document.querySelectorAll<HTMLInputElement>("input[type=checkbox]"));
}

function click(input: HTMLInputElement): void {
  input.checked = !input.checked;
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

describe("task ticks", () => {
  let sent: TickRequest[];

  beforeEach(() => {
    _internal.reset();
    sent = [];
    onTick((tick) => sent.push(tick));
  });

  test("checkboxes stay inert until ticking is turned on", () => {
    page(TASKS);
    refresh(document.body);
    expect(boxes().every((box) => box.disabled)).toBe(true);
  });

  test("a tick names where its item starts and what the reader made it", () => {
    page(TASKS);
    setEnabled(true);
    const [open, done] = boxes();
    expect(open.disabled).toBe(false);

    click(open);
    click(done);

    expect(sent).toEqual([
      { generation: 7, line: 1, column: 1, checked: true },
      { generation: 7, line: 2, column: 1, checked: false },
    ]);
  });

  test("a document drawn again is tickable without being asked again", () => {
    setEnabled(true);
    const body = page(TASKS);
    refresh(body);
    expect(boxes().every((box) => !box.disabled)).toBe(true);
  });

  test("turned off, the checkboxes go back to inert and send nothing", () => {
    page(TASKS);
    setEnabled(true);
    setEnabled(false);
    const [open] = boxes();
    expect(open.disabled).toBe(true);

    click(open);
    expect(sent).toEqual([]);
  });

  test("nothing is tickable before the app asks for ticks", () => {
    _internal.reset();
    page(TASKS);
    setEnabled(true);
    expect(boxes().every((box) => box.disabled)).toBe(true);
  });

  test("a checkbox that is not its item's task marker is left alone", () => {
    page(`<ul>
<li data-source-range="1:1-1:20">text <input type="checkbox" disabled></li>
<li><input type="checkbox" disabled> no range</li>
</ul>
<p data-source-range="3:1-3:5"><input type="checkbox" disabled></p>`);
    setEnabled(true);
    expect(boxes().every((box) => box.disabled)).toBe(true);
  });

  test("a task drawn inside something else on the page is left alone", () => {
    const body = page(TASKS);
    // A lens answer or a link's preview: another document's lines.
    const answer = document.createElement("div");
    answer.className = "markdown-body";
    answer.innerHTML = TASKS;
    body.appendChild(answer);
    const preview = document.createElement("div");
    preview.className = "markdown-body";
    preview.innerHTML = TASKS;
    document.body.appendChild(preview);

    setEnabled(true);

    const inert = boxes().filter((box) => box.disabled);
    expect(inert).toHaveLength(4);
    expect(inert.every((box) => box.closest(".markdown-body") !== body)).toBe(true);
  });

  test("a task in a lens's answer standing in for a block is left alone", () => {
    // A translation stands in the document's place, but its items carry the
    // lines of the answer, not of the file.
    page(`<ul data-lens-paired="0" data-source-range="1:1-2:9">
<li data-source-range="1:1-1:9"><input type="checkbox" disabled> ouvert</li>
</ul>`);
    setEnabled(true);
    expect(boxes().every((box) => box.disabled)).toBe(true);
  });

  test("drawing again what is already tickable changes nothing on the page", async () => {
    const body = page(TASKS);
    setEnabled(true);
    const records: MutationRecord[] = [];
    const observer = new MutationObserver((found) => records.push(...found));
    observer.observe(body, { subtree: true, attributes: true });

    refresh(body);
    setEnabled(true);
    await Promise.resolve();
    records.push(...observer.takeRecords());
    observer.disconnect();

    expect(records).toEqual([]);
  });

  test("a tick the app could not place is shown as the file has it", () => {
    page(TASKS);
    setEnabled(true);
    const [open] = boxes();
    click(open);
    expect(open.checked).toBe(true);

    show(7, { line: 1, column: 1 }, false);

    expect(open.checked).toBe(false);
  });

  test("an answer for a page no longer on screen leaves the one there alone", () => {
    page(TASKS);
    setEnabled(true);
    const [open] = boxes();
    click(open);

    show(6, { line: 1, column: 1 }, false);

    expect(open.checked).toBe(true);
  });

  test("the app is not asked twice when a tick is sent", () => {
    const callback = vi.fn();
    _internal.reset();
    onTick(callback);
    page(TASKS);
    setEnabled(true);
    setEnabled(true);
    click(boxes()[0]);
    expect(callback).toHaveBeenCalledTimes(1);
  });
});
