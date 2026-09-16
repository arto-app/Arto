import { defineConfig, type Plugin } from "vite";
import type { HLJSApi } from "highlight.js";
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

/**
 * A bundle is built per environment and the environments run one after
 * another, so this hook fires once per build with only that build's output in
 * `dist/`. A file a later build has yet to write is skipped rather than
 * copied, and a production run empties each consumer first — once, before the
 * first build's copy — so nothing an environment stopped emitting survives
 * there. A name that is never emitted therefore goes missing rather than
 * going stale, and the Rust side, which embeds these by name, fails to
 * compile.
 */
function syncBundlePlugin(consumers: BundleConsumer[], replace: boolean): Plugin {
  let outDir = "";
  const emptied = new Set<string>();
  return {
    name: "sync-bundle-to-consumers",
    apply: "build",
    configResolved(config) {
      outDir = path.resolve(config.root, config.build.outDir);
    },
    closeBundle() {
      for (const { dir, files } of consumers) {
        if (replace && !emptied.has(dir)) {
          fs.rmSync(dir, { recursive: true, force: true });
          emptied.add(dir);
        }
        fs.mkdirSync(dir, { recursive: true });
        if (files) {
          for (const file of files) {
            const source = path.join(outDir, file);
            if (!fs.existsSync(source)) {
              continue;
            }
            const destination = path.join(dir, file);
            // A listed file may sit in a subdirectory of its own (the icon
            // sprite does), which `replace` above has just removed.
            fs.mkdirSync(path.dirname(destination), { recursive: true });
            fs.copyFileSync(source, destination);
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
  // The app compiles in the ES module and the stylesheet and serves them from
  // its own protocol; the sprite it writes into each window's document. Naming
  // these three keeps the page's bundles, listed below, out of a binary that
  // already carries everything they hold.
  {
    dir: path.resolve(import.meta.dirname, "../crates/arto/assets/frontend"),
    files: ["main.js", "main.css", "icons/tabler-sprite.svg"],
  },
  // The page crate inlines the stylesheet, the page runtime, and whichever
  // library bundles the document it is writing calls for.
  {
    dir: path.resolve(import.meta.dirname, "../crates/arto-page/assets/frontend"),
    files: [
      "main.css",
      "page.iife.js",
      "page-mermaid.iife.js",
      "page-math.iife.js",
      "page-hljs-common.iife.js",
      "page-hljs-common.txt",
      "page-hljs-all.iife.js",
      "page-hljs-all.txt",
    ],
  },
];

/**
 * A library a page loads only when its document needs it.
 *
 * Each is built as an IIFE of its own, because that is the only way to leave
 * one out of a page: the IIFE format cannot code-split, so a dynamic import
 * would fold straight back into the bundle it was split from.
 */
const pageLibraries = [
  // The environment name is Vite's, which allows no dash; the entry name is
  // the file's, on both sides of the build.
  { environment: "pageMermaid", entry: "page-mermaid", global: "ArtoPageMermaid" },
  { environment: "pageMath", entry: "page-math", global: "ArtoPageMath" },
  // Two builds of one library, of which a page takes at most one: the
  // languages highlight.js calls common are a seventh of all of them, and
  // cover what nearly every document writes its code blocks in.
  { environment: "pageHljsCommon", entry: "page-hljs-common", global: "ArtoPageHljsCommon" },
  { environment: "pageHljsAll", entry: "page-hljs-all", global: "ArtoPageHljsAll" },
] as const;

/**
 * Write down the languages a highlight.js bundle answers to, beside it.
 *
 * `arto-page` reads the body it is writing to choose between the two builds,
 * and the choice is only as good as its idea of what is in them — a name
 * hard-coded on the Rust side would go quietly wrong on the next highlight.js
 * release. So the list is build output, like the bundle it describes, and the
 * aliases are in it because a fence saying `sh` is asking for `bash`.
 */
function hljsManifestPlugin(
  environment: string,
  load: () => Promise<{ default: HLJSApi }>,
  fileName: string,
): Plugin {
  return {
    name: `hljs-manifest:${fileName}`,
    apply: "build",
    applyToEnvironment: (candidate) => candidate.name === environment,
    async generateBundle() {
      const { default: hljs } = await load();
      const names = hljs
        .listLanguages()
        .flatMap((name) => [name, ...(hljs.getLanguage(name)?.aliases ?? [])]);
      this.emitFile({
        type: "asset",
        fileName,
        source: `${[...new Set(names)].sort().join("\n")}\n`,
      });
    },
  };
}

/** The build options for one IIFE entry under `src/`. */
function iifeEnvironment(entry: string, name: string) {
  return {
    // Everything here runs in a WebView. Without this an environment of its
    // own is taken for a server's, and every dependency is left external —
    // which for a bundle that has to be inlined whole means left out.
    consumer: "client" as const,
    build: {
      // Only the app's build, which runs first, may empty `dist/`.
      emptyOutDir: false,
      lib: {
        entry: path.resolve(import.meta.dirname, `src/${entry}.ts`),
        formats: ["iife" as const],
        name,
        fileName: () => `${entry}.iife.js`,
      },
    },
  };
}

export default defineConfig(({ mode }) => {
  // The Nix build sets VITE_OUT_DIR to its output path and copies the bundle
  // into the crate itself, so no consumer sync happens there.
  const outDir = process.env.VITE_OUT_DIR || path.resolve(import.meta.dirname, "dist");
  const consumers = process.env.VITE_OUT_DIR ? [] : bundleConsumers;
  const production = mode === "production";

  return {
    base: "/assets/frontend/",
    root: ".",
    plugins: [
      iconSpritePlugin(),
      primerThemesPlugin(),
      hljsManifestPlugin(
        "pageHljsCommon",
        () => import("highlight.js/lib/common"),
        "page-hljs-common.txt",
      ),
      hljsManifestPlugin("pageHljsAll", () => import("highlight.js"), "page-hljs-all.txt"),
      syncBundlePlugin(consumers, production),
    ],
    build: {
      outDir,
      // In dev mode, keep existing files for incremental updates
      // In production, clean the directory to avoid shipping stale artifacts
      emptyOutDir: production,
      cssCodeSplit: false,
      rollupOptions: {
        output: {
          // Emit one self-contained chunk per entry (no split chunks), so each
          // bundle is a single file with its assets inlined — which is what
          // lets a page carry one inline and leave another out.
          codeSplitting: false,
          assetFileNames: ({ names }) => {
            if (names.some((n) => n.endsWith(".css"))) return "main.css";
            return "[name][extname]";
          },
        },
      },
    },
    environments: {
      // `main.js`, the ES module the desktop app loads, with every library
      // built in: the app keeps one bundle for as long as it runs.
      client: {
        build: {
          lib: {
            entry: path.resolve(import.meta.dirname, "src/main.ts"),
            formats: ["es"],
            fileName: () => "main.js",
          },
        },
      },
      // `page.iife.js` exposes `window.ArtoRenderer` for a standalone page,
      // whose WebView — Quick Look's — loads HTML under an opaque origin
      // where ES modules are not reliable on older macOS. It leaves the
      // libraries below to their own bundles.
      page: iifeEnvironment("page", "ArtoRenderer"),
      ...Object.fromEntries(
        pageLibraries.map(({ environment, entry, global }) => [
          environment,
          iifeEnvironment(entry, global),
        ]),
      ),
    },
    builder: {
      // One after another, and the app's first: it is the build that may
      // empty `dist/`, and the copies into each crate are made as each
      // build finishes.
      async buildApp(builder) {
        await builder.build(builder.environments.client);
        await builder.build(builder.environments.page);
        for (const { environment } of pageLibraries) {
          await builder.build(builder.environments[environment]);
        }
      },
    },
  };
});
