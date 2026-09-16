/**
 * KaTeX, for a page whose document sets a formula.
 *
 * html2canvas rides along: the one thing it draws in a page is a typeset
 * formula, copied as an image from the button on the block. The two are
 * needed by the same documents, so they travel as one bundle rather than as
 * two the body would have to be read twice to choose between.
 */

import html2canvas from "html2canvas";
import katex from "katex";

const runtime = window.ArtoRenderer;
if (runtime) {
  runtime.provideKatex(katex);
  runtime.provideHtml2Canvas(html2canvas);
} else {
  console.error("KaTeX loaded without the page runtime; formulas will stay as source");
}
