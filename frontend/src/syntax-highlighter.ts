import { type HljsLibrary, hljsLibrary } from "./libraries";
import { whenNearViewport } from "./viewport-queue";

/**
 * The library with the two languages another renderer owns taken out of it.
 *
 * Done on the way to the first block rather than as this module loads,
 * because the library arrives from outside: the app hands it over as it
 * starts, and a page hands over whichever build its document called for.
 */
let prepared: HljsLibrary | null = null;

function highlighter(): HljsLibrary | null {
  const hljs = hljsLibrary();
  if (!hljs || hljs === prepared) {
    return hljs;
  }
  // Remove some languages that other libraries handle better
  if (hljs.getLanguage("mermaid")) hljs.unregisterLanguage("mermaid");
  if (hljs.getLanguage("math")) hljs.unregisterLanguage("math");
  prepared = hljs;
  return hljs;
}

export function highlightCodeBlocks(container: Element): void {
  // A page whose document has no code block carries no highlighter, and has
  // no block here for one to work on either.
  if (!highlighter()) {
    return;
  }

  const codeBlocks = container.querySelectorAll("pre code:not([data-highlighted])");

  if (codeBlocks.length === 0) {
    return;
  }

  console.debug(`Queueing ${codeBlocks.length} code blocks`);

  codeBlocks.forEach((block) => {
    const element = block as HTMLElement;
    // Highlighting walks the whole token stream of a block, so it waits for
    // the reader like the diagrams and formulas do. The code is already
    // readable as plain text until then.
    whenNearViewport(element, () => highlightCodeBlock(element));
  });
}

function highlightCodeBlock(element: HTMLElement): void {
  const hljs = highlighter();
  if (!hljs) {
    return;
  }

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
