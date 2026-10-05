/**
 * The words the frontend writes into a window itself.
 *
 * They are kept with the app's own translations, in
 * `crates/arto/locales/frontend.<code>.json`. English is bundled here, so a
 * page with nobody to ask — one `arto page` wrote — still reads; a window
 * the app opens carries the translation for its locale in
 * `#arto-i18n`, written into its head (see `assets::main_head`).
 */
import english from "../../crates/arto/locales/frontend.en.json";

type Tree = { [key: string]: string | Tree };

interface Catalog {
  locale: string;
  messages: Tree;
}

export const CATALOG_ID = "arto-i18n";

const FALLBACK: Catalog = { locale: "en", messages: english };

let catalog: Catalog | null = null;

function current(): Catalog {
  if (catalog) return catalog;
  const text = document.getElementById(CATALOG_ID)?.textContent;
  catalog = FALLBACK;
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
  const text = lookup(current().messages, key) ?? lookup(english, key) ?? key;
  return text.replace(/%\{(\w+)\}/g, (whole, name: string) => args[name] ?? whole);
}

/** Forget the catalog read from the page, so a test can write another. */
export function resetForTesting(): void {
  catalog = null;
}
