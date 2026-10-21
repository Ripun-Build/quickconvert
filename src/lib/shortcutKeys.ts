// Shortcut recorder: key presses → "Control+Shift+Space" (for Rust) and "⌃⇧Space" (to show).
// Pure so it runs under `node --test`. Rust has the final say on what's allowed.

export interface KeyPress {
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

const MODIFIER_CODES = new Set([
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "CapsLock",
  "Fn",
]);

const KEY_SYMBOLS: Record<string, string> = {
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Enter: "↩",
  Tab: "⇥",
  Backspace: "⌫",
};

const isModifierOnly = (press: KeyPress) => MODIFIER_CODES.has(press.code);

/** Mac-style modifiers in Apple's order: ⌃⌥⇧⌘. */
export function modifierSymbols(press: Omit<KeyPress, "code">): string {
  return `${press.ctrlKey ? "⌃" : ""}${press.altKey ? "⌥" : ""}${press.shiftKey ? "⇧" : ""}${press.metaKey ? "⌘" : ""}`;
}

export function keyLabel(code: string): string {
  return KEY_SYMBOLS[code] ?? code.replace(/^Key/, "").replace(/^Digit/, "");
}

/** What to show while recording: "⌃⇧…" until a non-modifier key arrives, then "⌃⇧K". */
export function liveLabel(press: KeyPress): string {
  return modifierSymbols(press) + (isModifierOnly(press) ? "…" : keyLabel(press.code));
}

/** "Control+Shift+KeyK", or null while only modifiers are held. */
export function toAccelerator(press: KeyPress): string | null {
  if (isModifierOnly(press)) return null;
  const parts: string[] = [];
  if (press.ctrlKey) parts.push("Control");
  if (press.altKey) parts.push("Alt");
  if (press.shiftKey) parts.push("Shift");
  if (press.metaKey) parts.push("Super");
  parts.push(press.code);
  return parts.join("+");
}
