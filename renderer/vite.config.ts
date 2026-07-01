import { defineConfig, type Plugin } from "vite";
import path from "path";
import fs from "fs";

import icons from "./icons.json";

function iconSpritePlugin(): Plugin {
  return {
    name: "icon-sprite-generator",
    buildStart() {
      const outlineDir = path.join(
        __dirname,
        "node_modules/@tabler/icons/icons/outline",
      );
      const filledDir = path.join(
        __dirname,
        "node_modules/@tabler/icons/icons/filled",
      );

      const outputPath = path.join(__dirname, "public/icons/tabler-sprite.svg");
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
          const attrs = isFilled
            ? ` fill="currentColor" stroke="none" stroke-width="0"`
            : "";

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
      process.env.VITE_OUT_DIR ||
      path.resolve(__dirname, "../desktop/assets/dist"),
    // In dev mode, keep existing files for incremental updates
    // In production, clean the directory to avoid shipping stale artifacts
    emptyOutDir: mode === "production",
    cssCodeSplit: false,
    lib: {
      entry: path.resolve(__dirname, "src/main.ts"),
      formats: ["es"],
    },
    rollupOptions: {
      output: {
        codeSplitting: false,
        entryFileNames: "main.js",
        assetFileNames: ({ names }) => {
          if (names.some((n) => n.endsWith(".css"))) return "main.css";
          return "[name][extname]";
        },
      },
    },
  },
}));
