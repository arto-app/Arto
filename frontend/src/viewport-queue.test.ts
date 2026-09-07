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
  // The queue drains on a frame. Running the callback inline keeps the tests
  // able to say "and then the job ran" without a timer in every one of them.
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 0;
  });
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

describe("isDrawing", () => {
  test("reports the block being drawn and what the job writes into it", async () => {
    const element = div();
    const child = document.createElement("span");
    element.append(child);
    const outside = div();

    let release = (): void => {};
    viewportQueue.whenNearViewport(
      element,
      () => new Promise<void>((resolve) => (release = resolve)),
    );
    expect(viewportQueue.isDrawing(element)).toBe(false);

    scrollTo(element);
    expect(viewportQueue.isDrawing(element)).toBe(true);
    expect(viewportQueue.isDrawing(child)).toBe(true);
    expect(viewportQueue.isDrawing(outside)).toBe(false);

    release();
    await viewportQueue.idle();
    expect(viewportQueue.isDrawing(element)).toBe(false);
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

/** Place `element` at `top` in the viewport; jsdom lays nothing out on its own. */
function at(element: HTMLElement, top: number): HTMLElement {
  element.getBoundingClientRect = (): DOMRect =>
    ({ top, bottom: top + 100, height: 100, left: 0, right: 0, width: 0, x: 0, y: top }) as DOMRect;
  return element;
}

/** Let the drain's awaits resolve. */
function drained(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("drain order", () => {
  test("runs the block nearest the viewport first", async () => {
    const far = at(div(), 4000);
    const near = at(div(), 100);
    const order: string[] = [];
    viewportQueue.whenNearViewport(far, () => {
      order.push("far");
    });
    viewportQueue.whenNearViewport(near, () => {
      order.push("near");
    });

    // Both cross the margin in the same batch, which is what a jump looks
    // like. Registration order puts the far one first.
    notify([
      { target: far, isIntersecting: true },
      { target: near, isIntersecting: true },
    ]);
    await drained();

    expect(order).toEqual(["near", "far"]);
  });

  test("drops a job whose block left the near zone before its turn", async () => {
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frames.push(callback);
      return 0;
    });

    const element = at(div(), 100);
    const job = vi.fn();
    viewportQueue.whenNearViewport(element, job);

    // The reader scrolls past between the block becoming eligible and the
    // frame that would have drawn it.
    notify([{ target: element, isIntersecting: true }]);
    notify([{ target: element, isIntersecting: false }]);
    for (const frame of frames) {
      frame(0);
    }
    await drained();

    expect(job).not.toHaveBeenCalled();
    // Still registered: coming back into view must still draw it.
    expect(viewportQueue.isPending(element)).toBe(true);
  });
});
