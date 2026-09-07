import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";

import * as viewportQueue from "./viewport-queue";

/** The observed elements, so a test can decide when one comes into view. */
let observed: Set<Element>;
let notify: (entries: Array<{ target: Element; isIntersecting: boolean }>) => void;
let options: IntersectionObserverInit | undefined;

class FakeIntersectionObserver {
  constructor(
    callback: (entries: Array<{ target: Element; isIntersecting: boolean }>) => void,
    init?: IntersectionObserverInit,
  ) {
    notify = callback;
    options = init;
  }
  observe(element: Element): void {
    observed.add(element);
  }
  unobserve(element: Element): void {
    observed.delete(element);
  }
  disconnect(): void {
    observed.clear();
  }
}

/** Report `element` as having come into view. */
function scrollTo(element: Element): void {
  notify([{ target: element, isIntersecting: true }]);
}

/** A block in the document, which is where the queue expects to find one. */
function div(): HTMLElement {
  const element = document.createElement("div");
  document.body.append(element);
  return element;
}

beforeEach(() => {
  observed = new Set();
  options = undefined;
  viewportQueue.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
});

afterEach(() => {
  viewportQueue.reset();
  document.body.replaceChildren();
  vi.unstubAllGlobals();
});

describe("whenNearViewport", () => {
  test("does not run the job until the element comes into view", async () => {
    const element = div();
    const job = vi.fn();

    viewportQueue.whenNearViewport(element, job);
    expect(job).not.toHaveBeenCalled();
    expect(viewportQueue.isPending(element)).toBe(true);

    scrollTo(element);
    expect(job).toHaveBeenCalledOnce();

    // The element is held until the job returns, so that a batch render
    // triggered by the job's own DOM writes cannot start a second one.
    await viewportQueue.idle();
    expect(viewportQueue.isPending(element)).toBe(false);
  });

  test("runs a job at most once", () => {
    const element = div();
    const job = vi.fn();

    viewportQueue.whenNearViewport(element, job);
    scrollTo(element);
    scrollTo(element);

    expect(job).toHaveBeenCalledOnce();
  });

  test("keeps the first job when an element is registered twice", () => {
    const element = div();
    const first = vi.fn();
    const second = vi.fn();

    viewportQueue.whenNearViewport(element, first);
    viewportQueue.whenNearViewport(element, second);
    scrollTo(element);

    expect(first).toHaveBeenCalledOnce();
    expect(second).not.toHaveBeenCalled();
  });

  test("stops observing an element once its job has run", () => {
    const element = div();

    viewportQueue.whenNearViewport(element, vi.fn());
    expect(observed.has(element)).toBe(true);

    scrollTo(element);
    expect(observed.has(element)).toBe(false);
  });

  test("a failing job does not keep the element queued", async () => {
    const element = div();
    const error = vi.spyOn(console, "error").mockImplementation(() => {});

    viewportQueue.whenNearViewport(element, () => {
      throw new Error("boom");
    });
    scrollTo(element);
    await viewportQueue.idle();

    expect(viewportQueue.isPending(element)).toBe(false);
    error.mockRestore();
  });

  test("does not start a second job while the first is still running", async () => {
    const element = div();
    let release = (): void => {};
    const first = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    const second = vi.fn();

    viewportQueue.whenNearViewport(element, first);
    scrollTo(element);
    expect(first).toHaveBeenCalledOnce();

    // A batch render provoked by the job's own DOM writes sees the block
    // before the job has marked it rendered, and tries to queue it again.
    viewportQueue.whenNearViewport(element, second);
    scrollTo(element);

    release();
    await viewportQueue.idle();

    expect(first).toHaveBeenCalledOnce();
    expect(second).not.toHaveBeenCalled();
  });

  test("runs immediately where the viewport cannot be observed", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    const job = vi.fn();

    viewportQueue.whenNearViewport(div(), job);

    expect(job).toHaveBeenCalledOnce();
  });

  test("skips a job whose element has left the document", () => {
    const element = div();
    const job = vi.fn();

    viewportQueue.whenNearViewport(element, job);
    element.remove();
    scrollTo(element);

    expect(job).not.toHaveBeenCalled();
    expect(viewportQueue.isPending(element)).toBe(false);
  });

  test("observes against the scroll container the document lives in", () => {
    const content = document.createElement("div");
    content.className = "content";
    document.body.append(content);
    const element = document.createElement("div");
    content.append(element);

    viewportQueue.whenNearViewport(element, vi.fn());

    // The margin only expands the observer's own root, so a nested scroller
    // has to be the root or the head start is clipped away.
    expect(options?.root).toBe(content);
  });
});

describe("flush", () => {
  test("runs every waiting job", async () => {
    const jobs = [vi.fn(), vi.fn(), vi.fn()];
    for (const job of jobs) {
      viewportQueue.whenNearViewport(div(), job);
    }
    expect(viewportQueue.pendingCount()).toBe(3);

    await viewportQueue.flush();

    for (const job of jobs) {
      expect(job).toHaveBeenCalledOnce();
    }
    expect(viewportQueue.pendingCount()).toBe(0);
  });

  test("drains work that a job queues while it runs", async () => {
    // A diagram replaces its block, and the replacement wants a copy button.
    const second = vi.fn();
    viewportQueue.whenNearViewport(div(), () => {
      viewportQueue.whenNearViewport(div(), second);
    });

    await viewportQueue.flush();

    expect(second).toHaveBeenCalledOnce();
    expect(viewportQueue.pendingCount()).toBe(0);
  });

  test("waits for an asynchronous job", async () => {
    let done = false;
    viewportQueue.whenNearViewport(div(), async () => {
      await Promise.resolve();
      done = true;
    });

    await viewportQueue.flush();

    expect(done).toBe(true);
  });
});

describe("cancel", () => {
  test("forgets a job for an element that is going away", () => {
    const element = div();
    const job = vi.fn();

    viewportQueue.whenNearViewport(element, job);
    viewportQueue.cancel(element);
    scrollTo(element);

    expect(job).not.toHaveBeenCalled();
    expect(observed.has(element)).toBe(false);
  });
});

describe("prune", () => {
  test("drops the jobs of a document that has been replaced", () => {
    const gone = div();
    const kept = div();
    viewportQueue.whenNearViewport(gone, vi.fn());
    viewportQueue.whenNearViewport(kept, vi.fn());

    gone.remove();
    viewportQueue.prune();

    expect(viewportQueue.isPending(gone)).toBe(false);
    expect(observed.has(gone)).toBe(false);
    expect(viewportQueue.isPending(kept)).toBe(true);
  });
});
