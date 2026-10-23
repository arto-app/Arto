import fs from "fs";
import path from "path";

/**
 * Write `content` to `file` unless it already holds exactly that, and say
 * whether it wrote.
 *
 * Files generated from `buildStart` are inputs of the build that follows:
 * `vite build --watch` watches them, so rewriting one with the same content
 * would still move its mtime and start the next build, and so on forever.
 */
export function writeIfChanged(file: string, content: string): boolean {
  if (fs.existsSync(file) && fs.readFileSync(file, "utf-8") === content) {
    return false;
  }
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
  return true;
}
