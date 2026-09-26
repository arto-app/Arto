/**
 * A Mermaid diagram made safe to show in a link's preview.
 *
 * The page draws with `securityLevel: "loose"`, which writes a diagram's
 * `click` links and HTML labels as the source asks. On the page that is the
 * reader's own document; in a preview it is another's, put up because the
 * pointer rested on a link, so nothing in it may run. What it shows — images
 * included — is left as drawn.
 */

/** Elements that are there to run or embed something. */
const RUNNERS = new Set(["script", "iframe", "frame", "object", "embed", "portal", "base", "meta"]);

/** Schemes a link in a preview's diagram may be followed to. */
const FOLLOWABLE = new Set(["http", "https", "mailto"]);

/**
 * Whether following `url` from the preview goes where the diagram meant: a
 * place in it, or a page or mail address. A `javascript:` link, or a `data:`
 * document, would run the linked document's code in the app; a path would be
 * read against the document on screen rather than the one the diagram is
 * from.
 */
function isFollowable(url: string): boolean {
  // A URL parser skips control characters and spaces around an address and
  // tabs and newlines inside it, so none of them decides what it names.
  const bare = Array.from(url)
    .filter((char) => char > " ")
    .join("");
  if (bare.startsWith("#")) return true;
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(bare)?.[1];
  return scheme !== undefined && FOLLOWABLE.has(scheme.toLowerCase());
}

function clean(el: Element): void {
  const tag = el.localName.toLowerCase();
  if (RUNNERS.has(tag)) {
    el.remove();
    return;
  }
  for (const name of el.getAttributeNames()) {
    const lower = name.toLowerCase();
    if (lower.startsWith("on")) {
      el.removeAttribute(name);
    } else if (
      tag === "a" &&
      lower.endsWith("href") &&
      !isFollowable(el.getAttribute(name) ?? "")
    ) {
      el.removeAttribute(name);
    }
  }
}

/**
 * The diagram in Mermaid's `markup`, with everything that would run taken
 * out, ready to be put on the page; `null` when the markup holds no diagram.
 *
 * Parsed into a document of its own, where nothing runs, and cleaned there
 * before it is brought into the page.
 */
export function inertSvg(markup: string): SVGSVGElement | null {
  const parsed = new DOMParser().parseFromString(markup, "text/html");
  const svg = parsed.body.querySelector("svg");
  if (!svg) return null;
  for (const el of [svg, ...Array.from(svg.querySelectorAll("*"))]) {
    if (el.isConnected) clean(el);
  }
  const adopted = document.importNode(svg, true);
  return adopted instanceof SVGSVGElement ? adopted : null;
}
