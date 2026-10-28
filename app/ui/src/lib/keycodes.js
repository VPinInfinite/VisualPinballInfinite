// The web view's keys as SDL numbers them (USB's usage codes), as the player reads the keyboard:
// for the Controls page and cabinet mode, which read keys by the user's mapping.
export const CODES = {
  Enter: 40, Escape: 41, Backspace: 42, Tab: 43, Space: 44, Minus: 45, Equal: 46, BracketLeft: 47, BracketRight: 48, Backslash: 49,
  Semicolon: 51, Quote: 52, Backquote: 53, Comma: 54, Period: 55, Slash: 56, CapsLock: 57, PrintScreen: 70, ScrollLock: 71, Pause: 72,
  Insert: 73, Home: 74, PageUp: 75, Delete: 76, End: 77, PageDown: 78, ArrowRight: 79, ArrowLeft: 80, ArrowDown: 81, ArrowUp: 82,
  NumLock: 83, NumpadDivide: 84, NumpadMultiply: 85, NumpadSubtract: 86, NumpadAdd: 87, NumpadEnter: 88, Numpad0: 98, NumpadDecimal: 99,
  IntlBackslash: 100, ContextMenu: 101, ControlLeft: 224, ShiftLeft: 225, AltLeft: 226, MetaLeft: 227, ControlRight: 228, ShiftRight: 229,
  AltRight: 230, MetaRight: 231,
};
for (let i = 0; i < 26; i++) CODES[`Key${String.fromCharCode(65 + i)}`] = 4 + i;
for (let i = 1; i <= 9; i++) {
  CODES[`Digit${i}`] = 29 + i;
  CODES[`Numpad${i}`] = 88 + i;
}
CODES.Digit0 = 39;
for (let i = 1; i <= 12; i++) CODES[`F${i}`] = 57 + i;
for (let i = 13; i <= 24; i++) CODES[`F${i}`] = 91 + i;
