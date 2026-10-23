import { applyPictureTheme } from "./picture-theme";
import { type Theme, currentTheme, isDarkTheme, themedElement } from "./theme";
import * as mermaidRenderer from "./mermaid-renderer";
import { renderCoordinator } from "./render-coordinator";
import * as linkPreview from "./link-preview";
import * as viewportQueue from "./viewport-queue";

/**
 * Switch the theme.
 *
 * Returns once the diagrams have been queued again in the new theme — the
 * print path waits on that before draining the queue, everything else can
 * ignore it.
 */
export function setCurrentTheme(theme: Theme): Promise<void> {
  themedElement().setAttribute("data-theme", theme);
  // The stylesheet repaints on the attribute alone. Mermaid has to be told,
  // because its colours are baked into the SVG it already drew, and so do
  // theme-aware pictures, whose media query answers for the system rather
  // than for the theme the reader chose here.
  mermaidRenderer.setTheme(theme);
  applyPictureTheme();
  // A preview's diagrams were drawn in the old colours too, and are its own
  // to draw; it goes, and comes back in the new theme on the next rest.
  linkPreview.hide();
  return renderCoordinator.forceRenderMermaid();
}

/**
 * Theme that was active before `preparePrint()` switched to light,
 * or null when no print is in progress (or the theme was already light).
 */
let printSavedTheme: Theme | null = null;

/**
 * Force the light theme for printing.
 *
 * Printed output must always be light-on-white regardless of the UI theme.
 * The print CSS can override plain colors, but Mermaid bakes its theme into
 * the generated SVG, so the diagrams have to be re-rendered with the light
 * theme before the print dialog captures the page. Resolves once that
 * re-render completes (with a timeout fallback so printing never hangs).
 *
 * A print job also has no reader and no scrolling, so everything the reader
 * never scrolled to has to be rendered first. This is the only place that
 * can do it: the caller awaits this promise before opening the dialog,
 * whereas a `beforeprint` listener cannot delay the capture.
 */
export async function preparePrint(): Promise<void> {
  if (!isDarkTheme(currentTheme())) {
    await viewportQueue.flush();
    return;
  }
  printSavedTheme = currentTheme();

  // Flushing before the switch would draw every diagram in the dark theme
  // only for the switch to throw it away, so the queue is drained once, after
  // the theme is already light. A diagram still waiting in the queue renders
  // from the Mermaid config current when its job runs, which is the light one
  // by then.
  //
  // Waiting on the switch itself rather than on a render-complete callback:
  // the switch clears the drawn diagrams and queues them again, and draining
  // before that has happened would drain them as nothing.
  await setCurrentTheme("light");
  await viewportQueue.flush();
}

/** Restore the theme that was active before `preparePrint()`. */
export function restorePrint(): void {
  if (printSavedTheme === null) {
    return;
  }
  setCurrentTheme(printSavedTheme);
  printSavedTheme = null;
}
