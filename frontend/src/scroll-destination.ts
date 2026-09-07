/**
 * Going somewhere in a document whose height is still being decided.
 *
 * Diagrams and formulas are drawn only once the reader is near them, and a
 * block is a different height once its diagram is in it than it was as the
 * code block it started as. So a position computed now is computed partly
 * from heights that are about to change. Worse, arriving is what changes
 * them: getting there is what brings those blocks near, which is what makes
 * them draw.
 *
 * So a destination is not a number to jump to once. It is something to
 * re-apply while the document is still moving underneath — until it holds
 * still, or until the reader takes over.
 */

/** How long a destination keeps being re-applied. */
const SETTLE_MS = 2000;

/** How long to wait for a smooth scroll to land if `scrollend` never comes. */
const SCROLL_END_TIMEOUT = 1000;

/** How far, in viewport heights, a journey is still worth animating. */
const SMOOTH_REACH = 3;

/**
 * The element the document scrolls in.
 *
 * The app puts the document inside `.content`; the standalone page renderer
 * has no such element and scrolls the document itself. Everything that reads
 * or sets a scroll position goes through here so the two hosts cannot drift
 * apart.
 */
export function scrollContainer(): HTMLElement | null {
  return (
    document.querySelector<HTMLElement>(".content") ??
    (document.scrollingElement as HTMLElement | null)
  );
}

/** The rendered document, whose size changing is what moves a destination. */
function documentBody(): Element | null {
  return document.querySelector(".markdown-body");
}

/**
 * The y of the scroller's top edge, in viewport coordinates.
 *
 * When the document itself scrolls — the standalone page renderer — the
 * scrolling element's own rect top is `-scrollTop` rather than the top of the
 * view, so measuring against it would double the scroll offset.
 */
export function scrollerTop(scroller: HTMLElement): number {
  if (scroller === document.scrollingElement || scroller === document.documentElement) {
    return 0;
  }
  return scroller.getBoundingClientRect().top + scroller.clientTop;
}

/** Ends the destination currently being held, if any. */
let releaseCurrent: (() => void) | null = null;

/**
 * Give up the destination being held, if there is one.
 *
 * A destination outlives the jump that set it, so anything that moves the view
 * somewhere else has to say so — otherwise the next lazily drawn diagram takes
 * the reader back to a place they have already left. `wheel` and `touchstart`
 * say it for the trackpad and the touchscreen; a scroll the app performs on the
 * reader's behalf — a keybinding, the content cursor, a search match — reaches
 * neither of those, and says it by calling this.
 *
 * Dragging the scrollbar is the gap: it is a UA widget and dispatches no
 * pointer event on the scroller, so a drag within the settle window is
 * fought until the window closes. `pointerdown` would not cover it either,
 * and giving up on any scroll the destination did not itself make cannot
 * tell the reader apart from the browser's own anchoring adjustments.
 */
export function releaseDestination(): void {
  releaseCurrent?.();
}

/**
 * Go where `apply` says, and go there again while the document keeps
 * changing size under it.
 *
 * `apply` has to be instant and repeatable: it is called until the document
 * settles. Only one destination is held at a time — asking for a new one
 * gives up the old, which is what makes it safe for a caller to ask twice
 * (the two-phase scroll restore does).
 */
export function settleAt(apply: () => void): void {
  releaseDestination();

  apply();

  const scroller = scrollContainer();
  const body = documentBody();
  if (!scroller || !body || typeof ResizeObserver === "undefined") {
    return;
  }

  const observer = new ResizeObserver(() => apply());
  const release = (): void => {
    if (releaseCurrent !== release) {
      return;
    }
    releaseCurrent = null;
    observer.disconnect();
    clearTimeout(timer);
    scroller.removeEventListener("wheel", release);
    scroller.removeEventListener("touchstart", release);
  };
  releaseCurrent = release;

  const timer = setTimeout(release, SETTLE_MS);
  // The reader wins: a wheel or a touch means they no longer want to be here.
  scroller.addEventListener("wheel", release, { once: true, passive: true });
  scroller.addEventListener("touchstart", release, { once: true, passive: true });
  observer.observe(body);
}

/**
 * Whether travelling to `target` is close enough to be worth animating.
 *
 * A smooth scroll travels through everything in between, and with blocks laid
 * out lazily that means laying out and drawing every block it passes — work
 * the reader asked to skip, spent on screens they will never see, and spent
 * during the animation, where it costs frames. Beyond a few screens the jump
 * is not conveying continuity any more, only paying for it.
 */
export function withinSmoothReach(scroller: HTMLElement, target: Element): boolean {
  return withinSmoothDistance(scroller, target.getBoundingClientRect().top - scrollerTop(scroller));
}

/** [`withinSmoothReach`] for a journey named by its length rather than its target. */
export function withinSmoothDistance(scroller: HTMLElement, distance: number): boolean {
  return Math.abs(distance) <= scroller.clientHeight * SMOOTH_REACH;
}

/**
 * Run `landed` once a smooth scroll has finished, or after
 * `SCROLL_END_TIMEOUT` if the browser never says so.
 *
 * Travelling to a destination is worth animating; correcting it afterwards is
 * not, and re-issuing a smooth scroll would restart the journey rather than
 * adjust it. So the two are separated: animate once, then settle.
 *
 * The journey counts as the destination while it lasts, so whatever gives the
 * destination up gives the journey up with it. Without that, a reader who
 * scrolls away — or asks for somewhere else — during the animation is carried
 * to the abandoned target the moment `scrollend` arrives, because a scroll of
 * their own is what makes it arrive.
 */
export function afterScroll(scroller: HTMLElement, landed: () => void): void {
  releaseDestination();

  let done = false;
  let moved = false;
  const noteScroll = (): void => {
    moved = true;
  };
  const stop = (): void => {
    done = true;
    if (releaseCurrent === abandon) {
      releaseCurrent = null;
    }
    clearTimeout(timer);
    scroller.removeEventListener("scrollend", finish);
    scroller.removeEventListener("scroll", noteScroll);
    scroller.removeEventListener("wheel", abandon);
    scroller.removeEventListener("touchstart", abandon);
  };
  const abandon = (): void => {
    if (done) {
      return;
    }
    stop();
  };
  const finish = (): void => {
    if (done) {
      return;
    }
    stop();
    landed();
  };
  releaseCurrent = abandon;

  const timer = setTimeout(finish, SCROLL_END_TIMEOUT);
  scroller.addEventListener("scrollend", finish, { once: true });
  scroller.addEventListener("scroll", noteScroll, { passive: true });
  scroller.addEventListener("wheel", abandon, { once: true, passive: true });
  scroller.addEventListener("touchstart", abandon, { once: true, passive: true });

  // A journey with nothing to travel — the view is already where it was
  // asked to go, which is what the second phase of a restore looks like —
  // scrolls not at all, so neither `scroll` nor `scrollend` ever fires and
  // the correction would wait out the whole timeout. That wait is exactly
  // when the blocks around the destination draw and move it, and nothing is
  // holding the position: this call gave up whatever was. Two frames without
  // a single scroll event means the journey never started.
  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (!moved) {
          finish();
        }
      }),
    );
  }
}
