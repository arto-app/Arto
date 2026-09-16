/**
 * The app's bundle: the runtime with every library built into it.
 *
 * The app loads this once and keeps it for as long as it runs, so there is
 * nothing to gain by leaving a library out — and the separate windows, which
 * only the app opens, are exported from here rather than from the runtime so
 * that a page's bundle does not carry them (a Mermaid window would carry
 * Mermaid with it, whatever the document holds).
 */

import hljs from "highlight.js";
import html2canvas from "html2canvas";
import katex from "katex";
import mermaid from "mermaid";

import { provideHljs, provideHtml2Canvas, provideKatex, provideMermaid } from "./libraries";

provideMermaid(mermaid);
provideKatex(katex);
provideHtml2Canvas(html2canvas);
// Every language, because the app opens whatever it is given next.
provideHljs(hljs);

export * from "./runtime";

export { initMermaidWindow } from "./mermaid-window-controller";
export { initMathWindow, setMathTheme, copyMathAsImage } from "./math-window-controller";
export {
  initImageWindow,
  toggleImageFitMode,
  getImageDimensions,
  copyImageToClipboard,
} from "./image-window-controller";
