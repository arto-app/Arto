/**
 * What every library bundle does once it has loaded: hand what it carries to
 * the page runtime appended before it.
 *
 * The bundles share nothing with that runtime but its global, so a bundle
 * that finds no global has nowhere to put its library; it says what the
 * reader will see instead, since nothing else on the page will.
 */
export function registerWithPage(
  register: (runtime: NonNullable<Window["ArtoRenderer"]>) => void,
  missing: string,
): void {
  const runtime = window.ArtoRenderer;
  if (runtime) {
    register(runtime);
  } else {
    console.error(missing);
  }
}
