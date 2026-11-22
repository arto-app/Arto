import { writePrimerThemes } from "@arto-app/markdown/build/primer-themes";

/**
 * Vitest's global setup: write the renderer's generated Primer stylesheet
 * before any test file runs.
 *
 * Vitest reads its own config instead of `vite.config.ts`, so the plugin that
 * generates the stylesheet never runs here — and the generated file is not
 * committed. `theme.test.ts` reads it, so it has to exist whether or not a
 * build has happened first.
 */
export default function setup(): void {
  writePrimerThemes();
}
