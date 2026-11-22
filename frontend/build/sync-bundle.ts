import fs from "fs";
import path from "path";
import type { Plugin } from "vite";

/**
 * A crate that embeds part of a bundle.
 *
 * The Rust side cannot read the bundle from `dist/` directly: Dioxus'
 * `asset!()` refuses paths outside its own crate, and a crate's package
 * cannot carry files from outside its directory either. So every
 * consumer keeps its own copy under `assets/frontend/`, and
 * {@link syncBundlePlugin} refreshes those copies after each build, including
 * every rebuild in watch mode, so `dx serve` picks the change up.
 */
export interface BundleConsumer {
  /** Directory that receives the copy. */
  dir: string;
  /** Files to copy from `dist/`; everything when omitted. */
  files?: string[];
}

/**
 * Copy the finished bundle into the crates that embed it.
 *
 * A bundle is built per environment and the environments run one after
 * another, so this hook fires once per build with only that build's output in
 * `dist/`. A file a later build has yet to write is skipped rather than
 * copied, and a production run (`replace`) empties each consumer first — once,
 * before the first build's copy — so nothing an environment stopped emitting
 * survives there. A name that is never emitted therefore goes missing rather
 * than going stale, and the Rust side, which embeds these by name, fails to
 * compile. A development build only overwrites.
 */
export function syncBundlePlugin(consumers: BundleConsumer[], replace: boolean): Plugin {
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
