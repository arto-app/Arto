import { defineConfig } from "vitest/config";

import { writePrimerThemes } from "./vite.config.ts";

// Vitest reads this file instead of `vite.config.ts`, so the plugin that
// generates `style/generated/primer-themes.css` never runs here — and the
// generated file is not committed. `theme.test.ts` reads it, so it has to
// exist whether or not a build has happened first.
writePrimerThemes();

export default defineConfig({
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts"],
  },
});
