/**
 * A standalone page's bundle: the runtime with no library built into it.
 *
 * `arto page` and Quick Look inline this in every document they write, so
 * what it carries is paid for by every page and every preview. Mermaid and
 * KaTeX are therefore left out and appended as bundles of their own, for the
 * documents that actually hold a diagram or a formula — see `page-mermaid`
 * and `page-math`, which register themselves through the provide functions
 * this one publishes.
 */

export * from "./runtime";
