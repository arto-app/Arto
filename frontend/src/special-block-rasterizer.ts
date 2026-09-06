/**
 * Rasterization functions for special blocks (Math/Mermaid) in the context menu.
 * These convert DOM elements to PNG data URLs for clipboard copy and file save.
 */

import html2canvas from "html2canvas";
import { findSvgElement, getSvgDimensions, convertSvgToDataUrl } from "./code-copy";

/**
 * Rasterize a Math block (KaTeX HTML) to PNG data URL via html2canvas.
 * Uses html2canvas because KaTeX renders as HTML + CSS with custom fonts,
 * which cannot be captured by a simple Canvas drawImage approach.
 */
export async function rasterizeMathBlock(
  element: HTMLElement,
  opaque: boolean,
): Promise<string | null> {
  try {
    const backgroundColor = opaque
      ? getComputedStyle(document.body).getPropertyValue("--bg-color").trim() || "#ffffff"
      : "transparent";

    // Ensure fonts are loaded before rasterization so KaTeX renders correctly.
    // This must happen before html2canvas because onclone is not awaited.
    await document.fonts.ready;

    const canvas = await html2canvas(element, {
      backgroundColor,
      scale: 2,
      logging: false,
      useCORS: true,
      onclone: (_clonedDoc, clonedEl) => {
        // Hide copy buttons so they don't appear in the exported image
        for (const btn of clonedEl.ownerDocument.querySelectorAll<HTMLElement>(".copy-button")) {
          btn.style.display = "none";
        }
        if (!opaque) {
          // The <pre> element inherits background-color from GitHub Markdown CSS.
          // Strip it on the clone so html2canvas renders only the transparent canvas.
          clonedEl.style.backgroundColor = "transparent";
        }
      },
    });

    return canvas.toDataURL("image/png");
  } catch (error) {
    console.error("Failed to rasterize Math block:", error);
    return null;
  }
}

/**
 * Rasterize a Mermaid SVG to PNG data URL via existing rasterizeImage.
 * Reuses convertSvgToDataUrl which resolves inherited font-family and sets
 * proper width/height attributes so the SVG renders correctly as a standalone image.
 */
export async function rasterizeMermaidBlock(
  element: HTMLElement,
  opaque: boolean,
): Promise<string | null> {
  try {
    const svg = findSvgElement(element);
    const dimensions = getSvgDimensions(svg);
    const svgDataUrl = convertSvgToDataUrl(svg, dimensions);
    return window.Arto.rasterize.image(svgDataUrl, opaque);
  } catch (error) {
    console.error("Failed to rasterize Mermaid block:", error);
    return null;
  }
}
