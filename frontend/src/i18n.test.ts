import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, test } from "vitest";

import * as appI18n from "./app-i18n";
import { CATALOG_ID, locale, resetForTesting, t } from "./i18n";
import rendererEnglish from "./i18n.en.json";

const locales = path.resolve(import.meta.dirname, "../../crates/arto/locales");

function writeCatalog(text: string): void {
  const script = document.createElement("script");
  script.type = "application/json";
  script.id = CATALOG_ID;
  script.textContent = text;
  document.head.append(script);
}

afterEach(() => {
  document.getElementById(CATALOG_ID)?.remove();
  resetForTesting();
});

describe("t", () => {
  test("a page with no catalog reads English", () => {
    expect(t("frontend.code_copy.copy_image")).toBe("Copy as image");
    expect(locale()).toBe("en");
  });

  test("the catalog the window carries wins", () => {
    writeCatalog(
      JSON.stringify({
        locale: "ja",
        messages: { frontend: { code_copy: { copy_image: "画像" } } },
      }),
    );
    expect(t("frontend.code_copy.copy_image")).toBe("画像");
    expect(locale()).toBe("ja");
  });

  test("a key the catalog lacks falls back to English, and then to itself", () => {
    writeCatalog(JSON.stringify({ locale: "ja", messages: {} }));
    expect(t("frontend.code_copy.copy_image")).toBe("Copy as image");
    expect(t("frontend.no.such.key")).toBe("frontend.no.such.key");
  });

  test("an unreadable catalog is English", () => {
    writeCatalog("{");
    expect(t("frontend.code_copy.copy_image")).toBe("Copy as image");
  });

  test("placeholders are filled, and one without a value is left as written", () => {
    expect(appI18n.t("frontend.changes.when", { since: "2 hours ago" })).toBe(", 2 hours ago");
    expect(appI18n.t("frontend.changes.when")).toBe(", %{since}");
  });

  test("a word a host registered falls back to the host's English", () => {
    expect(appI18n.t("frontend.changes.when", { since: "now" })).toBe(", now");
    expect(t("frontend.code_copy.copy_image")).toBe("Copy as image");
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
    const sources = readdirSync(import.meta.dirname).filter(
      (file) => file.endsWith(".ts") && !file.endsWith(".test.ts"),
    );
    const unknown = sources.flatMap((file) => {
      const source = readFileSync(path.join(import.meta.dirname, file), "utf-8");
      return [...source.matchAll(/\bt\("([\w.]+)"/g)]
        .map((match) => match[1])
        .filter((key) => !english.has(key))
        .map((key) => `${file}: ${key}`);
    });
    expect(unknown).toEqual([]);
  });
});
