import { defineConfig, type Plugin } from "vite";
import path from "path";
import fs from "fs";

import { type BundleConsumer, syncBundlePlugin } from "./build/sync-bundle";
import { writeIfChanged } from "./build/write-if-changed";

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

      writeIfChanged(outputPath, sprite);
    },
  };
}

/**
 * Crates that embed the bundle. The library bundles the page also needs come
 * with the released `arto-markdown` crate, built from the same package this
 * one bundles.
 */
const bundleConsumers: BundleConsumer[] = [
  // The app compiles in the ES module and the stylesheet and serves them from
  // its own protocol; the sprite it writes into each window's document. Naming
  // these three keeps the page runtime, listed below, out of a binary that
  // already carries everything it holds.
  {
    dir: path.resolve(import.meta.dirname, "../crates/arto/assets/frontend"),
    files: ["main.js", "main.css", "icons/tabler-sprite.svg"],
  },
  // The page crate inlines the stylesheet and the page runtime, and takes the
  // library bundles from the renderer crate.
  {
    dir: path.resolve(import.meta.dirname, "../crates/arto-page/assets/frontend"),
    files: ["main.css", "page.iife.js"],
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
    plugins: [iconSpritePlugin(), syncBundlePlugin(consumers, production)],
    build: {
      outDir,
      // In dev mode, keep existing files for incremental updates
      // In production, clean the directory to avoid shipping stale artifacts
      emptyOutDir: production && !process.env.VITE_OUT_DIR,
      cssCodeSplit: false,
      rollupOptions: {
        output: {
          // Emit one self-contained chunk per entry (no split chunks), so each
          // bundle is a single file with its assets inlined.
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
      // libraries to the renderer's bundles.
      page: {
        // Everything here runs in a WebView. Without this an environment of
        // its own is taken for a server's, and every dependency is left
        // external — which for a bundle that has to be inlined whole means
        // left out.
        consumer: "client",
        build: {
          // Only the app's build, which runs first, may empty `dist/`.
          emptyOutDir: false,
          lib: {
            entry: path.resolve(import.meta.dirname, "src/page.ts"),
            formats: ["iife"],
            name: "ArtoRenderer",
            fileName: () => "page.iife.js",
          },
        },
      },
    },
    builder: {
      // One after another, and the app's first: it is the build that may
      // empty `dist/`, and the copies into each crate are made as each
      // build finishes.
      async buildApp(builder) {
        await builder.build(builder.environments.client);
        await builder.build(builder.environments.page);
      },
    },
  };
});
