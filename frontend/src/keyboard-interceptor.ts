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
 * True when running on macOS, where the primary accelerator is Cmd (metaKey).
 * On Windows/Linux the primary accelerator is Ctrl (ctrlKey). Detected from the
 * WebView user agent, which reliably reports the host OS.
 */
const IS_MAC = /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);

/**
 * Primary-modifier + key combinations reserved for native OS behavior.
 * These must not be intercepted so the system clipboard (C/V/X), select-all
 * (A), and app-quit (Q) keep working normally. The primary modifier is Cmd on
 * macOS and Ctrl on Windows/Linux (see {@link IS_MAC}).
 *
 * A reserved letter is only swallowed when the active keybinding config does
 * NOT bind it as a bare primary+letter chord (see {@link boundPrimaryKeys}) —
 * otherwise chords like the Emacs `Ctrl+x` prefix or `Ctrl+v` would never reach
 * the engine.
 */
const RESERVED_OS_PRIMARY_KEYS = new Set(["q", "c", "v", "x", "a"]);

/**
 * Lowercased single letters the active config binds as a bare primary-modifier
 * chord (Cmd+letter on macOS, Ctrl+letter on Windows/Linux), in any position of
 * any sequence, in any context. Populated from Rust via
 * {@link setReservedKeyOverrides}; these letters are excluded from reserved-key
 * swallowing so bound chords reach the keybinding engine.
 */
let boundPrimaryKeys = new Set<string>();

/** Inputs for {@link shouldSwallowReservedKey} (pure, host-independent). */
export interface ReservedKeyDecision {
  key: string;
  isMac: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  boundPrimaryKeys: ReadonlySet<string>;
}

/**
 * Decide whether a keydown is an OS-reserved primary+letter chord that must be
 * swallowed (not forwarded to the keybinding engine) to preserve native
 * clipboard / select-all / quit behavior.
 *
 * The reserved gate is per-letter, so the config-aware override is per-letter
 * too: a letter the config binds as a bare primary+letter chord is never
 * swallowed. Only fires for exactly the primary modifier (no secondary/Alt);
 * a Super/Windows key press reads as the secondary modifier off macOS and so is
 * never swallowed.
 */
export function shouldSwallowReservedKey(input: ReservedKeyDecision): boolean {
  const { key, isMac, ctrlKey, metaKey, altKey } = input;
  const primaryPressed = isMac ? metaKey : ctrlKey;
  const secondaryPressed = isMac ? ctrlKey : metaKey;
  if (!primaryPressed || secondaryPressed || altKey) return false;
  const baseKey = key.toLowerCase();
  if (!RESERVED_OS_PRIMARY_KEYS.has(baseKey)) return false;
  return !input.boundPrimaryKeys.has(baseKey);
}

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

  // Skip OS-reserved shortcuts (primary+Q/C/V/X/A) unless the config binds
  // them — handled natively. All other primary+Key combos are processed by the
  // keybinding engine. The primary modifier is Cmd on macOS and Ctrl elsewhere.
  if (
    shouldSwallowReservedKey({
      key,
      isMac: IS_MAC,
      ctrlKey: e.ctrlKey,
      metaKey: e.metaKey,
      altKey: e.altKey,
      boundPrimaryKeys,
    })
  ) {
    return;
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

/**
 * Replace the set of letters the active config binds as a bare primary-modifier
 * chord, excluding them from OS-reserved swallowing (see
 * {@link shouldSwallowReservedKey}).
 *
 * Called from Rust whenever the keybinding config changes. Each entry is a
 * single lowercase letter produced by `reserved_key_overrides` on the Rust side.
 */
export function setReservedKeyOverrides(keys: string[]): void {
  boundPrimaryKeys = new Set(keys.map((k) => k.toLowerCase()));
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
