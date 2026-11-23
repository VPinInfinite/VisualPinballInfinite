// A table seen from above, as the table inspector and the achievements editor draw it: each kind
// of part's fill and line, the order they're drawn in, and an outline's points for SVG.

/** each kind's fill and line on the playfield */
export const COLOURS = {
  Wall: ['#2c3547', '#56647c'],
  Ramp: ['rgba(76,194,255,0.22)', '#4cc2ff'],
  Rubber: ['none', '#9aa5b8'],
  Light: ['rgba(255,210,122,0.55)', '#ffd27a'],
  Bumper: ['rgba(255,107,129,0.55)', '#ff6b81'],
  Kicker: ['rgba(159,240,194,0.35)', '#9ff0c2'],
  Flipper: ['none', '#ffffff'],
  Target: ['none', '#ff9f43'],
  Trigger: ['rgba(123,237,159,0.25)', '#7bed9f'],
  Gate: ['none', '#c7a3ff'],
  Spinner: ['none', '#c7a3ff'],
  Flasher: ['rgba(255,255,255,0.05)', 'rgba(255,255,255,0.25)'],
  Primitive: ['#8c99ad', '#8c99ad'],
  Plunger: ['none', '#c7a3ff'],
};

/** drawn last first: flippers on top */
export const ORDER = ['Flipper', 'Bumper', 'Ramp', 'Wall', 'Rubber', 'Target', 'Kicker', 'Trigger', 'Gate', 'Spinner', 'Light', 'Flasher', 'Primitive', 'Plunger'];

/** an outline's points as SVG's points attribute */
export const points = (o) => o.points.map((q) => q.join(',')).join(' ');

/** a part's fill and line */
export const colours = (p) => COLOURS[p.kind] || ['none', '#667'];

/** the drawing's view box: the table's bounds, a margin round them */
export function viewBox(bounds) {
  const [l, t, r, b] = bounds;
  return `${l - 20} ${t - 20} ${r - l + 40} ${b - t + 40}`;
}
