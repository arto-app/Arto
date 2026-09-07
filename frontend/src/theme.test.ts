import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, test, expect } from "vitest";

import { isDarkTheme } from "./theme";

const repository = path.resolve(import.meta.dirname, "../..");

/** The theme names the app can put in `data-theme`, read from the enum. */
function themesTheAppCanAskFor(): string[] {
  const source = readFileSync(
    path.join(repository, "crates/arto-config/src/color_theme.rs"),
    "utf-8",
  );
  const names = source.match(/Self::\w+ => "(\w+)"/g)?.map((line) => line.split('"')[1]);
  expect(names, "no theme names found in color_theme.rs").toBeTruthy();
  return names as string[];
}

function generatedStylesheet(): string {
  return readFileSync(path.join(repository, "frontend/style/generated/primer-themes.css"), {
    encoding: "utf-8",
  });
}

/** The theme names the generated stylesheet has tokens for. */
function themesTheStylesheetPaints(): string[] {
  return [...generatedStylesheet().matchAll(/\[data-theme="(\w+)"\]/g)].map((match) => match[1]);
}

describe("theme names", () => {
  // The two sides are derived independently — the enum by hand, the
  // stylesheet from whatever @primer/primitives ships — and nothing fails
  // when they drift. What the user would see is an app painted in no colours
  // at all, so the drift is caught here instead.
  test("every theme the app can ask for has tokens in the stylesheet", () => {
    const painted = themesTheStylesheetPaints();
    for (const theme of themesTheAppCanAskFor()) {
      expect(painted, `no token block for "${theme}"`).toContain(theme);
    }
  });

  // The theme is named on the root element, so a fallback written as a bare
  // `:root` matches that same element at the same specificity as the theme's
  // own block — and the blocks sorting after it silently win. That is a dark
  // theme that stays light, with nothing failing anywhere.
  test("the fallback cannot outrank the theme actually named", () => {
    const css = generatedStylesheet();
    expect(css).toContain(":root:not([data-theme])");
    expect(css, "a bare `:root` selector competes with every theme block").not.toMatch(
      /^:root\s*[,{]/m,
    );
  });

  test("darkness is read off the name", () => {
    expect(isDarkTheme("dark_dimmed")).toBe(true);
    expect(isDarkTheme("light_high_contrast")).toBe(false);
  });
});
