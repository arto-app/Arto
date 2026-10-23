import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts", "build/**/*.test.ts"],
    globalSetup: ["build/primer-themes-setup.ts"],
  },
});
