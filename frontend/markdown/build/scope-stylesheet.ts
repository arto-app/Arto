/**
 * Confine a stylesheet to the rendered document, for a page that is someone
 * else's.
 *
 * Arto's own windows and the pages `arto page` writes are Arto's to style
 * whole: the tokens sit on the root, and a theme is named on `<html>`. A host
 * that places the rendered fragment in its own window owns that root, and
 * may well name a `data-theme` or a `--bg-color` of its own there. So the
 * stylesheet handed to a host starts every rule at `.markdown-viewer`, the
 * element the fragment is placed in: the tokens are declared on it, a theme
 * is named on it, and nothing outside it matches.
 */

import type { Plugin as PostcssPlugin, Rule } from "postcss";

/** The element a host places the rendered fragment in. */
export const SCOPE = ".markdown-viewer";

/** A selector that starts at the scope's own element, not merely its name. */
const AT_SCOPE = new RegExp(`^${SCOPE.replace(".", "\\.")}(?![\\w-])`);

/** A leading `html` or `body` compound, and what follows it. */
const DOCUMENT_ELEMENT = /^(html|body)(?![\w-])(\S*)(.*)$/;

/**
 * `selector` as it reads inside the scope.
 *
 * A bare `body` is the body of the rendered document, which in a host's page
 * is the viewer.
 *
 * Throws for a selector that can only ever match the host's own document
 * (`html`, `body > …`): there is no rendered content for it to style, and
 * dropping it quietly would leave the two stylesheets apart without anyone
 * deciding they should be.
 */
export function scopeSelector(selector: string): string {
  const s = selector.trim();
  if (s.startsWith(":root")) {
    return SCOPE + s.slice(":root".length);
  }
  if (s.startsWith("[data-theme")) {
    return SCOPE + s;
  }
  if (AT_SCOPE.test(s)) {
    return s;
  }
  if (s === "body") {
    return SCOPE;
  }
  const document = DOCUMENT_ELEMENT.exec(s);
  if (document) {
    const [, , , rest] = document;
    const compound = s.slice(0, s.length - rest.length);
    // A state the host's body carries (`body.keyboard-navigating`) reaches
    // the content through any number of elements, which is what a
    // descendant combinator says. Any other combinator names the body's own
    // children, which are the host's.
    if (!/^\s+[^\s>+~]/.test(rest)) {
      throw new Error(`cannot confine \`${s}\` to ${SCOPE}: it styles the host's document`);
    }
    return `${compound} ${SCOPE}${rest}`;
  }
  return `${SCOPE} ${s}`;
}

/**
 * A rule a host's page would see as written: one at the top of the sheet or
 * inside a conditional group, not one nested in another rule (whose selector
 * is relative to its parent's) or a keyframe (whose selector is a time).
 */
function isOutermost(rule: Rule): boolean {
  let parent = rule.parent;
  while (parent && parent.type === "atrule") {
    if (/keyframes$/i.test((parent as { name: string }).name)) {
      return false;
    }
    parent = parent.parent;
  }
  return !parent || parent.type === "root";
}

/**
 * Scope every rule of the stylesheets whose entry `matches`, after their
 * imports have been inlined.
 */
export function scopeStylesheetPlugin(matches: (file: string) => boolean): PostcssPlugin {
  return {
    postcssPlugin: "arto-scope-stylesheet",
    OnceExit(root, { result }) {
      const file = result.opts.from;
      if (!file || !matches(file)) {
        return;
      }
      root.walkRules((rule) => {
        if (isOutermost(rule)) {
          rule.selectors = rule.selectors.map(scopeSelector);
        }
      });
    },
  };
}
