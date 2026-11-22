import { afterEach, describe, expect, test } from "vitest";

import { addFallback, CATALOG_ID, locale, resetForTesting, t } from "./i18n";

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
    expect(t("frontend.html_embed.too_large", { limit: "1 MB" })).toBe(
      "This embed is larger than 1 MB and is not drawn.",
    );
    expect(t("frontend.html_embed.too_large")).toBe(
      "This embed is larger than %{limit} and is not drawn.",
    );
  });
});

describe("addFallback", () => {
  test("a word a host registered falls back to the host's English", () => {
    addFallback({ host: { greeting: "Hello, %{name}" } });
    expect(t("host.greeting", { name: "reader" })).toBe("Hello, reader");
  });

  test("the window's catalog still wins over a host's English", () => {
    addFallback({ host: { farewell: "Goodbye" } });
    writeCatalog(JSON.stringify({ locale: "ja", messages: { host: { farewell: "さようなら" } } }));
    expect(t("host.farewell")).toBe("さようなら");
  });

  test("the renderer's own English comes before a host's for the same key", () => {
    addFallback({ frontend: { code_copy: { copy_image: "Host's wording" } } });
    expect(t("frontend.code_copy.copy_image")).toBe("Copy as image");
  });
});
