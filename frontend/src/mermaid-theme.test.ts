import { describe, test, expect } from "vitest";

import { buildMermaidThemeConfig } from "./mermaid-theme";

/** Answers every token with its own name, so a mapping is visible in the value. */
const echo = (name: string) => name;

describe("buildMermaidThemeConfig", () => {
  test("returns dark Mermaid theme for a dark GitHub theme", () => {
    expect(buildMermaidThemeConfig("dark", echo).theme).toBe("dark");
    expect(buildMermaidThemeConfig("dark_dimmed", echo).theme).toBe("dark");
  });

  test("returns default Mermaid theme for a light GitHub theme", () => {
    expect(buildMermaidThemeConfig("light", echo).theme).toBe("default");
    expect(buildMermaidThemeConfig("light_high_contrast", echo).theme).toBe("default");
  });

  test("paints the diagram on the theme's own canvas", () => {
    const config = buildMermaidThemeConfig("dark_dimmed", echo);
    expect(config.themeVariables.background).toBe("--bgColor-default");
    expect(config.themeVariables.textColor).toBe("--fgColor-default");
  });

  test("writes node labels in the ink that goes with the node fill", () => {
    const config = buildMermaidThemeConfig("light", echo);
    expect(config.themeVariables.mainBkg).toBe("--bgColor-muted");
    expect(config.themeVariables.nodeTextColor).toBe("--fgColor-default");
  });

  test("keeps the catch-all label ink on the page, not on emphasis", () => {
    // Mermaid falls back to primaryTextColor for state, requirement, quadrant
    // and xy chart labels, none of which are drawn on primaryColor. The ink on
    // emphasis is white, so those labels would vanish in the light themes.
    const config = buildMermaidThemeConfig("light", echo);
    expect(config.themeVariables.primaryTextColor).toBe("--fgColor-default");
    expect(config.themeVariables.gitBranchLabel0).toBe("--fgColor-onEmphasis");
  });

  test("includes the shared font size override", () => {
    // Mermaid defaults to 16px; Arto overrides to 14px to match --font-size-base
    expect(buildMermaidThemeConfig("light", echo).themeVariables.fontSize).toBe("14px");
  });

  test("defines git graph colors for all branches", () => {
    const config = buildMermaidThemeConfig("light", echo);
    for (const key of ["git0", "git1", "git2", "git3"]) {
      expect(config.themeVariables[key], `missing ${key}`).toBeDefined();
    }
  });

  test("drops variables whose token resolves to nothing", () => {
    const config = buildMermaidThemeConfig("light", (name) =>
      name === "--bgColor-default" ? "" : "#123456",
    );
    expect(config.themeVariables.background).toBeUndefined();
    expect(config.themeVariables.textColor).toBe("#123456");
  });
});
