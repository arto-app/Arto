import "../style/main.css";

import { type Theme, getSystemTheme } from "./theme";
import * as markdownViewer from "./markdown-viewer";
import * as syntaxHighlighter from "./syntax-highlighter";
import * as mermaidRenderer from "./mermaid-renderer";
import { renderCoordinator } from "./render-coordinator";
import { setup as setupContextMenu, restoreSelection } from "./context-menu-handler";
import * as findInPage from "./find-in-page";
import * as keyboardInterceptor from "./keyboard-interceptor";
import * as scrollController from "./scroll-controller";

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
        find: typeof findInPage.find;
        navigate: typeof findInPage.navigate;
        navigateTo: typeof findInPage.navigateTo;
        clear: typeof findInPage.clear;
        reapply: typeof findInPage.reapply;
        setPinned: typeof findInPage.setPinned;
        scrollToPinnedMatch: typeof findInPage.scrollToPinnedMatch;
      };
      keyboard: {
        onKeydown: typeof keyboardInterceptor.onKeydown;
        pause: typeof keyboardInterceptor.pause;
        resume: typeof keyboardInterceptor.resume;
        isActive: typeof keyboardInterceptor.isActive;
      };
      scroll: {
        scrollBy: typeof scrollController.scrollBy;
        scrollPage: typeof scrollController.scrollPage;
        scrollHalfPage: typeof scrollController.scrollHalfPage;
        scrollToEdge: typeof scrollController.scrollToEdge;
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

  // Initialize keyboard interceptor (listens for keydown in bubble phase)
  keyboardInterceptor.setup();

  // Expose Arto API on window for Rust interop
  window.Arto = {
    setupContextMenu,
    restoreSelection,
    onRenderComplete: (callback) => renderCoordinator.onRenderComplete(callback),
    search: {
      setup: findInPage.setup,
      find: findInPage.find,
      navigate: findInPage.navigate,
      navigateTo: findInPage.navigateTo,
      clear: findInPage.clear,
      reapply: findInPage.reapply,
      setPinned: findInPage.setPinned,
      scrollToPinnedMatch: findInPage.scrollToPinnedMatch,
    },
    keyboard: {
      onKeydown: keyboardInterceptor.onKeydown,
      pause: keyboardInterceptor.pause,
      resume: keyboardInterceptor.resume,
      isActive: keyboardInterceptor.isActive,
    },
    scroll: {
      scrollBy: scrollController.scrollBy,
      scrollPage: scrollController.scrollPage,
      scrollHalfPage: scrollController.scrollHalfPage,
      scrollToEdge: scrollController.scrollToEdge,
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
