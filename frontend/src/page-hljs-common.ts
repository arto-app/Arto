/**
 * highlight.js with the languages it calls common, for a page whose document
 * names none outside them — and for one that leaves a block unlabelled, since
 * detecting a language is what this set is upstream's answer for.
 *
 * It is a seventh of the whole library, which is what makes the split worth
 * making: a page's code blocks are nearly always JavaScript, a shell, a
 * config format, or one of the other few dozen in here.
 *
 * The manifest beside it (`page-hljs-common.txt`) is what tells `arto-page`
 * which names these are, so that the two cannot drift apart across a
 * highlight.js upgrade.
 */

import hljs from "highlight.js/lib/common";

const runtime = window.ArtoRenderer;
if (runtime) {
  runtime.provideHljs(hljs);
} else {
  console.error("highlight.js loaded without the page runtime; code will stay unhighlighted");
}
