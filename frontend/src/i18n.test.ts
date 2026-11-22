import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { describe, expect, test } from "vitest";

import * as appI18n from "./app-i18n";
import rendererEnglish from "@arto-app/markdown/i18n.en.json";

const locales = path.resolve(import.meta.dirname, "../../crates/arto/locales");

/** The renderer's sources, which ask for words of their own. */
const rendererSources = path.dirname(
  createRequire(import.meta.url).resolve("@arto-app/markdown/i18n"),
);

describe("t, as the app imports it", () => {
  test("placeholders are filled, and one without a value is left as written", () => {
    expect(appI18n.t("frontend.changes.when", { since: "2 hours ago" })).toBe(", 2 hours ago");
    expect(appI18n.t("frontend.changes.when")).toBe(", %{since}");
  });

  test("the app's words and the renderer's both read in English", () => {
    expect(appI18n.t("frontend.changes.when", { since: "now" })).toBe(", now");
    expect(appI18n.t("frontend.code_copy.copy_image")).toBe("Copy as image");
  });
});

describe("the frontend's catalogs", () => {
  function keys(tree: unknown, prefix = ""): string[] {
    return Object.entries(tree as Record<string, unknown>).flatMap(([name, value]) =>
      typeof value === "string" ? [prefix + name] : keys(value, `${prefix}${name}.`),
    );
  }

  function catalog(file: string): unknown {
    return JSON.parse(readFileSync(path.join(locales, file), "utf-8"));
  }

  test("the renderer's English is the app's English for the same words", () => {
    // The renderer carries its own words so it reads in a page the app did
    // not write; the app keeps them too, so they are translated with the
    // rest of its catalog. Edited in one place only, the two would drift.
    const at = (tree: unknown, key: string): unknown =>
      key.split(".").reduce((node, part) => (node as Record<string, unknown>)?.[part], tree);
    const app = catalog("frontend.en.json");
    const drifted = keys(rendererEnglish).filter(
      (key) => at(app, key) !== at(rendererEnglish, key),
    );
    expect(drifted).toEqual([]);
  });

  test("every key the source asks for is in English", () => {
    const english = new Set([...keys(catalog("frontend.en.json")), ...keys(rendererEnglish)]);
    const sources = [import.meta.dirname, rendererSources].flatMap((dir) =>
      readdirSync(dir)
        .filter((file) => file.endsWith(".ts") && !file.endsWith(".test.ts"))
        .map((file) => path.join(dir, file)),
    );
    const unknown = sources.flatMap((file) => {
      const source = readFileSync(file, "utf-8");
      return [...source.matchAll(/\bt\("([\w.]+)"/g)]
        .map((match) => match[1])
        .filter((key) => !english.has(key))
        .map((key) => `${path.basename(file)}: ${key}`);
    });
    expect(unknown).toEqual([]);
  });
});
