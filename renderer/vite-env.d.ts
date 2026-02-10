/// <reference types="vite-plugin-svgr/client" />

// CSS Custom Highlight API (Safari 17.2+, Chrome 105+)
// Not yet in TypeScript's DOM lib for ES2020 target
interface HighlightRegistry {
  set(name: string, highlight: Highlight): void;
  get(name: string): Highlight | undefined;
  delete(name: string): boolean;
  clear(): void;
  has(name: string): boolean;
}

declare class Highlight {
  constructor(...ranges: AbstractRange[]);
  priority: number;
  add(range: AbstractRange): void;
  delete(range: AbstractRange): boolean;
  clear(): void;
  readonly size: number;
}

declare namespace CSS {
  const highlights: HighlightRegistry;
}
