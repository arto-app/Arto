import { describe, test, expect } from "vitest";

import { _internal } from "./mermaid-contrast";

const { relativeLuminance, blendWithBackground } = _internal;

describe("relativeLuminance", () => {
  test("pure white has luminance 1.0", () => {
    expect(relativeLuminance({ r: 255, g: 255, b: 255 })).toBeCloseTo(1.0, 4);
  });

  test("pure black has luminance 0.0", () => {
    expect(relativeLuminance({ r: 0, g: 0, b: 0 })).toBeCloseTo(0.0, 4);
  });

  test("mid-gray has expected luminance (~0.2140)", () => {
    // sRGB (128, 128, 128) → linearized ≈ 0.2158
    const luminance = relativeLuminance({ r: 128, g: 128, b: 128 });
    expect(luminance).toBeGreaterThan(0.2);
    expect(luminance).toBeLessThan(0.23);
  });

  test("green contributes most to luminance (WCAG coefficient 0.7152)", () => {
    const redOnly = relativeLuminance({ r: 255, g: 0, b: 0 });
    const greenOnly = relativeLuminance({ r: 0, g: 255, b: 0 });
    const blueOnly = relativeLuminance({ r: 0, g: 0, b: 255 });

    // Green should have the highest luminance due to 0.7152 weight
    expect(greenOnly).toBeGreaterThan(redOnly);
    expect(greenOnly).toBeGreaterThan(blueOnly);

    // Verify WCAG coefficients: R=0.2126, G=0.7152, B=0.0722
    expect(redOnly).toBeCloseTo(0.2126, 3);
    expect(greenOnly).toBeCloseTo(0.7152, 3);
    expect(blueOnly).toBeCloseTo(0.0722, 3);
  });

  test("linearization uses sRGB gamma curve with threshold at 0.03928", () => {
    // Below threshold (10/255 ≈ 0.0392): linear mapping c/12.92
    const lowValue = relativeLuminance({ r: 10, g: 0, b: 0 });
    const expectedLinear = (10 / 255 / 12.92) * 0.2126;
    expect(lowValue).toBeCloseTo(expectedLinear, 6);

    // Above threshold: gamma curve ((c + 0.055) / 1.055) ^ 2.4
    const highValue = relativeLuminance({ r: 128, g: 0, b: 0 });
    const c = 128 / 255;
    const expectedGamma = Math.pow((c + 0.055) / 1.055, 2.4) * 0.2126;
    expect(highValue).toBeCloseTo(expectedGamma, 6);
  });
});

describe("blendWithBackground", () => {
  test("fully opaque foreground ignores background", () => {
    const fg = { r: 100, g: 150, b: 200, a: 1 };
    const bg = { r: 0, g: 0, b: 0 };
    const result = blendWithBackground(fg, bg);

    expect(result.r).toBe(100);
    expect(result.g).toBe(150);
    expect(result.b).toBe(200);
  });

  test("fully transparent foreground equals background", () => {
    const fg = { r: 255, g: 0, b: 0, a: 0 };
    const bg = { r: 100, g: 200, b: 50 };
    const result = blendWithBackground(fg, bg);

    expect(result.r).toBe(100);
    expect(result.g).toBe(200);
    expect(result.b).toBe(50);
  });

  test("50% alpha blends equally between foreground and background", () => {
    const fg = { r: 200, g: 100, b: 0, a: 0.5 };
    const bg = { r: 0, g: 0, b: 200 };
    const result = blendWithBackground(fg, bg);

    expect(result.r).toBe(100); // 0.5 * 200 + 0.5 * 0
    expect(result.g).toBe(50); // 0.5 * 100 + 0.5 * 0
    expect(result.b).toBe(100); // 0.5 * 0 + 0.5 * 200
  });

  test("result values are rounded to integers", () => {
    const fg = { r: 100, g: 100, b: 100, a: 0.33 };
    const bg = { r: 200, g: 200, b: 200 };
    const result = blendWithBackground(fg, bg);

    // 0.33 * 100 + 0.67 * 200 = 33 + 134 = 167
    expect(result.r).toBe(Math.round(0.33 * 100 + 0.67 * 200));
    expect(Number.isInteger(result.r)).toBe(true);
  });
});
