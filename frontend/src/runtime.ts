import "../style/main.css";

import { createArtoApi } from "./api";
import { refreshReadingPosition, setupReadingPosition } from "./reading-position";
import { setupRowHover } from "./row-hover";
import { setupScrollbarReach } from "./scrollbar-reach";
import { currentTheme } from "./theme";
import { setCurrentTheme } from "./theme-switch";
import { renderCoordinator } from "./render-coordinator";
import * as keyboardInterceptor from "./keyboard-interceptor";
import * as mouseNavigation from "./mouse-navigation";
import * as scrollController from "./scroll-controller";
import * as contentCursor from "./content-cursor";
import * as lenses from "./lenses";
import * as changes from "./changes";
import * as linkPreview from "./link-preview";
import * as fragmentLink from "./fragment-link";
import * as viewportQueue from "./viewport-queue";
import * as stickyTableHead from "./sticky-table-head";
import * as videoFallback from "./video-fallback";
import { forgetStep, setupFocusScrolling, stepFocus } from "./focus-mode";

export { setCurrentTheme } from "./theme-switch";

export function init(): void {
  renderCoordinator.init();

  // The header's line and the gutter's current tick both answer to where the
  // reader is, and a new document moves them without a scroll happening. The
  // callback fires once, so it re-arms itself for the render after this one.
  setupReadingPosition();
  // The full name of a row the panel had to cut, floating clear of the box
  // that scrolls it.
  setupRowHover();
  // The answer a lens keeps beside a block, on hover over its mark.
  lenses.setup();
  // What a footnote, a heading link or a document link points at, on a
  // rest over the link.
  linkPreview.setup();
  // A link to elsewhere on the page — a footnote and the way back from it.
  fragmentLink.setup();
  // The scrollbar, brought up to a native width by the pointer arriving at
  // the edge it is on.
  setupScrollbarReach();
  // A long table's header row, held at the top of the view while it is read.
  stickyTableHead.setup();
  // A GitHub video that cannot play, swapped for the link it holds.
  videoFallback.setup();
  // What a mark of a change since last read means, over the mark.
  changes.setup();
  // A key can move the content cursor without scrolling, and focus mode marks
  // the block the cursor is on.
  contentCursor.onCursorChange(refreshReadingPosition);
  // Focus mode brings the block being read to the middle on the way in, the
  // same way a jump to a heading is, so neither undoes the other.
  setupFocusScrolling((block) => scrollController.toElement(block, "center"));
  // And a line-scroll key steps from block to block.
  scrollController.setLineStep(stepFocus, forgetStep);
  const trackAfterRender = (): void => {
    refreshReadingPosition();
    // A document replaced under a preview takes the preview's link with it.
    linkPreview.hideIfDetached();
    renderCoordinator.onRenderComplete(trackAfterRender);
  };
  renderCoordinator.onRenderComplete(trackAfterRender);

  // A page with no `.content` is one `arto page` wrote: a whole document,
  // which its reader can print with the browser's own command. Nothing can
  // delay that capture, so the rest of it is drawn in idle time instead of
  // waiting for a scroll that may never come. See `backfillWhenIdle`.
  if (!document.querySelector(".content")) {
    viewportQueue.backfillWhenIdle();
  }

  // Expose Arto API on window for Rust interop
  window.Arto = createArtoApi();

  // Set up keyboard interceptor event listeners
  keyboardInterceptor.setup();

  // The mouse's thumb buttons, which walk the history like the arrows do.
  mouseNavigation.setup();

  // Listen for theme changes from Rust
  document.addEventListener("arto:theme-changed", ((event: CustomEvent) => {
    setCurrentTheme(event.detail);
  }) as EventListener);

  // Set initial theme
  setCurrentTheme(currentTheme());
}

// The libraries this runtime draws with are handed to it rather than built
// in, so that a page can leave out the ones its document never calls for.
// Both entries re-export these: the app's calls them itself, the page's
// exposes them for the bundles appended after it.
export { provideMermaid, provideKatex, provideHtml2Canvas, provideHljs } from "./libraries";
