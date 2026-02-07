/**
 * Keyboard interceptor for the keybinding engine.
 *
 * Listens for keydown events in the bubble phase and forwards them to Rust
 * via a registered callback. Component-level handlers (search_bar, tab_bar)
 * can call `event.stopPropagation()` to prevent the interceptor from seeing
 * them.
 *
 * Key decisions:
 * - Bubble phase (not capture): lets component handlers take priority
 * - IME guard: ignores events during composition (Japanese input, etc.)
 * - Focus check: blocks all keys when an input field is focused
 * - Input mode tracking: adds `keyboard-navigating` class to body on keydown,
 *   removes on mousedown — used by CSS to show/hide panel focus indicators
 * - event.repeat is forwarded: Engine decides whether to accept repeats
 * - Callback pattern: Rust registers callback via eval to bridge dioxus.send()
 */

/** Key event data sent to Rust */
interface KeyEvent {
  key: string;
  modifiers: number;
  repeat: boolean;
}

/** Modifier bitmask matching Rust Modifiers bitflags */
const SHIFT = 0b0001;
const CTRL = 0b0010;
const ALT = 0b0100;
const CMD = 0b1000;

/** Whether the interceptor is active (can be paused for key recording) */
let active = true;

/** Whether an IME composition is in progress */
let composing = false;

/** Registered callback for forwarding key events to Rust */
let keyCallback: ((data: KeyEvent) => void) | null = null;

/**
 * Initialize the keyboard interceptor.
 * Call once after the Arto main module is loaded.
 */
export function setup(): void {
  // Track IME composition state
  document.addEventListener("compositionstart", () => {
    composing = true;
  });
  document.addEventListener("compositionend", () => {
    composing = false;
  });

  // Listen in bubble phase so component stopPropagation() takes precedence
  document.addEventListener("keydown", handleKeydown, { capture: false });

  // Track input mode for panel focus indicators.
  // Mouse click → focus is self-evident, hide glow.
  // Keyboard navigation → show glow so user knows which panel is active.
  document.addEventListener("mousedown", () => {
    document.body.classList.remove("keyboard-navigating");
  });
}

/**
 * Register a callback for receiving key events.
 * Called from Rust via eval to bridge dioxus.send().
 */
export function onKeydown(callback: (data: KeyEvent) => void): void {
  keyCallback = callback;
}

/**
 * Pause the interceptor (e.g., during key recording in Preferences).
 */
export function pause(): void {
  active = false;
}

/**
 * Resume the interceptor after pausing.
 */
export function resume(): void {
  active = true;
}

/**
 * Check if the interceptor is currently active.
 */
export function isActive(): boolean {
  return active;
}

function handleKeydown(event: KeyboardEvent): void {
  if (!active) return;

  // Ignore events during IME composition
  if (composing || event.isComposing) return;

  // Build modifier bitmask
  let modifiers = 0;
  if (event.shiftKey) modifiers |= SHIFT;
  if (event.ctrlKey) modifiers |= CTRL;
  if (event.altKey) modifiers |= ALT;
  if (event.metaKey) modifiers |= CMD;

  // Check if focus is in an input field.
  // When editing text, let the browser/OS handle all keys (including Ctrl+H,
  // Ctrl+A, etc.) so IME and text-editing shortcuts work correctly.
  // Component-level handlers (e.g., search_bar's onkeydown for Escape) use
  // stopPropagation(), so they never reach this bubble-phase listener.
  const target = event.target as HTMLElement | null;
  if (isEditableElement(target)) {
    return;
  }

  // Forward the key event via registered callback
  if (keyCallback) {
    document.body.classList.add("keyboard-navigating");

    keyCallback({
      key: event.key,
      modifiers,
      repeat: event.repeat,
    });

    // Prevent default browser action for keys that the engine might handle.
    // This prevents e.g., "/" from opening Firefox's quick find.
    // If the engine doesn't match, the key event is simply consumed silently.
    event.preventDefault();
  }
}

/**
 * Check if the given element is an editable input field.
 */
function isEditableElement(el: HTMLElement | null): boolean {
  if (!el) return false;

  const tagName = el.tagName.toLowerCase();
  if (tagName === "input" || tagName === "textarea") return true;
  if (el.isContentEditable) return true;

  return false;
}
