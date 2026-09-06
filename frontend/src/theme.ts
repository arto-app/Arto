const mediaQuery = "(prefers-color-scheme: dark)";

export type Theme = "light" | "dark";

export type ThemeStyle = {
  element: HTMLStyleElement;
  mount: () => void;
  enable: () => void;
  disable: () => void;
};

export function createThemeStyle(css: string, { enabled }: { enabled?: boolean } = {}): ThemeStyle {
  const element = document.createElement("style");
  element.disabled = !enabled;
  element.textContent = css;
  return {
    element,
    mount: () => {
      if (!element.isConnected) {
        document.head.append(element);
      }
    },
    enable: () => {
      element.disabled = false;
    },
    disable: () => {
      element.disabled = true;
    },
  };
}

export function getSystemTheme(): Theme {
  return window.matchMedia(mediaQuery).matches ? "dark" : "light";
}

/**
 * Listen for `arto:theme-changed` custom events and sync `data-theme` on body.
 * Intended for child viewer windows (Math, Image) where only the body attribute
 * needs to be updated. Main window uses its own listener with additional logic.
 */
export function setupBodyThemeSync(): void {
  document.addEventListener("arto:theme-changed", ((event: CustomEvent) => {
    const detail: unknown = event.detail;
    // Only accept supported theme values; resolve unknown to system theme
    const theme: Theme = detail === "light" || detail === "dark" ? detail : getSystemTheme();
    document.body.setAttribute("data-theme", theme);
  }) as EventListener);
}
