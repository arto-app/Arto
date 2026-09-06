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

export default defineConfig(({ mode }) => ({
  base: "/assets/dist/",
  root: ".",
  plugins: [iconSpritePlugin()],
  build: {
    outDir:
      process.env.VITE_OUT_DIR || path.resolve(import.meta.dirname, "../crates/arto/assets/dist"),
    // In dev mode, keep existing files for incremental updates
    // In production, clean the directory to avoid shipping stale artifacts
    emptyOutDir: mode === "production",
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
}));
