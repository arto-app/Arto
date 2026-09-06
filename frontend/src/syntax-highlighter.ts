import hljs from "highlight.js";
import hljsLightTheme from "highlight.js/styles/github.css?inline";
import hljsDarkTheme from "highlight.js/styles/github-dark.css?inline";
import { createThemeStyle, type Theme } from "./theme";

// Remove some languages that other libraries handle better
if (hljs.getLanguage("mermaid")) hljs.unregisterLanguage("mermaid");
if (hljs.getLanguage("math")) hljs.unregisterLanguage("math");

const lightThemeStyle = createThemeStyle(hljsLightTheme, { enabled: true });
const darkThemeStyle = createThemeStyle(hljsDarkTheme);

export function mount(): void {
  lightThemeStyle.mount();
  darkThemeStyle.mount();
}

export function setTheme(theme: Theme): void {
  switch (theme) {
    case "light":
      lightThemeStyle.enable();
      darkThemeStyle.disable();
      console.debug("Light theme of highlight.js applied");
      break;
    case "dark":
      lightThemeStyle.disable();
      darkThemeStyle.enable();
      console.debug("Dark theme of highlight.js applied");
      break;
  }
}

export function highlightCodeBlocks(container: Element): void {
  const codeBlocks = container.querySelectorAll("pre code:not([data-highlighted])");

  if (codeBlocks.length === 0) {
    return;
  }

  console.debug(`Highlighting ${codeBlocks.length} code blocks`);

  codeBlocks.forEach((block) => {
    highlightCodeBlock(block as HTMLElement);
  });
}

function highlightCodeBlock(element: HTMLElement): void {
  // Skip if already highlighted
  if (element.dataset.highlighted === "yes") {
    return;
  }

  // Extract language from class name (e.g., "language-rust" -> "rust")
  const langMatch = element.className.match(/language-([\w-]+)/);
  if (langMatch) {
    const lang = langMatch[1];

    if (lang === "mermaid" || lang === "math") {
      element.dataset.highlighted = "yes";
      return;
    }

    // Only highlight if the language is registered
    if (hljs.getLanguage(lang)) {
      try {
        // Highlight the code block
        hljs.highlightElement(element);
        console.debug(`Highlighted code block with language: ${lang}`);
      } catch (error) {
        console.warn(`Failed to highlight code block (${lang}):`, error);
        element.dataset.highlighted = "yes";
      }
    } else {
      console.debug(`Language not registered: ${lang}`);
      element.dataset.highlighted = "yes";
    }
    return;
  }

  try {
    hljs.highlightElement(element);
    console.debug("Highlighted code block with auto-detection");
  } catch (error) {
    console.warn("Failed to highlight code block (auto):", error);
  } finally {
    element.dataset.highlighted = "yes";
  }
}
