/**
 * The mouse's two side buttons, walking the history.
 *
 * A reader whose mouse has thumb buttons expects them to go back and forward
 * the way every browser and most native apps do — the same places `Cmd+[` and
 * the header's arrows reach. The WebView hands the presses to the page as
 * `mousedown` with `button` 3 and 4, and nothing else in the app reads those:
 * the markdown link handler takes only left and middle, and the keybinding
 * engine speaks keys alone.
 *
 * The Rust side registers the callback and turns a direction into
 * `Action::HistoryBack` / `Action::HistoryForward`; see
 * `crates/arto/src/components/app/mouse_navigation.rs`.
 */

/** Which way through the history a press asks to go. */
export type NavigationDirection = "back" | "forward";

/** The side buttons as `MouseEvent.button` reports them. */
const BACK_BUTTON = 3;
const FORWARD_BUTTON = 4;

type NavigateCallback = (direction: NavigationDirection) => void;

let currentCallback: NavigateCallback | null = null;

/**
 * The direction a `MouseEvent.button` asks for, or null when it is not one of
 * the two side buttons.
 */
export function navigationDirection(button: number): NavigationDirection | null {
  if (button === BACK_BUTTON) return "back";
  if (button === FORWARD_BUTTON) return "forward";
  return null;
}

function handleMouseDown(e: MouseEvent): void {
  const direction = navigationDirection(e.button);
  if (direction === null) return;
  // The app is one page the WebView never leaves, so whatever session history
  // the engine would walk on its own is not the history the reader means.
  e.preventDefault();
  currentCallback?.(direction);
}

/** Register a callback for side-button presses. */
export function onNavigate(callback: NavigateCallback): void {
  currentCallback = callback;
}

/** Set up the side-button listener (call once during init). */
export function setup(): void {
  document.addEventListener("mousedown", handleMouseDown);
}
