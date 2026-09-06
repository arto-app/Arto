import { describe, test, expect } from "vitest";

import { buildMermaidThemeConfig } from "./mermaid-theme";

describe("buildMermaidThemeConfig", () => {
  test("returns dark Mermaid theme for dark Arto theme", () => {
    const config = buildMermaidThemeConfig("dark");
    expect(config.theme).toBe("dark");
  });

  test("returns default Mermaid theme for light Arto theme", () => {
    const config = buildMermaidThemeConfig("light");
    expect(config.theme).toBe("default");
  });

  test("dark theme uses dark content background", () => {
    const config = buildMermaidThemeConfig("dark");
    expect(config.themeVariables.background).toBe("#0d1117");
  });

  test("light theme uses white content background", () => {
    const config = buildMermaidThemeConfig("light");
    expect(config.themeVariables.background).toBe("#ffffff");
  });

  test("both themes include shared font size override", () => {
    const dark = buildMermaidThemeConfig("dark");
    const light = buildMermaidThemeConfig("light");

    // Mermaid defaults to 16px; Arto overrides to 14px to match --font-size-base
    expect(dark.themeVariables.fontSize).toBe("14px");
    expect(light.themeVariables.fontSize).toBe("14px");
  });

  test("dark theme text color uses light text for readability", () => {
    const config = buildMermaidThemeConfig("dark");
    expect(config.themeVariables.textColor).toBe("#e6edf3");
    expect(config.themeVariables.primaryTextColor).toBe("#e6edf3");
  });

  test("light theme text color uses dark text for readability", () => {
    const config = buildMermaidThemeConfig("light");
    expect(config.themeVariables.textColor).toBe("#1f2328");
    expect(config.themeVariables.primaryTextColor).toBe("#1f2328");
  });

  test("both themes define git graph colors for all 4 branches", () => {
    for (const theme of ["light", "dark"] as const) {
      const config = buildMermaidThemeConfig(theme);
      for (const key of ["git0", "git1", "git2", "git3"]) {
        expect(config.themeVariables[key], `${theme} theme missing ${key}`).toBeDefined();
      }
    }
  });
});
