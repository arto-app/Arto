/**
 * The one script an HTML embed's frame runs.
 *
 * The frame is sandboxed without the page's origin, so the page can neither
 * measure it nor reach into it to repaint it; this script does both from the
 * inside, and talks to the page by message alone. It reports the height of
 * the figure whenever that changes, and repaints it when the page names a new
 * theme — `data-theme` on `<html>`, `color-scheme` for `light-dark()`, and
 * the `<style data-theme>` blocks switched in and out, the three ways Zenn
 * lets a figure follow the reader's theme.
 *
 * It is a string rather than a module because it is written into the frame
 * as text, and because `arto page` allowlists it by hash: a page's policy
 * reaches into a `srcdoc` frame, so a script of the frame's is held to it.
 * The build writes it out as `html-embed-frame.js` for that, and the two
 * stay one text by being this one constant. Plain ES5 for the same reason
 * nothing transforms it on the way.
 */
export const FRAME_SCRIPT = `(function () {
  var root = document.documentElement;
  function paint(theme) {
    root.setAttribute("data-theme", theme);
    root.style.colorScheme = theme;
    var styles = document.querySelectorAll("style[data-theme]");
    for (var i = 0; i < styles.length; i++) {
      styles[i].media = styles[i].getAttribute("data-theme") === theme ? "all" : "not all";
    }
  }
  function report() {
    var height = document.body.getBoundingClientRect().height;
    parent.postMessage({ artoHtmlEmbed: "height", height: height }, "*");
  }
  addEventListener("message", function (event) {
    var data = event.data;
    if (event.source !== parent || !data || data.artoHtmlEmbed !== "theme") return;
    if (data.theme === "light" || data.theme === "dark") {
      paint(data.theme);
      report();
    }
  });
  if (typeof ResizeObserver === "function") {
    new ResizeObserver(report).observe(document.body);
  }
  addEventListener("load", report);
  report();
})();`;
