import { describe, test, expect, vi, afterEach } from "vitest";

import { registerWithPage } from "./page-library";

afterEach(() => {
  delete window.ArtoRenderer;
  vi.restoreAllMocks();
});

function pageRuntime(): NonNullable<Window["ArtoRenderer"]> {
  return {
    provideMermaid: vi.fn(),
    provideKatex: vi.fn(),
    provideHtml2Canvas: vi.fn(),
    provideHljs: vi.fn(),
  };
}

describe("registerWithPage", () => {
  test("hands the page runtime to the bundle", () => {
    const runtime = pageRuntime();
    window.ArtoRenderer = runtime;
    const register = vi.fn();

    registerWithPage(register, "unused");

    expect(register).toHaveBeenCalledWith(runtime);
  });

  test("says what is lost when the bundle loads without the runtime", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const register = vi.fn();

    registerWithPage(register, "Mermaid loaded without the page runtime");

    expect(register).not.toHaveBeenCalled();
    expect(error).toHaveBeenCalledWith("Mermaid loaded without the page runtime");
  });
});
