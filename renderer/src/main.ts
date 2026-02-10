import "../style/main.css";

import { type Theme, getSystemTheme } from "./theme";
import * as markdownViewer from "./markdown-viewer";
import * as syntaxHighlighter from "./syntax-highlighter";
import * as mermaidRenderer from "./mermaid-renderer";
import { renderCoordinator } from "./render-coordinator";
import { setup as setupContextMenu, restoreSelection } from "./context-menu-handler";
import * as findInPage from "./find-in-page";

// Declare global Arto namespace
declare global {
  interface Window {
    Arto: {
      setupContextMenu: typeof setupContextMenu;
      restoreSelection: typeof restoreSelection;
      /** Register a callback to be called when rendering (Mermaid, KaTeX, etc.) completes */
      onRenderComplete: (callback: () => void) => void;
      search: {
        setup: typeof findInPage.setup;
        applyHighlights: typeof findInPage.applyHighlights;
        navigate: typeof findInPage.navigate;
        navigateTo: typeof findInPage.navigateTo;
        clear: typeof findInPage.clear;
        reapply: typeof findInPage.reapply;
        setPinned: typeof findInPage.setPinned;
        scrollToPinnedMatch: typeof findInPage.scrollToPinnedMatch;
        setNavigateOnApply: typeof findInPage.setNavigateOnApply;
      };
    };
  }
}

function getCurrentTheme(): Theme {
  const theme = document.body.getAttribute("data-theme");
  switch (theme) {
    case "light":
    case "dark":
      return theme;
    default:
      return getSystemTheme();
  }
}

export function setCurrentTheme(theme: Theme) {
  document.body.setAttribute("data-theme", theme);
  markdownViewer.setTheme(theme);
  syntaxHighlighter.setTheme(theme);
  mermaidRenderer.setTheme(theme);
  renderCoordinator.forceRenderMermaid();
}

export function init(): void {
  markdownViewer.mount();
  syntaxHighlighter.mount();
  mermaidRenderer.init();
  renderCoordinator.init();

  // Expose Arto API on window for Rust interop
  window.Arto = {
    setupContextMenu,
    restoreSelection,
    onRenderComplete: (callback) => renderCoordinator.onRenderComplete(callback),
    search: {
      setup: findInPage.setup,
      applyHighlights: findInPage.applyHighlights,
      navigate: findInPage.navigate,
      navigateTo: findInPage.navigateTo,
      clear: findInPage.clear,
      reapply: findInPage.reapply,
      setPinned: findInPage.setPinned,
      scrollToPinnedMatch: findInPage.scrollToPinnedMatch,
      setNavigateOnApply: findInPage.setNavigateOnApply,
    },
  };

  // Listen for theme changes from Rust
  document.addEventListener("arto:theme-changed", ((event: CustomEvent) => {
    setCurrentTheme(event.detail);
  }) as EventListener);

  // Set initial theme
  setCurrentTheme(getCurrentTheme());
}

// Re-export mermaid window functions
export { initMermaidWindow } from "./mermaid-window-controller";
