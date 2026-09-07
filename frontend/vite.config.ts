import { defineConfig, type Plugin } from "vite";
import path from "path";
import fs from "fs";

import icons from "./icons.json" with { type: "json" };

function iconSpritePlugin(): Plugin {
  return {
    name: "icon-sprite-generator",
    buildStart() {
      const outlineDir = path.join(import.meta.dirname, "node_modules/@tabler/icons/icons/outline");
      const filledDir = path.join(import.meta.dirname, "node_modules/@tabler/icons/icons/filled");

      const outputPath = path.join(import.meta.dirname, "public/icons/tabler-sprite.svg");
      const symbols = icons
        .map((name) => {
          // Check if this is a filled icon (e.g., "star-filled" -> filled/star.svg)
          const isFilled = name.endsWith("-filled");
          const iconName = isFilled ? name.replace(/-filled$/, "") : name;
          const iconsDir = isFilled ? filledDir : outlineDir;

          const svgPath = path.join(iconsDir, `${iconName}.svg`);
          const svg = fs.readFileSync(svgPath, "utf-8");
          const content = svg
            .replace(/<svg[^>]*>/, "")
            .replace(/<\/svg>/, "")
            .trim();

          // Filled icons need fill & stroke attributes (lost when stripping <svg> wrapper)
          const attrs = isFilled ? ` fill="currentColor" stroke="none" stroke-width="0"` : "";

          return `  <symbol id="tabler-${name}" viewBox="0 0 24 24"${attrs}>${content}</symbol>`;
        })
        .join("\n");

      const sprite = `<svg xmlns="http://www.w3.org/2000/svg" style="display: none">${symbols}</svg>`;

      fs.mkdirSync(path.dirname(outputPath), { recursive: true });
      fs.writeFileSync(outputPath, sprite);
    },
  };
}

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

const themesDir = path.join(
  import.meta.dirname,
  "node_modules/@primer/primitives/dist/css/functional/themes",
);

/** Where the generated stylesheet lands; `style/main.css` imports it. */
const primerThemesPath = path.join(import.meta.dirname, "style/generated/primer-themes.css");

/** Generate the stylesheet and write it, leaving an unchanged file untouched. */
export function writePrimerThemes(): string {
  const css = buildPrimerThemes();

  // The file is part of the CSS module graph, so `vite build --watch` watches
  // it; rewriting it from `buildStart` would trigger the next build, and so on
  // forever. Only a real change may touch the mtime.
  if (fs.existsSync(primerThemesPath) && fs.readFileSync(primerThemesPath, "utf-8") === css) {
    return css;
  }
  fs.mkdirSync(path.dirname(primerThemesPath), { recursive: true });
  fs.writeFileSync(primerThemesPath, css);
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

/**
 * Copy the finished bundle into the crates that embed it.
 *
 * The Rust side cannot read the bundle from `dist/` directly: Dioxus'
 * `asset!()` refuses paths outside its own crate, and a crate published to
 * crates.io cannot ship files from outside its directory either. So every
 * consumer keeps its own copy under `assets/frontend/`, and this plugin
 * refreshes those copies after each build, including every rebuild in watch
 * mode, so `dx serve` picks the change up.
 *
 * A production build replaces the copies wholesale so nothing stale ships; a
 * development build only overwrites, matching `emptyOutDir` below.
 */
interface BundleConsumer {
  /** Directory that receives the copy. */
  dir: string;
  /** Files to copy from `dist/`; everything when omitted. */
  files?: string[];
}

function syncBundlePlugin(consumers: BundleConsumer[], replace: boolean): Plugin {
  let outDir = "";
  return {
    name: "sync-bundle-to-consumers",
    apply: "build",
    configResolved(config) {
      outDir = path.resolve(config.root, config.build.outDir);
    },
    closeBundle() {
      for (const { dir, files } of consumers) {
        if (replace) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
        fs.mkdirSync(dir, { recursive: true });
        if (files) {
          for (const file of files) {
            fs.copyFileSync(path.join(outDir, file), path.join(dir, file));
          }
        } else {
          fs.cpSync(outDir, dir, { recursive: true });
        }
      }
    },
  };
}

/** Crates that embed the bundle. */
const bundleConsumers: BundleConsumer[] = [
  // The app serves everything (ES module, stylesheet, icon sprite) as assets.
  { dir: path.resolve(import.meta.dirname, "../crates/arto/assets/frontend") },
  // The page crate inlines only the stylesheet and the IIFE bundle.
  {
    dir: path.resolve(import.meta.dirname, "../crates/arto-page/assets/frontend"),
    files: ["main.css", "main.iife.js"],
  },
];

export default defineConfig(({ mode }) => {
  // The Nix build sets VITE_OUT_DIR to its output path and copies the bundle
  // into the crate itself, so no consumer sync happens there.
  const outDir = process.env.VITE_OUT_DIR || path.resolve(import.meta.dirname, "dist");
  const consumers = process.env.VITE_OUT_DIR ? [] : bundleConsumers;
  const production = mode === "production";

  return {
    base: "/assets/frontend/",
    root: ".",
    plugins: [iconSpritePlugin(), primerThemesPlugin(), syncBundlePlugin(consumers, production)],
    build: {
      outDir,
      // In dev mode, keep existing files for incremental updates
      // In production, clean the directory to avoid shipping stale artifacts
      emptyOutDir: production,
      cssCodeSplit: false,
      lib: {
        entry: path.resolve(import.meta.dirname, "src/main.ts"),
        // ES build (`main.js`) drives the desktop app; the IIFE build
        // (`main.iife.js`) exposes `window.ArtoRenderer` for the Quick Look
        // preview extension, whose WebView loads HTML under an opaque origin
        // where ES modules are not reliable on older macOS.
        formats: ["es", "iife"],
        name: "ArtoRenderer",
        fileName: (format) => (format === "iife" ? "main.iife.js" : "main.js"),
      },
      rollupOptions: {
        output: {
          // Emit one self-contained chunk per format (no split chunks) so both
          // `main.js` and `main.iife.js` are single files with assets inlined.
          codeSplitting: false,
          assetFileNames: ({ names }) => {
            if (names.some((n) => n.endsWith(".css"))) return "main.css";
            return "[name][extname]";
          },
        },
      },
    },
  };
});
