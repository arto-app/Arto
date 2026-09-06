/**
 * Post-processing utility to fix text contrast in Mermaid SVG diagrams.
 *
 * When users apply custom fill colors via Mermaid's `style` directive
 * (e.g., `style NODE fill:#ff9`), the text color remains the theme default.
 * In dark mode, this results in light text on a bright background — unreadable.
 *
 * This module walks the rendered SVG, detects nodes with inline fill styles,
 * calculates the fill's relative luminance (WCAG 2.0), and sets the text color
 * to either dark or light for sufficient contrast.
 */

// WCAG relative luminance threshold: above → use dark text, below → use light text
const LUMINANCE_THRESHOLD = 0.179;

interface Rgb {
  r: number;
  g: number;
  b: number;
}

interface Rgba extends Rgb {
  a: number;
}

/**
 * Fix text contrast for all nodes with custom inline fill styles in the SVG.
 *
 * Targets `.node` (flowchart nodes) and `.cluster` (subgraph backgrounds)
 * groups that contain shapes with inline `fill` styles set by Mermaid's
 * `style` directive.
 */
export function fixTextContrast(svg: SVGSVGElement): void {
  const groups = svg.querySelectorAll(".node, .cluster");
  const { dark, light } = getTextColors();
  const bg = getBackgroundColor();

  for (const group of groups) {
    const shape = findStyledShape(group);
    if (!shape) continue;

    const fillValue = shape.style.fill;
    if (!fillValue) continue;

    const rgba = parseColor(fillValue);
    if (!rgba) continue;

    // Blend semi-transparent fills with the diagram background
    const rgb = blendWithBackground(rgba, bg);
    const luminance = relativeLuminance(rgb);
    const textColor = luminance > LUMINANCE_THRESHOLD ? dark : light;

    applyTextColor(group, textColor);
  }
}

/**
 * Get text colors from theme CSS variables.
 *
 * Reads `--light-text-color` (dark text for bright backgrounds) and
 * `--dark-text-color` (light text for dark backgrounds) from the computed
 * style of the document body (where `data-theme` is applied).
 *
 * Falls back to hardcoded values if CSS variables are not available.
 */
function getTextColors(): { dark: string; light: string } {
  const style = getComputedStyle(document.body);
  return {
    dark: style.getPropertyValue("--light-text-color").trim() || "#1f2328",
    light: style.getPropertyValue("--dark-text-color").trim() || "#e6edf3",
  };
}

/**
 * Get the diagram background color from the theme CSS variable `--bg-color`.
 * Used to blend semi-transparent fill colors before computing luminance.
 * Reads from document body where `data-theme` overrides are applied.
 */
function getBackgroundColor(): Rgb {
  const style = getComputedStyle(document.body);
  const bgVar = style.getPropertyValue("--bg-color").trim();
  if (bgVar) {
    const parsed = parseColor(bgVar);
    if (parsed) return parsed;
  }
  // Fallback: white (safe default for luminance calculation)
  return { r: 255, g: 255, b: 255 };
}

/**
 * Alpha-blend a foreground RGBA color onto an opaque background.
 * Formula: out = alpha * fg + (1 - alpha) * bg
 */
function blendWithBackground(fg: Rgba, bg: Rgb): Rgb {
  if (fg.a >= 1) return fg;
  const a = fg.a;
  return {
    r: Math.round(a * fg.r + (1 - a) * bg.r),
    g: Math.round(a * fg.g + (1 - a) * bg.g),
    b: Math.round(a * fg.b + (1 - a) * bg.b),
  };
}

/**
 * Find the first shape element with an inline fill style within a group.
 * Mermaid's `style` directive applies inline styles to shape elements
 * (rect, polygon, circle, ellipse, path) inside node groups.
 */
function findStyledShape(group: Element): SVGElement | null {
  const shapes = group.querySelectorAll(
    ":scope > rect, :scope > polygon, :scope > circle, :scope > ellipse, :scope > path, :scope > .basic",
  );
  for (const shape of shapes) {
    const svgShape = shape as SVGElement;
    if (svgShape.style?.fill) {
      return svgShape;
    }
  }
  return null;
}

/**
 * Apply the computed text color to all label elements within a node group.
 *
 * Handles two rendering modes Mermaid may use:
 * - foreignObject labels: HTML `<span class="nodeLabel">` → set CSS `color`
 * - SVG text labels: `<text>` / `<tspan>` elements → set SVG `fill`
 */
function applyTextColor(group: Element, color: string): void {
  // HTML labels inside foreignObject
  const htmlLabels = group.querySelectorAll(".nodeLabel");
  for (const label of htmlLabels) {
    (label as HTMLElement).style.color = color;
  }

  // SVG text elements (fallback for diagrams without foreignObject)
  const svgTexts = group.querySelectorAll("text, tspan");
  for (const text of svgTexts) {
    (text as SVGElement).style.fill = color;
  }
}

// Cached canvas for color parsing — reused across all parseColor calls
// to avoid repeated DOM allocations on large diagrams
let cachedCanvas: HTMLCanvasElement | null = null;
let cachedContext: CanvasRenderingContext2D | null = null;

/**
 * Parse a CSS color string to RGBA components.
 *
 * Uses a canvas 2D context for reliable parsing — the browser normalizes
 * any valid CSS color (hex, rgb, named colors, hsl, etc.) to a canonical
 * format via `ctx.fillStyle`.
 *
 * Returns `null` for invalid colors (e.g., `none`, invalid syntax).
 * Validates using `CSS.supports()` to avoid sentinel collision issues.
 */
function parseColor(color: string): Rgba | null {
  if (!cachedCanvas) {
    cachedCanvas = document.createElement("canvas");
    cachedContext = cachedCanvas.getContext("2d");
  }
  if (!cachedContext) return null;

  // Validate color string before parsing to avoid sentinel collision with black
  if (!CSS.supports("color", color)) {
    return null;
  }

  const ctx = cachedContext;
  ctx.fillStyle = color;
  const resolved = ctx.fillStyle;

  // ctx.fillStyle returns "#rrggbb" for opaque colors
  const hexMatch = resolved.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (hexMatch) {
    return {
      r: parseInt(hexMatch[1], 16),
      g: parseInt(hexMatch[2], 16),
      b: parseInt(hexMatch[3], 16),
      a: 1,
    };
  }

  // Handle rgb()/rgba() format with optional alpha channel
  const rgbMatch = resolved.match(
    /rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*(\d*\.?\d+))?\s*\)/i,
  );
  if (rgbMatch) {
    const alphaStr = rgbMatch[4];
    const a = alphaStr !== undefined ? parseFloat(alphaStr) : 1;
    return {
      r: parseInt(rgbMatch[1], 10),
      g: parseInt(rgbMatch[2], 10),
      b: parseInt(rgbMatch[3], 10),
      a: Number.isNaN(a) ? 1 : a,
    };
  }

  return null;
}

/**
 * Calculate WCAG 2.0 relative luminance from RGB values.
 *
 * Formula: L = 0.2126 * R + 0.7152 * G + 0.0722 * B
 * where R, G, B are linearized (gamma-decoded) sRGB components.
 *
 * @see https://www.w3.org/TR/WCAG20/#relativeluminancedef
 */
function relativeLuminance(rgb: Rgb): number {
  const [r, g, b] = [rgb.r / 255, rgb.g / 255, rgb.b / 255].map((c) =>
    c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4),
  );
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** @internal */
export const _internal = { relativeLuminance, blendWithBackground };
