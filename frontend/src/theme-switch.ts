import { applyPictureTheme } from "./picture-theme";
import { type Theme, currentTheme, isDarkTheme, themedElement } from "./theme";
import * as viewportQueue from "./viewport-queue";

/** Switch the theme. */
export function setCurrentTheme(theme: Theme): void {
  themedElement().setAttribute("data-theme", theme);
  // The stylesheet repaints on the attribute alone, diagrams included: they
  // name their colours as CSS variables. Theme-aware pictures do not, because
  // their media query answers for the system rather than for the theme the
  // reader chose here.
  applyPictureTheme();
}

/**
 * Theme that was active before `preparePrint()` switched to light,
 * or null when no print is in progress (or the theme was already light).
 */
let printSavedTheme: Theme | null = null;

/**
 * Force the light theme for printing, and draw everything first.
 *
 * Printed output must always be light-on-white regardless of the UI theme.
 * A print job also has no reader and no scrolling, so everything the reader
 * never scrolled to has to be rendered first. This is the only place that
 * can do it: the caller awaits this promise before opening the dialog,
 * whereas a `beforeprint` listener cannot delay the capture.
 */
export async function preparePrint(): Promise<void> {
  if (isDarkTheme(currentTheme())) {
    printSavedTheme = currentTheme();
    setCurrentTheme("light");
  }
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
