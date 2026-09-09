import { currentTheme, isDarkTheme } from "./theme";

/**
 * Theme-aware `<picture>` follows the theme Arto is painting, not the OS's.
 *
 * GitHub's documented shape for a light/dark image is a `<source>` carrying
 * `media="(prefers-color-scheme: dark)"`. That query answers for the system,
 * and Arto's theme is its own setting: a reader on a light desktop reading in
 * `dark_dimmed` would otherwise get the light artwork on the dark page, which
 * is the one combination the author drew two images to avoid.
 *
 * So the query is answered here instead. Each `prefers-color-scheme` term is
 * swapped for one that is true or false by construction, and the original is
 * kept on the element so the next theme can be answered from it rather than
 * from what the last one left behind. Everything else in the media condition
 * (a width, a resolution) is left to the browser.
 */
const COLOR_SCHEME = /\(\s*prefers-color-scheme\s*:\s*(dark|light)\s*\)/gi;

/** True in every viewport there is. */
const ALWAYS = "(min-width: 0px)";

/** False in every viewport there is — no `<source>` will be picked over it. */
const NEVER = "(min-width: 999999px)";

const ORIGINAL_MEDIA = "artoMedia";

export function applyPictureTheme(root: ParentNode = document): void {
  const dark = isDarkTheme(currentTheme());

  for (const source of root.querySelectorAll("picture source[media]")) {
    if (!(source instanceof HTMLSourceElement)) {
      continue;
    }

    const original = source.dataset[ORIGINAL_MEDIA] ?? source.media;
    const resolved = original.replace(COLOR_SCHEME, (_, scheme: string) =>
      (scheme.toLowerCase() === "dark") === dark ? ALWAYS : NEVER,
    );
    if (resolved === original) {
      continue;
    }

    source.dataset[ORIGINAL_MEDIA] = original;
    if (source.media !== resolved) {
      source.media = resolved;
    }
  }
}
