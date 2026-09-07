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
 * The observer starts a job several screens ahead of the viewport, so normal
 * scrolling arrives at content that is already rendered. Printing needs the
 * whole document at once and calls [`flush`] first.
 */

/** How far outside the viewport a job starts. Three screens above and below. */
const ROOT_MARGIN = "300% 0px";

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
 * The elements whose job has started and not yet finished.
 *
 * A job that awaits — every Mermaid one does — lets the batch render that its
 * own DOM writes scheduled see the block again before it is marked rendered.
 * Holding the element here until the job returns is what stops that batch
 * from starting a second job on it.
 */
const running = new Set<Element>();
let observer: IntersectionObserver | null = null;

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

function observerOrNull(): IntersectionObserver | null {
  if (typeof IntersectionObserver === "undefined") {
    return null;
  }
  observer ??= new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          void run(entry.target);
        }
      }
    },
    { root: document.querySelector(SCROLLER), rootMargin: ROOT_MARGIN },
  );
  return observer;
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
  for (let round = 0; round < MAX_FLUSH_ROUNDS && pending.size > 0; round++) {
    const batch = Array.from(pending.keys());
    await Promise.all(batch.map((element) => run(element)));
  }
  if (pending.size > 0) {
    console.warn(`viewport-queue: ${pending.size} jobs still queued after flush`);
  }
}

/** Forget a waiting job, for an element that is being replaced or removed. */
export function cancel(element: Element): void {
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
  pending.clear();
  running.clear();
  settleIfIdle();
  observer?.disconnect();
  observer = null;
}
