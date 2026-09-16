/**
 * The heavy libraries a document may need, handed over at runtime.
 *
 * The app carries all three in its own bundle and hands them over as it
 * starts. A standalone page cannot: it embeds the frontend inline, so every
 * byte is paid again by every page written and every Quick Look preview, and
 * mermaid alone is three quarters of the bundle. The page bundle therefore
 * leaves them out, and `arto-page` appends the ones the document actually
 * calls for — each a bundle of its own that registers what it carries here.
 *
 * So a renderer asks for its library rather than importing it, and gets
 * nothing on a page whose document never needed it. That case is not an
 * error to report: the document has no diagram to draw, no formula to set
 * and no block to rasterize, so nothing asks in the first place.
 */

declare global {
  interface Window {
    /**
     * The page runtime, under the name its IIFE bundle publishes it as.
     *
     * A library bundle is appended after that one and shares nothing with it
     * but this object: each is a bundle of its own, with its own module
     * scope, so the global is the only way back to the slots above.
     */
    ArtoRenderer?: {
      provideMermaid: typeof provideMermaid;
      provideKatex: typeof provideKatex;
      provideHtml2Canvas: typeof provideHtml2Canvas;
      provideHljs: typeof provideHljs;
    };
  }
}

export type MermaidLibrary = typeof import("mermaid").default;
export type KatexLibrary = typeof import("katex").default;
export type Html2CanvasLibrary = typeof import("html2canvas").default;
export type HljsLibrary = typeof import("highlight.js").default;

let mermaid: MermaidLibrary | null = null;
let katex: KatexLibrary | null = null;
let html2canvas: Html2CanvasLibrary | null = null;
let hljs: HljsLibrary | null = null;

export function provideMermaid(library: MermaidLibrary): void {
  mermaid = library;
}

export function mermaidLibrary(): MermaidLibrary | null {
  return mermaid;
}

export function provideKatex(library: KatexLibrary): void {
  katex = library;
}

export function katexLibrary(): KatexLibrary | null {
  return katex;
}

export function provideHtml2Canvas(library: Html2CanvasLibrary): void {
  html2canvas = library;
}

export function html2canvasLibrary(): Html2CanvasLibrary | null {
  return html2canvas;
}

/**
 * The languages this one arrives with are whatever the bundle that provided
 * it was built from: all of them for the app, and for a page the set its
 * document turned out to need.
 */
export function provideHljs(library: HljsLibrary): void {
  hljs = library;
}

export function hljsLibrary(): HljsLibrary | null {
  return hljs;
}
