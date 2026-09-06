// katex ships type declarations for its main entry only; the auto-render
// contrib module is untyped upstream. This mirrors the options documented at
// https://katex.org/docs/autorender for the subset the app relies on.
declare module "katex/contrib/auto-render" {
  import type { KatexOptions } from "katex";

  export interface RenderMathInElementDelimiter {
    left: string;
    right: string;
    display: boolean;
  }

  export interface RenderMathInElementOptions extends KatexOptions {
    delimiters?: RenderMathInElementDelimiter[];
    ignoredTags?: string[];
    ignoredClasses?: string[];
    errorCallback?: (message: string, error: Error) => void;
    preProcess?: (math: string) => string;
  }

  export default function renderMathInElement(
    element: HTMLElement,
    options?: RenderMathInElementOptions,
  ): void;
}
