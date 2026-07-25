import { describe, test, expect } from "vitest";
import { shouldSwallowReservedKey } from "./keyboard-interceptor";

// ============================================================================
// shouldSwallowReservedKey
// ============================================================================

const NONE: ReadonlySet<string> = new Set();

/** Build a decision input with sensible defaults for the fields under test. */
function decision(overrides: {
  key: string;
  isMac?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  boundPrimaryKeys?: ReadonlySet<string>;
}) {
  return {
    key: overrides.key,
    isMac: overrides.isMac ?? false,
    ctrlKey: overrides.ctrlKey ?? false,
    metaKey: overrides.metaKey ?? false,
    altKey: overrides.altKey ?? false,
    boundPrimaryKeys: overrides.boundPrimaryKeys ?? NONE,
  };
}

describe("shouldSwallowReservedKey", () => {
  test("macOS Cmd+C is reserved (native clipboard)", () => {
    expect(shouldSwallowReservedKey(decision({ key: "c", isMac: true, metaKey: true }))).toBe(true);
  });

  test("non-mac Ctrl+C is reserved when unbound", () => {
    expect(shouldSwallowReservedKey(decision({ key: "c", isMac: false, ctrlKey: true }))).toBe(
      true,
    );
  });

  test("non-mac Ctrl+X is NOT swallowed when bound by config", () => {
    // Emacs binds the Ctrl+x prefix; the config override must let it through.
    expect(
      shouldSwallowReservedKey(
        decision({
          key: "x",
          isMac: false,
          ctrlKey: true,
          boundPrimaryKeys: new Set(["x", "v", "c"]),
        }),
      ),
    ).toBe(false);
  });

  test("non-mac Ctrl+V (bound, later chord) is NOT swallowed", () => {
    expect(
      shouldSwallowReservedKey(
        decision({
          key: "v",
          isMac: false,
          ctrlKey: true,
          boundPrimaryKeys: new Set(["v"]),
        }),
      ),
    ).toBe(false);
  });

  test("non-mac uppercase key is matched case-insensitively", () => {
    expect(shouldSwallowReservedKey(decision({ key: "C", isMac: false, ctrlKey: true }))).toBe(
      true,
    );
  });

  test("Super/Windows key alone is never swallowed", () => {
    // The Windows key reads as metaKey on non-mac (the secondary modifier).
    expect(shouldSwallowReservedKey(decision({ key: "c", isMac: false, metaKey: true }))).toBe(
      false,
    );
  });

  test("non-mac Ctrl+Meta (secondary also held) is not swallowed", () => {
    expect(
      shouldSwallowReservedKey(decision({ key: "c", isMac: false, ctrlKey: true, metaKey: true })),
    ).toBe(false);
  });

  test("primary+Alt is not swallowed (Alt escapes the reserved gate)", () => {
    expect(
      shouldSwallowReservedKey(decision({ key: "v", isMac: false, ctrlKey: true, altKey: true })),
    ).toBe(false);
  });

  test("non-reserved primary+letter is not swallowed", () => {
    expect(shouldSwallowReservedKey(decision({ key: "n", isMac: false, ctrlKey: true }))).toBe(
      false,
    );
  });

  test("reserved letter without the primary modifier is not swallowed", () => {
    expect(shouldSwallowReservedKey(decision({ key: "c", isMac: false }))).toBe(false);
  });

  test("macOS Ctrl+C (secondary modifier on mac) is not swallowed", () => {
    // On macOS the primary modifier is Cmd; a bare Ctrl press must pass through.
    expect(shouldSwallowReservedKey(decision({ key: "c", isMac: true, ctrlKey: true }))).toBe(
      false,
    );
  });
});
