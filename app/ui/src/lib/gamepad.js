// A controller in the app: its pad or left stick moves between what can be pressed on the page —
// the tiles, a list's rows, the side's groups, a page's buttons — to the nearest the way pressed;
// A presses it (a list of choices goes to its next), B goes back as Esc does (a window or menu
// closed, a table's page back to the library), Start plays the table pointed at or shown, and the
// right stick scrolls. The shoulder buttons are a table page's own (`TablePage.svelte`). Read
// while the window has the focus; never in cabinet mode, which takes its own keys, nor on the
// Controls page, where presses are tried out.
import { app, play } from './store.svelte.js';

const FOCUSABLE = 'button, a[href], input, select, textarea, [tabindex]:not([tabindex="-1"])';
// a stick past half its travel counts as a press
const DEAD = 0.5;
// a direction held: again after this long, then this often (ms)
const FIRST = 380;
const AGAIN = 110;
// the standard layout's buttons
const A = 0;
const B = 1;
const START = 9;
const UP = 12;
const DOWN = 13;
const LEFT = 14;
const RIGHT = 15;

/** Starts reading the controllers; what it returns stops it. */
export function startPad() {
  let frame = 0;
  // every button held as it starts, so one held then counts only once let go
  let held = new Set([A, B, START]);
  let direction = null;
  let next = 0;
  const using = () => document.documentElement.classList.add('pad');
  const mouse = () => document.documentElement.classList.remove('pad');
  window.addEventListener('mousemove', mouse);
  window.addEventListener('mousedown', mouse);

  const poll = (now) => {
    frame = requestAnimationFrame(poll);
    if (!document.hasFocus() || app.testing || app.cabinet || (app.page === 'settings' && app.settingsPage === 'controls')) {
      direction = null;
      return;
    }
    const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter(Boolean) : [];
    if (!pads.length) return;
    const down = new Set();
    let [x, y, sy] = [0, 0, 0];
    for (const p of pads) {
      p.buttons.forEach((b, i) => b?.pressed && down.add(i));
      if (Math.abs(p.axes[0] ?? 0) > Math.abs(x)) x = p.axes[0];
      if (Math.abs(p.axes[1] ?? 0) > Math.abs(y)) y = p.axes[1];
      if (Math.abs(p.axes[3] ?? 0) > Math.abs(sy)) sy = p.axes[3];
    }
    const pressed = (i) => down.has(i) && !held.has(i);
    if (pressed(A)) { using(); press(); }
    if (pressed(B)) { using(); back(); }
    if (pressed(START)) { using(); start(); }
    held = down;

    // the pad, else the left stick: one step, then steps while it's held
    const way = down.has(UP) || y < -DEAD ? 'up' : down.has(DOWN) || y > DEAD ? 'down' : down.has(LEFT) || x < -DEAD ? 'left' : down.has(RIGHT) || x > DEAD ? 'right' : null;
    if (way !== direction) {
      direction = way;
      next = now + FIRST;
      if (way) { using(); move(way); }
    } else if (way && now >= next) {
      next = now + AGAIN;
      move(way);
    }
    if (Math.abs(sy) > 0.2) scroller().scrollBy(0, sy * 28);
  };
  frame = requestAnimationFrame(poll);
  return () => {
    cancelAnimationFrame(frame);
    window.removeEventListener('mousemove', mouse);
    window.removeEventListener('mousedown', mouse);
    mouse();
  };
}

/** What can be pressed now: inside the window or menu on top, where one is open. */
function pressable() {
  const layers = document.querySelectorAll('[aria-modal="true"], [role="menu"]');
  const scope = layers.length ? layers[layers.length - 1] : document;
  return [...scope.querySelectorAll(FOCUSABLE)].filter((el) => {
    if (el.disabled || el.closest('[data-pad="skip"]')) return false;
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return false;
    return getComputedStyle(el).visibility !== 'hidden';
  });
}

/** Where the controller starts on a page: what it marks, else its first tile or row. */
function first() {
  const all = pressable();
  return all.find((el) => el.hasAttribute('data-pad-start')) || all.find((el) => el.matches('.tile .open, .row .col.name')) || all[0];
}

function focus(el) {
  if (!el) return;
  el.focus({ preventScroll: true });
  el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
}

/** The focus moved to the nearest the way pressed: by the gap between their edges that way, and
 * twice how far it lies off to the side. A range's own value goes left and right. */
function move(way) {
  const from = document.activeElement;
  const all = pressable();
  if (!from || from === document.body || !all.includes(from)) return focus(first());
  if (from.type === 'range' && (way === 'left' || way === 'right')) {
    if (way === 'left') from.stepDown();
    else from.stepUp();
    from.dispatchEvent(new Event('input', { bubbles: true }));
    from.dispatchEvent(new Event('change', { bubbles: true }));
    return;
  }
  const [dx, dy] = { up: [0, -1], down: [0, 1], left: [-1, 0], right: [1, 0] }[way];
  const a = from.getBoundingClientRect();
  const [ax, ay] = [a.left + a.width / 2, a.top + a.height / 2];
  let best = null;
  let score = Infinity;
  for (const el of all) {
    if (el === from || el.contains(from) || from.contains(el)) continue;
    const b = el.getBoundingClientRect();
    const [bx, by] = [b.left + b.width / 2, b.top + b.height / 2];
    if ((dx ? (bx - ax) * dx : (by - ay) * dy) <= 4) continue;
    const gap = Math.max(0, dx > 0 ? b.left - a.right : dx < 0 ? a.left - b.right : dy > 0 ? b.top - a.bottom : a.top - b.bottom);
    const overlap = dx ? Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) : Math.min(a.right, b.right) - Math.max(a.left, b.left);
    const s = gap + 2 * Math.max(0, -overlap);
    if (s < score) {
      score = s;
      best = el;
    }
  }
  if (best) focus(best);
}

function press() {
  const el = document.activeElement;
  if (!el || el === document.body) return focus(first());
  if (el.tagName === 'SELECT') {
    el.selectedIndex = (el.selectedIndex + 1) % Math.max(1, el.options.length);
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return;
  }
  el.click();
}

/** Back, as Esc: typing stopped first. */
function back() {
  const el = document.activeElement;
  if (el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') && el.type !== 'checkbox' && el.type !== 'radio') {
    el.blur();
    return;
  }
  (el && el !== document.body ? el : window).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true }));
}

/** PLAY: the table pointed at, else the one whose page shows. */
function start() {
  const id = document.activeElement?.closest?.('[data-table]')?.dataset.table || (app.page === 'table' ? app.tableId : null);
  const t = id && app.tables.find((x) => x.id === id);
  if (t) play(t);
}

/** What the right stick scrolls: the nearest that scrolls around the focus, else the page. */
function scroller() {
  for (let el = document.activeElement; el && el !== document.body; el = el.parentElement) {
    const s = getComputedStyle(el);
    if (/(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight) return el;
  }
  return document.scrollingElement;
}
