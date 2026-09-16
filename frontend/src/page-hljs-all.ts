/**
 * highlight.js with every language it ships, for a page whose document names
 * one the common set does not have.
 *
 * Six times the size of that set, and embedded only for the documents that
 * would otherwise show their code as plain text.
 */

import hljs from "highlight.js";

const runtime = window.ArtoRenderer;
if (runtime) {
  runtime.provideHljs(hljs);
} else {
  console.error("highlight.js loaded without the page runtime; code will stay unhighlighted");
}
