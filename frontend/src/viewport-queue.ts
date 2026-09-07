/**
 * Defer per-element rendering until the element comes near the viewport.
 *
 * A document costs as much to render as it has constructs in it, but a reader
 * sees one screen at a time. Rendering every diagram, formula and code block
 * up front puts all of it on the critical path: on a megabyte of Markdown that
 * is a single task of about two seconds, and the window does not respond for
 * its duration. Registering the work here spreads it across scrolling, and
 * work for a screen the reader never reaches is never done at all.
 *
 * The observer decides which blocks are *eligible* — several screens either
 * way, so that reading arrives at content already rendered. It does not decide
 * what runs. Reading and jumping look the same to an observer, and running
 * every block that crosses the margin is only right for the first: a jump
 * crosses the whole margin at once, and drawing all of it competes for the
 * frame with the screen the reader actually landed on, having been queued for
 * screens they flew past and will never see.
 *
 * So eligibility is a set, and a drain decides the order: nearest to the
 * viewport first, a few milliseconds per frame, re-checking before each job
 * that its block is still near. Work for a screen that has been left behind is
 * dropped rather than finished. Printing needs the whole document at once and
 * calls [`flush`] instead.
 */

/** How far outside the viewport a block becomes eligible. Three screens. */
const ROOT_MARGIN = "300% 0px";

/**
 * How long a drain runs before yielding to the next frame.
 *
 * The point is not to finish the queue quickly — it is to never hold the main
 * thread long enough to drop a frame while the reader is scrolling.
 */
const FRAME_BUDGET_MS = 8;

/**
 * The scroll container the app puts the document in.
 *
 * `rootMargin` only expands the observer's own root: a target is first
 * clipped against every intervening scroll container, unexpanded, and only
 * then intersected with the margin-expanded root. With the implicit
 * (viewport) root and the document scrolling inside `.content`, the margin
 * would therefore buy nothing and every job would start at the moment its
 * block became visible. Where there is no such container — the standalone
 * page renderer, where the document itself scrolls — the implicit root is
 * the right one.
 */
const SCROLLER = ".content";

/** How many times [`flush`] re-drains a queue that jobs keep refilling. */
const MAX_FLUSH_ROUNDS = 20;

type Job = () => void | Promise<void>;

const pending = new Map<Element, Job>();
/**
 * The elements the observer currently reports as near the viewport.
 *
 * Membership is not a promise that the job will run — only that it may. An
 * element that leaves before the drain reaches it is removed and goes back to
 * waiting, which is how work for a screen the reader flew past is never done.
 */
const eligible = new Set<Element>();
/**
 * The elements whose job has started and not yet finished.
 *
 * A job that awaits — every Mermaid one does — lets the batch render that its
 * own DOM writes scheduled see the block again before it is marked rendered.
 * Holding the element here until the job returns is what stops that batch
 * from starting a second job on it.
 */
const running = new Set<Element>();
let observer: IntersectionObserver | null = null;
/** The root `observer` was built against, to notice when it stops being ours. */
let observerRoot: Element | null = null;

/** Resolvers waiting for the jobs that have started to finish. */
let idleWaiters: Array<() => void> = [];

/** Told whether any job is running; see [`setActivityListener`]. */
let activityListener: ((active: boolean) => void) | null = null;

/**
 * Watch whether the queue is drawing.
 *
 * Drawing writes to the DOM, and whatever watches the document for changes
 * has to be able to tell those writes apart from an edit to the document
 * itself. Without that, one block drawn while scrolling looks like new
 * content and costs a pass over the whole document.
 */
export function setActivityListener(listener: ((active: boolean) => void) | null): void {
  activityListener = listener;
}

/**
 * Whether `node` is inside a block the queue is drawing right now.
 *
 * A job only ever writes within the block it was registered for, so a
 * mutation that stays inside one is the renderer's own output rather than
 * content that has to be rendered again. Knowing merely *that* the queue is
 * drawing is not enough to tell the two apart: the writes still have to be
 * recorded as deferred, and are then replayed as a pass over the whole
 * document per drawn block — the cost this file exists to avoid.
 */
export function isDrawing(node: Node): boolean {
  // Walked upwards from the node rather than across `running`: a flush holds
  // every block of the document at once, and asking each of them whether it
  // contains the node makes one observer batch cost the document squared —
  // during printing, which is when the flush happens.
  for (let current: Node | null = node; current; current = current.parentNode) {
    if (running.has(current as Element)) {
      return true;
    }
  }
  return false;
}

function observerOrNull(): IntersectionObserver | null {
  if (typeof IntersectionObserver === "undefined") {
    return null;
  }
  const root = document.querySelector(SCROLLER);
  // An observer's root is fixed for its life, so a mismatch has to be rebuilt
  // rather than adjusted. Registering a block before `.content` exists would
  // otherwise pin the viewport as the root for the rest of the process, and
  // the head start would be silently clipped away for every document after.
  if (observer && observerRoot !== root) {
    observer.disconnect();
    observer = null;
  }
  if (!observer) {
    observerRoot = root;
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            eligible.add(entry.target);
          } else {
            eligible.delete(entry.target);
          }
        }
        scheduleDrain();
      },
      { root, rootMargin: ROOT_MARGIN },
    );
    for (const element of pending.keys()) {
      observer.observe(element);
    }
  }
  return observer;
}

function now(): number {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}

/** The bounds jobs are ranked against: the part of the document on screen. */
function viewportBounds(): { top: number; bottom: number } {
  const scroller = document.querySelector(SCROLLER);
  if (!scroller) {
    return { top: 0, bottom: window.innerHeight };
  }
  const rect = scroller.getBoundingClientRect();
  return { top: rect.top, bottom: rect.bottom };
}

/**
 * The eligible elements, nearest the viewport first.
 *
 * Measured in one pass before any job writes to the DOM: interleaving reads
 * with a job's writes would force a reflow for each one, which is the cost
 * this whole file exists to avoid.
 */
function byProximity(): Element[] {
  const { top, bottom } = viewportBounds();
  const ranked: Array<{ element: Element; distance: number }> = [];
  for (const element of eligible) {
    if (!pending.has(element) || running.has(element)) {
      eligible.delete(element);
      continue;
    }
    const rect = element.getBoundingClientRect();
    const distance = rect.bottom < top ? top - rect.bottom : Math.max(rect.top - bottom, 0);
    ranked.push({ element, distance });
  }
  ranked.sort((a, b) => a.distance - b.distance);
  return ranked.map((entry) => entry.element);
}

let draining = false;
let drainScheduled = false;

/** Ask for a drain on the next frame, if one is not already coming. */
function scheduleDrain(): void {
  if (drainScheduled || draining || eligible.size === 0) {
    return;
  }
  drainScheduled = true;
  const start = (): void => {
    drainScheduled = false;
    void drain();
  };
  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(start);
  } else {
    setTimeout(start, 0);
  }
}

/** Run the nearest eligible jobs until this frame's budget is spent. */
async function drain(): Promise<void> {
  if (draining) {
    return;
  }
  draining = true;
  try {
    const queue = byProximity();
    const deadline = now() + FRAME_BUDGET_MS;
    for (const element of queue) {
      // Re-checked per job rather than trusted from the ranking: the reader
      // keeps scrolling while the queue drains, and the block that was worth
      // drawing when the frame began may be behind them by now.
      if (!eligible.has(element)) {
        continue;
      }
      await run(element);
      if (now() >= deadline) {
        break;
      }
    }
  } finally {
    draining = false;
    scheduleDrain();
  }
}

async function execute(job: Job): Promise<void> {
  try {
    await job();
  } catch (error) {
    console.error("viewport-queue: deferred render failed", error);
  }
}

async function run(element: Element): Promise<void> {
  const job = pending.get(element);
  if (!job || running.has(element)) {
    return;
  }
  observer?.unobserve(element);
  eligible.delete(element);
  const first = running.size === 0;
  running.add(element);
  if (first) {
    activityListener?.(true);
  }
  try {
    if (!element.isConnected) {
      // The document was replaced while the job waited. Rendering into a
      // detached node costs what rendering into a visible one costs and
      // shows nobody anything.
      return;
    }
    await execute(job);
  } finally {
    running.delete(element);
    pending.delete(element);
    settleIfIdle();
  }
}

function settleIfIdle(): void {
  if (running.size > 0) {
    return;
  }
  activityListener?.(false);
  const waiters = idleWaiters;
  idleWaiters = [];
  for (const resolve of waiters) {
    resolve();
  }
}

/**
 * Resolve once the jobs that have started have finished.
 *
 * Not "once the queue is empty": the jobs for blocks the reader has not
 * reached are meant to stay waiting. This is the signal for anything that
 * has to see the screen as the reader will see it — restoring a scroll
 * position, for one, which measures a layout that KaTeX and Mermaid are
 * about to change.
 */
export function idle(): Promise<void> {
  if (running.size === 0) {
    return Promise.resolve();
  }
  return new Promise((resolve) => idleWaiters.push(resolve));
}

/**
 * Whether the queue still has work it considers near the viewport.
 *
 * A drain runs its jobs one at a time, so nothing is running in the gap
 * between two of them and [`idle`] resolves there. Anything that has to wait
 * for the screen rather than for a single block asks this as well.
 */
export function busy(): boolean {
  return running.size > 0 || eligible.size > 0 || draining || drainScheduled;
}

/**
 * Run `job` once `element` is near the viewport.
 *
 * Runs it immediately where `IntersectionObserver` is unavailable, so the
 * document still renders in full. Registering the same element twice keeps
 * the first job.
 */
export function whenNearViewport(element: Element, job: Job): void {
  const io = observerOrNull();
  if (!io) {
    // Same swallowing as the observed path: one block that throws must not
    // stop the caller from registering the blocks after it.
    void execute(job);
    return;
  }
  if (pending.has(element) || running.has(element)) {
    return;
  }
  pending.set(element, job);
  io.observe(element);
}

/** Whether `element` is waiting to be rendered. */
export function isPending(element: Element): boolean {
  return pending.has(element);
}

/** Number of jobs still waiting; for tests and for debugging. */
export function pendingCount(): number {
  return pending.size;
}

/**
 * Run every waiting job.
 *
 * Printing captures the whole document with no reader to scroll it, so the
 * queue has to be empty before the dialog opens.
 */
export async function flush(): Promise<void> {
  // A job can queue more work (Mermaid replaces a `<pre>` with an SVG that
  // wants a copy button), so drain rather than iterate once. Each round drains
  // everything pending, so a handful of rounds covers any document; the bound
  // is there because the caller is the print path, which would otherwise hang
  // forever on a queue that keeps refilling itself.
  //
  // `idle()` is part of a round because `run` returns at once for a block the
  // viewport already started — the block stays in `pending` until its job
  // returns, so without waiting the rounds would spin past an in-flight
  // diagram and report the queue as unflushable.
  for (let round = 0; round < MAX_FLUSH_ROUNDS && (pending.size > 0 || running.size > 0); round++) {
    const batch = Array.from(pending.keys());
    await Promise.all(batch.map((element) => run(element)));
    await idle();
  }
  if (pending.size > 0) {
    console.warn(`viewport-queue: ${pending.size} jobs still queued after flush`);
  }
}

/** How long a backfill pass runs before handing the thread back. */
const BACKFILL_BUDGET_MS = 4;

let backfilling = false;

/**
 * Draw the rest of the document in idle time, a little at a time.
 *
 * For a page that *is* a document rather than a window onto one — what
 * `arto page` writes, which a reader may print with the browser's own
 * command — deferring is right for the first paint and wrong forever after
 * it. `beforeprint` cannot delay the capture, so whatever is still queued
 * when the reader prints comes out as blank diagram boxes and raw TeX.
 * Filling in during the gaps keeps the fast first paint and still arrives at
 * a complete document.
 *
 * The app does not do this: there the reader is the one deciding what to
 * look at, its own print path drains the queue first, and drawing a
 * megabyte nobody asked for costs a laptop battery.
 */
export function backfillWhenIdle(): void {
  if (backfilling) {
    return;
  }
  backfilling = true;

  const later =
    typeof requestIdleCallback === "function"
      ? (run: () => void): void => void requestIdleCallback(() => run())
      : (run: () => void): void => void setTimeout(run, 50);

  const step = async (): Promise<void> => {
    const deadline = now() + BACKFILL_BUDGET_MS;
    // Drawing near the viewport is the reader's, and it takes the thread
    // first: this only ever picks up what the drain has left behind.
    while (pending.size > 0 && !draining && now() < deadline) {
      const next = pending.keys().next();
      if (next.done) {
        break;
      }
      await run(next.value);
    }
    if (pending.size > 0) {
      later(() => void step());
    } else {
      backfilling = false;
    }
  };
  later(() => void step());
}

/** Forget a waiting job, for an element that is being replaced or removed. */
export function cancel(element: Element): void {
  eligible.delete(element);
  if (pending.delete(element)) {
    observer?.unobserve(element);
  }
}

/**
 * Forget the jobs whose element has left the document.
 *
 * A job holds its element until it runs, and the job for a block the reader
 * never reached never runs. Without this, every block of every document
 * opened in the window stays alive for as long as the window does.
 */
export function prune(): void {
  for (const element of Array.from(pending.keys())) {
    if (!element.isConnected) {
      cancel(element);
    }
  }
}

/** Drop every job and the observer. For teardown between documents. */
export function reset(): void {
  backfilling = false;
  pending.clear();
  eligible.clear();
  running.clear();
  settleIfIdle();
  observer?.disconnect();
  observer = null;
  observerRoot = null;
}
