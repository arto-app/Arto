/**
 * A video in the document that cannot play gives way to what it holds.
 *
 * The Markdown pipeline writes a GitHub video attachment as a `<video>` with
 * a link to it inside. A browser shows that content only when it has no
 * `<video>` at all — not when the address fails or the codec is one the
 * WebView cannot decode, which is when the reader needs the link most. So
 * the player is swapped for its content once it reports an error.
 *
 * Media errors do not bubble, so one listener captures them for the whole
 * document, including documents rendered after it was installed.
 */

function onError(event: Event): void {
  const video = event.target;
  if (!(video instanceof HTMLVideoElement) || !video.closest(".markdown-body")) {
    return;
  }
  if (!video.querySelector("a[href]")) {
    return;
  }
  video.replaceWith(...Array.from(video.childNodes));
}

export function setup(): void {
  document.addEventListener("error", onError, true);
}

export function teardown(): void {
  document.removeEventListener("error", onError, true);
}
