/// Keyboard interceptor for Arto keybinding system.
///
/// Intercepts keydown events in the bubble phase, skipping:
/// - IME composition events
/// - Editable element focus (input, textarea, contenteditable)
/// - Reserved OS shortcuts (Cmd+Q, Cmd+C/V/X/A)
///
/// Sends normalized key data to Rust via a registered callback.

/** Key event data sent to Rust side. */
export interface KeyEventData {
  key: string;
  modifiers: number;
  repeat: boolean;
  searchFocused: boolean;
}

type KeydownCallback = (data: KeyEventData) => void;

/** Modifier bit values matching keyboard-types Modifiers. */
const ALT = 0x01;
const CONTROL = 0x08;
const META = 0x40;
const SHIFT = 0x200;

/**
 * Cmd+Key combinations reserved for native OS behavior.
 * These must not be intercepted so the system clipboard (Cmd+C/V/X),
 * select-all (Cmd+A), and app-quit (Cmd+Q) keep working normally.
 */
const RESERVED_OS_CMD_KEYS = new Set(["q", "c", "v", "x", "a"]);

/**
 * Chords that are bound as native menu accelerators (canonical form
 * `"<modifier-bits>:<physical-code>"`, matching {@link canonicalChord}).
 *
 * These are dispatched by the OS via the menu bar, so the interceptor must NOT
 * forward them to the keybinding engine — otherwise the shortcut fires twice
 * (once by the OS menu, once by the engine). Populated from Rust via
 * {@link setMenuAccelerators}.
 */
let menuAccelerators = new Set<string>();

/**
 * Build the canonical chord key for an event, matching the Rust side.
 *
 * Uses the physical `KeyboardEvent.code` (e.g. `"KeyW"`) rather than `.key`,
 * because Alt/Option remaps the produced glyph on macOS; `.code` is stable and
 * matches how muda dispatches native accelerators.
 */
function canonicalChord(code: string, modifiers: number): string {
  return `${modifiers}:${code}`;
}

/** Minimum mouse movement (px) to switch from keyboard to mouse mode. */
const MOUSE_MOVE_THRESHOLD_SQ = 5 * 5;

let currentCallback: KeydownCallback | null = null;
let composing = false;
let paused = false;
let isKeyboardMode = false;
let lastMouseX = 0;
let lastMouseY = 0;
let mouseAnchorX = 0;
let mouseAnchorY = 0;

function isEditableElement(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (target.isContentEditable) return true;
  return false;
}

function isSearchInputFocused(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.classList.contains("search-input");
}

function buildModifiers(e: KeyboardEvent): number {
  let mods = 0;
  if (e.altKey) mods |= ALT;
  if (e.ctrlKey) mods |= CONTROL;
  if (e.metaKey) mods |= META;
  if (e.shiftKey) mods |= SHIFT;
  return mods;
}

function handleKeydown(e: KeyboardEvent): void {
  if (paused) return;
  if (composing) return;
  if (!currentCallback) return;
  const searchFocused = isSearchInputFocused(e.target);
  if (isEditableElement(e.target) && !searchFocused) return;

  const key = e.key;

  // Skip modifier-only key presses
  if (key === "Control" || key === "Shift" || key === "Alt" || key === "Meta") {
    return;
  }

  // Skip OS-reserved shortcuts (Cmd+Q/C/V/X/A) — handled by PredefinedMenuItem.
  // All other Cmd+Key combos are processed by the keybinding engine.
  if (e.metaKey && !e.ctrlKey && !e.altKey) {
    const baseKey = key.toLowerCase();
    if (RESERVED_OS_CMD_KEYS.has(baseKey)) return;
  }

  const modifiers = buildModifiers(e);

  // Skip chords owned by a native menu accelerator: the OS menu already
  // dispatches them, so forwarding to the engine would double-fire.
  if (menuAccelerators.has(canonicalChord(e.code, modifiers))) return;

  if (!isKeyboardMode) {
    isKeyboardMode = true;
    mouseAnchorX = lastMouseX;
    mouseAnchorY = lastMouseY;
    document.body.classList.add("keyboard-navigating");
    // Re-show content cursor when switching from mouse to keyboard mode
    window.Arto?.contentCursor?.show?.();
  }
  currentCallback({ key, modifiers, repeat: e.repeat, searchFocused });
}

function handleCompositionStart(): void {
  composing = true;
}

function handleCompositionEnd(): void {
  composing = false;
}

/** Register a callback for keydown events. */
export function onKeydown(callback: KeydownCallback): void {
  currentCallback = callback;
}

/**
 * Replace the set of chords owned by native menu accelerators.
 *
 * Called from Rust whenever the menu-shortcut config changes. Each entry is the
 * canonical `"<modifier-bits>:<lowercased-key>"` form produced by the Rust side.
 */
export function setMenuAccelerators(chords: string[]): void {
  menuAccelerators = new Set(chords);
}

/** Pause interceptor (e.g., during key recording in Preferences). */
export function pause(): void {
  paused = true;
}

/** Resume interceptor after pause. */
export function resume(): void {
  paused = false;
}

/** Set up the keyboard interceptor (call once during init). */
export function setup(): void {
  document.addEventListener("keydown", handleKeydown, { capture: false });
  document.addEventListener("compositionstart", handleCompositionStart);
  document.addEventListener("compositionend", handleCompositionEnd);

  // Input mode tracking: switch between keyboard and mouse modes.
  // Keyboard mode: show content cursor, disable hover on interactive blocks.
  // Mouse mode: hide content cursor, enable normal hover behavior.
  // Requires intentional mouse movement (>5px) to avoid accidental switches.
  document.addEventListener(
    "mousemove",
    (e: MouseEvent) => {
      lastMouseX = e.clientX;
      lastMouseY = e.clientY;
      if (!isKeyboardMode) return;
      const dx = e.clientX - mouseAnchorX;
      const dy = e.clientY - mouseAnchorY;
      if (dx * dx + dy * dy < MOUSE_MOVE_THRESHOLD_SQ) return;
      isKeyboardMode = false;
      document.body.classList.remove("keyboard-navigating");
    },
    { passive: true },
  );
}
