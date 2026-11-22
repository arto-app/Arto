import { defineConfig, type Plugin } from "vite";
import type { HLJSApi } from "highlight.js";
import path from "path";

import { primerThemesPlugin } from "./build/primer-themes.ts";
import { scopeStylesheetPlugin } from "./build/scope-stylesheet.ts";
import { type BundleConsumer, syncBundlePlugin } from "./build/sync-bundle.ts";
import { FRAME_SCRIPT } from "./src/html-embed-frame.ts";

/**
 * The renderer crate's `frontend` feature hands a host the runtime and the
 * stylesheet for a document in someone else's page, and every consumer the
 * library bundles and the manifests that choose among them.
 */
const bundleConsumers: BundleConsumer[] = [
  {
    dir: path.resolve(import.meta.dirname, "../../crates/arto-markdown/assets/frontend"),
    files: [
      "embed.iife.js",
      "embed.css",
      "page-mermaid.iife.js",
      "page-math.iife.js",
      "page-hljs-common.iife.js",
      "page-hljs-common.txt",
      "page-hljs-all.iife.js",
      "page-hljs-all.txt",
      "html-embed-frame.js",
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
 * `arto_markdown::frontend` reads the body it is given to choose between the two builds,
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

/**
 * Write the script an HTML embed's frame runs out as a file of its own.
 *
 * The frontend writes it into each frame as text, and a page whose policy
 * allowlists scripts by hash — what `arto page` writes — has to name its hash
 * too, since a `srcdoc` frame is held to the page's policy as well as its
 * own. The Rust side reads this file to compute that hash, so it is the same
 * constant the frontend bundles rather than a copy.
 */
function htmlEmbedFramePlugin(environment: string): Plugin {
  return {
    name: "html-embed-frame",
    apply: "build",
    applyToEnvironment: (candidate) => candidate.name === environment,
    generateBundle() {
      this.emitFile({ type: "asset", fileName: "html-embed-frame.js", source: FRAME_SCRIPT });
    },
  };
}

/** The build options for one IIFE entry under `src/`. */
function iifeEnvironment(entry: string, name: string, emptyOutDir: boolean) {
  return {
    // Everything here runs in a WebView. Without this an environment of its
    // own is taken for a server's, and every dependency is left external —
    // which for a bundle that has to be inlined whole means left out.
    consumer: "client" as const,
    build: {
      emptyOutDir,
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
  // into the crate itself, so no consumer sync happens there. The app's build
  // writes into the same directory after this one.
  const outDir = process.env.VITE_OUT_DIR || path.resolve(import.meta.dirname, "dist");
  const consumers = process.env.VITE_OUT_DIR ? [] : bundleConsumers;
  const production = mode === "production";

  return {
    base: "/assets/frontend/",
    root: import.meta.dirname,
    plugins: [
      primerThemesPlugin(),
      hljsManifestPlugin(
        "pageHljsCommon",
        () => import("highlight.js/lib/common"),
        "page-hljs-common.txt",
      ),
      hljsManifestPlugin("pageHljsAll", () => import("highlight.js"), "page-hljs-all.txt"),
      htmlEmbedFramePlugin("embed"),
      syncBundlePlugin(consumers, production),
    ],
    css: {
      postcss: {
        plugins: [scopeStylesheetPlugin((file) => file.endsWith("/style/embed.css"))],
      },
    },
    build: {
      outDir,
      cssCodeSplit: false,
      rollupOptions: {
        output: {
          // Emit one self-contained chunk per entry (no split chunks), so each
          // bundle is a single file with its assets inlined — which is what
          // lets a page carry one inline and leave another out.
          codeSplitting: false,
          assetFileNames: "[name][extname]",
        },
      },
    },
    environments: {
      // `embed.iife.js` is the runtime for a document a host places in its
      // own page, under the global the app's page runtime also takes, so the
      // library bundles register with either alike. Its stylesheet is a file
      // of its own: the page's is the whole of Arto's, which would restyle
      // the host.
      embed: {
        ...iifeEnvironment("embed", "ArtoRenderer", production),
        build: {
          ...iifeEnvironment("embed", "ArtoRenderer", production).build,
          rollupOptions: { output: { assetFileNames: "embed.css" } },
        },
      },
      ...Object.fromEntries(
        pageLibraries.map(({ environment, entry, global }) => [
          environment,
          iifeEnvironment(entry, global, false),
        ]),
      ),
    },
    builder: {
      // One after another, `embed` first: it is the build that may empty
      // `dist/`, and the copies into the crate are made as each build
      // finishes.
      async buildApp(builder) {
        await builder.build(builder.environments.embed);
        for (const { environment } of pageLibraries) {
          await builder.build(builder.environments[environment]);
        }
      },
    },
  };
});
