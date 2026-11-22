import { type Theme, getSystemTheme, themedElement } from "@arto-app/markdown/theme";

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
