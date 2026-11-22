import fs from "fs";
import path from "path";
import type { Plugin } from "vite";

import { writeIfChanged } from "./write-if-changed.ts";

/**
 * Re-scope Primer's theme token files onto Arto's `data-theme` attribute.
 *
 * Upstream each theme is keyed by the pair GitHub's own markup carries —
 * `[data-color-mode="dark"][data-dark-theme="dark_dimmed"]` and an `auto`
 * variant guarded by `prefers-color-scheme`. Arto resolves `auto` itself and
 * names the theme it wants outright, and a light theme has to be selectable
 * as the dark-mode theme (GitHub allows that too), which those selectors
 * cannot express: `light.css` only ever matches `data-light-theme`. So the
 * first block of each file is lifted onto `[data-theme="<name>"]` and the
 * `prefers-color-scheme` duplicate is dropped.
 *
 * Every theme a user can choose is emitted, so the set the app offers is
 * decided by the Rust side alone.
 */
export function primerThemesPlugin(): Plugin {
  return {
    name: "primer-theme-generator",
    buildStart() {
      writePrimerThemes();
    },
  };
}

const packageRoot = path.join(import.meta.dirname, "..");

const themesDir = path.join(
  packageRoot,
  "node_modules/@primer/primitives/dist/css/functional/themes",
);

/**
 * Where the generated stylesheet lands; `style/embed.css` imports it, and so
 * does the app's stylesheet.
 */
const primerThemesPath = path.join(packageRoot, "style/generated/primer-themes.css");

/** Generate the stylesheet and write it, leaving an unchanged file untouched. */
export function writePrimerThemes(): string {
  const css = buildPrimerThemes();
  writeIfChanged(primerThemesPath, css);
  return css;
}

function buildPrimerThemes(): string {
  const blocks = fs
    .readdirSync(themesDir)
    .filter((file) => file.endsWith(".css"))
    .map((file) => [path.basename(file, ".css").replace(/-/g, "_"), file] as const)
    .filter(([name]) => !isForcedContrastPairing(name))
    .sort()
    .map(([name, file]) => {
      const css = fs.readFileSync(path.join(themesDir, file), "utf-8");
      // The attribute is named on `<html>`, so this one selector reaches the
      // root — whose own tokens the page scrollbar and the canvas behind the
      // document resolve against — and everything below inherits.
      const selectors = [`[data-theme="${name}"]`];
      // `light` doubles as the fallback for markup that names no theme. The
      // `:not()` is what makes it a fallback: the attribute is on the root
      // element, so a bare `:root` would match the very element a theme block
      // is meant to paint, at the same specificity, and the blocks that sort
      // after it would win over the theme actually named.
      if (name === "light") selectors.unshift(":root:not([data-theme])");
      return `${selectors.join(",\n")} {${firstBlockBody(css, file)}}`;
    });

  return blocks.join("\n\n") + "\n";
}

/**
 * Whether a theme is one GitHub swaps in for another when the operating
 * system asks for more contrast (`dark_dimmed_high_contrast` and the
 * colour-vision pairings) rather than one a user picks. Arto never names one,
 * so its tokens would be dead weight in the bundle.
 */
function isForcedContrastPairing(name: string): boolean {
  return (
    name.endsWith("_high_contrast") &&
    name !== "light_high_contrast" &&
    name !== "dark_high_contrast"
  );
}

/** The declarations of a stylesheet's first rule, braces excluded. */
function firstBlockBody(css: string, file: string): string {
  const start = css.indexOf("{");
  if (start === -1) throw new Error(`${file}: no rule to re-scope`);

  let depth = 0;
  for (let i = start; i < css.length; i++) {
    if (css[i] === "{") depth++;
    else if (css[i] === "}" && --depth === 0) return css.slice(start + 1, i);
  }
  throw new Error(`${file}: unterminated rule`);
}
