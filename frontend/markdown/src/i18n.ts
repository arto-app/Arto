/**
 * The words the frontend writes into a window itself.
 *
 * A window the app opens carries the translation for its locale in
 * `#arto-i18n`, written into its head (see `assets::main_head`). English is
 * bundled, so a page with nobody to ask — one `arto page` wrote, or another
 * app's — still reads: the renderer's own words here, and whatever a host
 * adds with {@link addFallback}.
 */
import rendererEnglish from "./i18n.en.json";

export type Tree = { [key: string]: string | Tree };

interface Catalog {
  locale: string;
  messages: Tree;
}

export const CATALOG_ID = "arto-i18n";

const fallbacks: Tree[] = [rendererEnglish];

let catalog: Catalog | null = null;

function current(): Catalog {
  if (catalog) return catalog;
  const text = document.getElementById(CATALOG_ID)?.textContent;
  catalog = { locale: "en", messages: {} };
  if (text) {
    try {
      catalog = JSON.parse(text) as Catalog;
    } catch (error) {
      console.warn("Unreadable translations; using English", error);
    }
  }
  return catalog;
}

function lookup(tree: Tree, key: string): string | undefined {
  let node: string | Tree | undefined = tree;
  for (const part of key.split(".")) {
    if (typeof node !== "object") return undefined;
    node = node[part];
  }
  return typeof node === "string" ? node : undefined;
}

/**
 * Add English for words of the host's own, read when the window's catalog
 * lacks a key.
 */
export function addFallback(english: Tree): void {
  if (!fallbacks.includes(english)) fallbacks.push(english);
}

/** The interface locale, for `Intl` formatters. */
export function locale(): string {
  return current().locale;
}

/**
 * The text for `key`, with each `%{name}` replaced by `args[name]` — the
 * same placeholders the Rust side uses. A key the locale lacks falls back to
 * English, and one English lacks to the key itself.
 */
export function t(key: string, args: Record<string, string> = {}): string {
  let text = lookup(current().messages, key);
  for (const english of fallbacks) {
    text ??= lookup(english, key);
  }
  return (text ?? key).replace(/%\{(\w+)\}/g, (whole, name: string) => args[name] ?? whole);
}

/** Forget the catalog read from the page, so a test can write another. */
export function resetForTesting(): void {
  catalog = null;
}
