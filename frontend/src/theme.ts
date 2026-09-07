const mediaQuery = "(prefers-color-scheme: dark)";

/**
 * A theme name as it appears in `data-theme`, e.g. `light`, `dark_dimmed`.
 *
 * The names are GitHub's and the set of them is decided on the Rust side; the
 * stylesheet carries a token block for each. Nothing here enumerates them, so
 * a theme is added without touching the frontend.
 */
export type Theme = string;

/** Whether a theme paints a dark canvas, by the same rule the stylesheet uses. */
export function isDarkTheme(theme: Theme): boolean {
  return theme.startsWith("dark");
}

export function getSystemTheme(): Theme {
  return window.matchMedia(mediaQuery).matches ? "dark" : "light";
}

/**
 * The element the theme is named on.
 *
 * The root rather than the body, so that `<html>` resolves the theme's tokens
 * too: the page's own scrollbars and the canvas behind it take their colours
 * from the root element, and everything below inherits either way.
 */
export function themedElement(): HTMLElement {
  return document.documentElement;
}

/** The theme currently painted, or the system's when none is named. */
export function currentTheme(): Theme {
  return themedElement().getAttribute("data-theme") || getSystemTheme();
}

/**
 * Listen for `arto:theme-changed` custom events and sync `data-theme`.
 * Intended for child viewer windows (Math, Image) where only the attribute
 * needs to be updated. Main window uses its own listener with additional logic.
 */
export function setupThemeSync(): void {
  document.addEventListener("arto:theme-changed", ((event: CustomEvent) => {
    const detail: unknown = event.detail;
    const theme: Theme = typeof detail === "string" && detail !== "" ? detail : getSystemTheme();
    themedElement().setAttribute("data-theme", theme);
  }) as EventListener);
}
