// The player's windows, as the Screens page and its stand-ins name and colour them.
export const KINDS = {
  playfield: { label: 'Playfield', what: 'The table itself.', colour: '#4cc2ff' },
  backglass: { label: 'Backglass', what: 'The backglass picture, or a PinUP pack’s backglass screen.', colour: '#b48cff' },
  scoreView: { label: 'Score display', what: 'The DMD or the score digits, or a PinUP pack’s DMD screen.', colour: '#ffb454' },
  topper: { label: 'Topper', what: 'A PinUP pack’s topper screen.', colour: '#5ee6a0' },
};

export const ORDER = ['playfield', 'backglass', 'scoreView', 'topper'];

// shapes by the names screens and pictures go by, width over height
const NAMED = [
  [1, '1:1'], [5 / 4, '5:4'], [4 / 3, '4:3'], [3 / 2, '3:2'], [16 / 10, '16:10'], [16 / 9, '16:9'], [2.36, '21:9'], [32 / 9, '32:9'], [4, '4:1'],
  [4 / 5, '4:5'], [3 / 4, '3:4'], [2 / 3, '2:3'], [10 / 16, '10:16'], [9 / 16, '9:16'],
];

/** A shape's name: 4:3, 16:9, 4:1 (a DMD's) …, else its width over its height. */
export function shapeName(ratio) {
  if (!(ratio > 0)) return '';
  for (const [v, name] of NAMED) if (Math.abs(ratio / v - 1) <= 0.015) return name;
  return ratio >= 1 ? `${ratio.toFixed(2)}:1` : `1:${(1 / ratio).toFixed(2)}`;
}

/** The helper windows' own settings, from their address: `#identify&n=1&name=…`. */
export function params(kind) {
  return new URLSearchParams(location.hash.slice(1).replace(new RegExp(`^${kind}&?`), ''));
}
