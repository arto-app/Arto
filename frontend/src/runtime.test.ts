import { describe, test, expect, vi } from "vitest";

vi.mock("../style/main.css", () => ({}));

import { init } from "./runtime";

/** Each member of a namespace and what it is, so a value cannot quietly become something else. */
function shape(value: object): Record<string, Record<string, string>> {
  return Object.fromEntries(
    Object.entries(value).map(([name, namespace]) => [
      name,
      Object.fromEntries(Object.entries(namespace).map(([key, member]) => [key, typeof member])),
    ]),
  );
}

describe("init", () => {
  // The app and the page call into these by name from Rust, so a member that
  // goes missing fails only at the call site, at run time.
  test("publishes the API the host calls into on window.Arto", () => {
    init();

    expect(shape(window.Arto)).toEqual({
      contextMenu: {
        setup: "function",
        restoreSelection: "function",
        cleanup: "function",
        fitSubmenu: "function",
      },
      render: {
        onComplete: "function",
        schedule: "function",
      },
      rasterize: {
        image: "function",
        mathBlock: "function",
        mermaidBlock: "function",
        mathElement: "function",
        mermaidElement: "function",
      },
      search: {
        setup: "function",
        find: "function",
        navigate: "function",
        navigateTo: "function",
        clear: "function",
        reapply: "function",
        setPinned: "function",
        scrollToPinnedMatch: "function",
      },
      highlights: {
        setup: "function",
        show: "function",
        describeSelection: "function",
        describeMenuSelection: "function",
        idsAtSelection: "function",
        scrollTo: "function",
        rectOf: "function",
        reveal: "function",
      },
      keyboard: {
        onKeydown: "function",
        pause: "function",
        resume: "function",
        setMenuAccelerators: "function",
        setReservedKeyOverrides: "function",
      },
      mouse: {
        onNavigate: "function",
      },
      scroll: {
        down: "function",
        up: "function",
        pageDown: "function",
        pageUp: "function",
        halfPageDown: "function",
        halfPageUp: "function",
        anchor: "function",
        toTop: "function",
        toBottom: "function",
        toHeading: "function",
        toAnchor: "function",
        reset: "function",
      },
      contentCursor: {
        next: "function",
        prev: "function",
        nextHeading: "function",
        prevHeading: "function",
        setFromContextTarget: "function",
        show: "function",
        clearCursor: "function",
        clearCursorDeferred: "function",
        syncToViewport: "function",
        getCodeText: "function",
        getCodeAsMarkdown: "function",
        getTableAsTsv: "function",
        getTableAsCsv: "function",
        getTableAsMarkdown: "function",
        getImageSrc: "function",
        getImageAsMarkdown: "function",
        getLinkHref: "function",
        getSourceLineRange: "function",
        getCurrentElement: "function",
      },
      readingPosition: {
        refresh: "function",
      },
      stickyTableHead: {
        refresh: "function",
      },
      feedback: {
        show: "function",
      },
      changes: {
        set: "function",
        clear: "function",
        next: "function",
        prev: "function",
        first: "function",
      },
      lenses: {
        collect: "function",
        markPending: "function",
        annotate: "function",
        annotateAll: "function",
        fail: "function",
        settle: "function",
        restorePage: "function",
        restoreMarks: "function",
        beginPage: "function",
        showPage: "function",
        showBlock: "function",
        showAnswers: "function",
      },
      linkPreview: {
        onRequest: "function",
        resolve: "function",
        showAtCursor: "function",
        hide: "function",
      },
      print: {
        prepare: "function",
        draw: "function",
        restore: "function",
      },
    });
  });
});
