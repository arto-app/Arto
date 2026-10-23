import { describe, expect, it } from "vitest";

import { scopeSelector } from "./scope-stylesheet.ts";

describe("scopeSelector", () => {
  it("puts the root's tokens on the viewer", () => {
    expect(scopeSelector(":root")).toBe(".markdown-viewer");
  });

  it("keeps what follows the root on the viewer itself", () => {
    expect(scopeSelector(":root:not([data-theme])")).toBe(".markdown-viewer:not([data-theme])");
  });

  it("names a theme on the viewer rather than on any element", () => {
    expect(scopeSelector('[data-theme="dark"]')).toBe('.markdown-viewer[data-theme="dark"]');
    expect(scopeSelector('[data-theme^="dark"]')).toBe('.markdown-viewer[data-theme^="dark"]');
  });

  it("leaves a selector that already starts at the viewer alone", () => {
    expect(scopeSelector(".markdown-viewer")).toBe(".markdown-viewer");
    expect(scopeSelector(".markdown-viewer .markdown-body")).toBe(
      ".markdown-viewer .markdown-body",
    );
    expect(scopeSelector(".markdown-viewer.full-width .markdown-body")).toBe(
      ".markdown-viewer.full-width .markdown-body",
    );
  });

  it("puts everything else inside the viewer", () => {
    expect(scopeSelector(".markdown-body pre")).toBe(".markdown-viewer .markdown-body pre");
    expect(scopeSelector(".hljs")).toBe(".markdown-viewer .hljs");
    expect(scopeSelector("pre.preprocessed-mermaid svg")).toBe(
      ".markdown-viewer pre.preprocessed-mermaid svg",
    );
  });

  it("does not take a class that merely starts with the viewer's name for it", () => {
    expect(scopeSelector(".markdown-viewer-toolbar")).toBe(
      ".markdown-viewer .markdown-viewer-toolbar",
    );
  });

  it("keeps a state the host's body carries in front of the viewer", () => {
    expect(scopeSelector("body.keyboard-navigating .markdown-body pre")).toBe(
      "body.keyboard-navigating .markdown-viewer .markdown-body pre",
    );
  });

  it("takes the viewer for the body of the document it holds", () => {
    // KaTeX resets its equation numbers on the body, which in a host's page
    // is the viewer: each document numbers its own.
    expect(scopeSelector("body")).toBe(".markdown-viewer");
  });

  it("refuses a rule that would style the host's own document", () => {
    expect(() => scopeSelector("html")).toThrow(/html/);
    expect(() => scopeSelector("body > .markdown-body")).toThrow(/body > \.markdown-body/);
  });
});
