// Hotkeys are stored like "Ctrl+Shift+KeyS" (key names from the browser's
// KeyboardEvent.code, which the Rust side understands). This makes them nicer to read.

const names: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  PrintScreen: "Print Screen",
  ScrollLock: "Scroll Lock",
  PageUp: "Page Up",
  PageDown: "Page Down",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

/** "Ctrl+Shift+KeyS" -> "Ctrl+Shift+S" */
export function hotkeyLabel(hotkey: string): string {
  return hotkey
    .split("+")
    .map((part) => {
      if (/^Key[A-Z]$/.test(part)) return part.slice(3);
      if (/^Digit\d$/.test(part)) return part.slice(5);
      if (/^Numpad/.test(part)) return "Num " + part.slice(6);
      return names[part] ?? part;
    })
    .join("+");
}

const modifierCodes = [
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
];

/** Keys that are OK without Ctrl/Alt/Shift (they don't get in the way of typing). */
function worksAlone(code: string) {
  return /^F\d{1,2}$/.test(code) || ["PrintScreen", "Pause", "ScrollLock", "Insert"].includes(code);
}

/**
 * Turns a key press into a hotkey string.
 * Returns null while only modifiers (Ctrl, Alt...) are held, or an { error } if the
 * combination isn't allowed.
 */
export function hotkeyFromEvent(e: KeyboardEvent): string | null | { error: string } {
  if (modifierCodes.includes(e.code)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  if (parts.length === 0 && !worksAlone(e.code)) {
    return { error: "Add Ctrl, Alt or Shift. Only F-keys and Print Screen work on their own." };
  }
  parts.push(e.code);
  return parts.join("+");
}
