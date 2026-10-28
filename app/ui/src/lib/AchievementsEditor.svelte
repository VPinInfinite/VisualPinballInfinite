<script>
  // The achievements editor, the table editor's second stage, inside the table inspector: the
  // table's parts down the left as the inspector lists them (those achievements can't count greyed,
  // each dragged onto a condition or clicked into the one being picked); the playfield in the
  // middle, its real look under the outlines, with Save (a draft, no version), Try it by playing and
  // Finalize and pack as its next version above it; on the right the pack — chosen among the
  // table's packs and drafts, named and described in a strip folded until opened — and its
  // achievements, the one chosen opened to change: its name, words, points, picture, and when it's
  // earned, made of the pack format's conditions — a part hit, spun or kicking; a score; a game or a
  // ball starting; balls in play; an extra ball; a tilt — each so many times in a ball, a game, some
  // seconds or over every game played, and all at once, any one, or one after another; what it
  // can't show kept as it's written. What's being made is kept while the app runs; leaving with
  // changes not saved asks first.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import PartsPanel from './PartsPanel.svelte';
  import PlayfieldView from './PlayfieldView.svelte';
  import { ORDER } from './playfield.js';
  import { app, notice, leaving, saveDraft } from './store.svelte.js';

  let { id, data, hidden = $bindable({}), header = null } = $props();

  const POINTS = [1, 2, 3, 5, 10, 15, 20, 25, 30, 50, 100];
  // the kinds that crowd the drawing, drawn only where achievements can count them
  const CROWDED = new Set(['Flasher', 'Primitive', 'Light']);
  const EVENTS = ['hit', 'spin', 'slingshot'];

  let opened = $state(null);
  let problem = $state(null);
  let busy = $state(false);

  const d = $derived(app.achDrafts[id]);
  const kept = $derived(opened && d ? (opened.packs.find((p) => p.pack.pack.id === d.which) ?? null) : null);
  const item = $derived(d && d.sel >= 0 ? d.items[d.sel] : null);
  const byName = $derived(new Map(data.parts.map((p) => [p.name.toLowerCase(), p])));
  const drawn = $derived(data.parts.filter((p) => (p.counts.length || (!CROWDED.has(p.kind) && !hidden[p.kind])) && p.outline.kind !== 'none').sort((a, b) => ORDER.indexOf(b.kind) - ORDER.indexOf(a.kind)));
  // the table file VPS knows this one as, which a pack can be made for alone
  const thisFile = $derived(opened?.fresh?.for?.tables?.[0] ?? null);
  // the part clicked, by its place in the table, shown below the playfield
  const partShown = $derived(d && d.part !== null ? (data.parts[d.part] ?? null) : null);

  $effect(() => {
    const wanted = id;
    opened = null;
    problem = null;
    invoke('ach_edit_open', { id: wanted })
      .then((o) => {
        if (wanted !== id) return;
        opened = o;
        if (!app.achDrafts[wanted]) app.achDrafts[wanted] = begin(o, choices(o)[0]?.id ?? 'new');
        else if (app.achDrafts[wanted].tried) showTried();
      })
      .catch((e) => {
        if (wanted === id) problem = String(e);
      });
  });

  // a try's end: what was earned in it
  $effect(() => {
    let stop = null;
    let gone = false;
    listen('player-ended', () => showTried()).then((u) => (gone ? u() : (stop = u)));
    return () => {
      gone = true;
      stop?.();
    };
  });

  const copy = (v) => JSON.parse(JSON.stringify(v));

  /** the packs to choose among: each kept (its draft of changes saved, where it has one), each
   *  draft of a pack not packed yet */
  function choices(o) {
    const draftOf = (pid) => o.drafts.find((x) => x.pack?.pack?.id === pid);
    const list = o.packs.map((k) => ({ id: k.pack.pack.id, label: `${k.pack.pack.name} ${k.pack.pack.version}${draftOf(k.pack.pack.id) ? ' · a draft of changes' : ''}` }));
    for (const x of o.drafts) {
      const pid = x.pack?.pack?.id;
      if (pid && !o.packs.some((k) => k.pack.pack.id === pid)) list.push({ id: pid, label: `${x.pack.pack.name || 'A new pack'} · a draft, not packed yet` });
    }
    return list;
  }

  /** a pack opened for making: its draft where one's saved, else the version kept, else a new one */
  function begin(o, which) {
    const draft = which !== 'new' ? o.drafts.find((x) => x.pack?.pack?.id === which) : null;
    if (draft) {
      const pictures = Object.fromEntries(Object.entries(draft.files || {}).filter(([, f]) => f.url).map(([k, f]) => [k, { path: f.path, url: f.url }]));
      const saved = draft.savedAt ? new Date(draft.savedAt).toLocaleString(undefined, { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' }) : '';
      const kept = copy({ ...draft, files: undefined });
      for (const x of kept.items || []) x.b = renumber(x.b);
      return { ...kept, which, pictures, sel: draft.items?.length ? 0 : -1, picking: null, part: null, said: `Your draft, saved ${saved}.`, tried: null, edits: 0, hasDraft: true, details: false };
    }
    const k = which === 'new' ? null : o.packs.find((p) => p.pack.pack.id === which);
    const pack = copy(k ? k.pack : o.fresh);
    if (!k) pack.pack.id = uuid();
    const items = (pack.achievements || []).map((a, n) => ({ k: n, a, b: parse(a.when), dirty: false, fresh: false }));
    delete pack.achievements;
    return { which: k ? which : pack.pack.id, from: k ? k.dir : null, next: k ? k.next : '1.0', pack, items, keys: items.length, pictures: {}, sel: items.length ? 0 : -1, picking: null, part: null, details: false, edits: k ? 0 : 1, said: null, tried: null, hasDraft: false };
  }

  /** a new pack's ID: a UUID, its version 4 */
  function uuid() {
    const b = crypto.getRandomValues(new Uint8Array(16));
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    const h = [...b].map((x) => x.toString(16).padStart(2, '0')).join('');
    return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
  }

  // a condition as the editor shows it: a row of one kind, so many times within a ball, a game,
  // some seconds or every game; else kept as it's written
  const names = (s) => s.split('|').map((x) => x.trim()).filter(Boolean);
  const only = (o, ...keys) => Object.keys(o).sort().join() === [...keys].sort().join();
  const whole = (n) => Number.isInteger(n) && n >= 0;
  const counted = (r) => ({ ...r, times: 1, within: 'game', seconds: 30, onBall: 1 });
  const onBallOf = (w) => (w && typeof w === 'object' && only(w, 'ball') && whole(w.ball) && w.ball >= 1 ? w.ball : null);
  function parseOne(c) {
    if (!c || typeof c !== 'object' || Array.isArray(c)) return { kind: 'hand', json: c };
    if (c.count && typeof c.count === 'object' && Object.keys(c).every((k) => ['count', 'times', 'within'].includes(k)) && (c.times === undefined || whole(c.times))) {
      const inner = parseOne(c.count);
      const w = c.within;
      const within = w === undefined ? 'game' : ['ball', 'game', 'games'].includes(w) ? w : onBallOf(w) ? 'onball' : w && typeof w === 'object' && only(w, 'seconds') && w.seconds > 0 ? 'seconds' : null;
      if (inner.kind !== 'hand' && inner.times === 1 && within) return { ...inner, times: Math.max(1, c.times || 1), within, seconds: within === 'seconds' ? w.seconds : 30, onBall: onBallOf(w) || 1 };
      return { kind: 'hand', json: c };
    }
    if (only(c, 'game') && (c.game === 'started' || c.game === 'over')) return counted({ kind: 'game', what: c.game, ball: 2 });
    if (only(c, 'ball') && whole(c.ball)) return counted({ kind: 'game', what: 'ball', ball: c.ball });
    if (only(c, 'score') && c.score && only(c.score, 'atLeast') && whole(c.score.atLeast)) return counted({ kind: 'score', atLeast: c.score.atLeast });
    if (only(c, 'ballsInPlay') && c.ballsInPlay && only(c.ballsInPlay, 'atLeast') && whole(c.ballsInPlay.atLeast)) return counted({ kind: 'balls', atLeast: c.ballsInPlay.atLeast });
    if (only(c, 'credits') && c.credits && only(c.credits, 'atLeast') && whole(c.credits.atLeast)) return counted({ kind: 'credits', atLeast: c.credits.atLeast });
    if (only(c, 'extraBall')) return counted({ kind: 'extra' });
    if (only(c, 'tilt')) return counted({ kind: 'tilt' });
    // a ROM machine's own: a lamp, a switch, a coil, by its number in the machine
    if ((only(c, 'lamp') || only(c, 'lamp', 'turns')) && whole(c.lamp)) return counted({ kind: 'lamp', number: c.lamp, on: (c.turns ?? 'on') !== 'off' });
    if ((only(c, 'switch') || only(c, 'switch', 'turns')) && whole(c.switch)) return counted({ kind: 'switch', number: c.switch, on: (c.turns ?? 'on') !== 'off' });
    if ((only(c, 'solenoid') || only(c, 'solenoid', 'fires')) && whole(c.solenoid) && c.fires !== false) return counted({ kind: 'solenoid', number: c.solenoid });
    // a table written for achievements telling its own events
    if (only(c, 'script') && typeof c.script === 'string') return counted({ kind: 'script', name: c.script });
    if (typeof c.part === 'string' && (only(c, 'part') || (only(c, 'part', 'does') && typeof c.does === 'string'))) return counted({ kind: 'part', parts: names(c.part), does: (c.does || 'any').toLowerCase() });
    return { kind: 'hand', json: c };
  }
  // each row an ID of its own while the editor's open, for picking parts into it and dropping on it
  let rowIds = 0;
  const withId = (r) => ({ ...r, uid: ++rowIds });
  function windowOf(w) {
    if (w === undefined) return { within: 'none', seconds: 60 };
    if (w === 'ball' || w === 'game') return { within: w, seconds: 60 };
    if (onBallOf(w)) return { within: 'onball', seconds: 60, onBall: w.ball };
    if (w && typeof w === 'object' && only(w, 'seconds') && w.seconds > 0) return { within: 'seconds', seconds: w.seconds };
    return null;
  }
  /** a group of conditions — the format's `then`, `all` (with a `within`: in any order) or `any` — else null */
  function groupOf(c) {
    if (!c || typeof c !== 'object' || Array.isArray(c)) return null;
    const key = ['all', 'any', 'then'].find((k) => Array.isArray(c[k]) && c[k].length);
    if (!key || !Object.keys(c).every((k) => k === key || (k === 'within' && key !== 'any') || (k === 'strict' && key === 'then'))) return null;
    const w = windowOf(c.within);
    if (!w) return null;
    const join = key === 'all' && c.within !== undefined ? 'anyorder' : key;
    return withId({ kind: 'group', join, rows: c[key].map(rowOf), within: w.within, seconds: w.seconds, onBall: w.onBall || 1, strict: c.strict === true });
  }
  const rowOf = (c) => groupOf(c) || withId(parseOne(c));
  /** an achievement's conditions: one group, and what starts it over */
  function parse(when) {
    let unless = null;
    if (when && typeof when === 'object' && !Array.isArray(when) && when.unless !== undefined) {
      const { unless: u, ...rest } = when;
      unless = withId(parseOne(u));
      when = rest;
    }
    const g = groupOf(when) || withId({ kind: 'group', join: 'all', rows: [rowOf(when)], within: 'none', seconds: 60, strict: false });
    g.unless = unless;
    return g;
  }
  /** conditions kept before groups made one, and every row numbered afresh */
  function renumber(g) {
    if (!g || g.kind !== 'group') g = { kind: 'group', join: g?.join || 'all', rows: g?.rows || [], within: 'none', seconds: 60, strict: false, unless: null };
    g.uid = ++rowIds;
    g.rows = g.rows.map((r) => (r.kind === 'group' ? renumber(r) : { ...r, uid: ++rowIds }));
    if (g.unless) g.unless = { ...g.unless, uid: ++rowIds };
    return g;
  }
  const leaves = (g) => g.rows.flatMap((r) => (r.kind === 'group' ? leaves(r) : [r])).concat(g.unless ? [g.unless] : []);
  function findRow(g, uid) {
    if (uid === null || uid === undefined) return null;
    for (const r of g.rows) {
      if (r.uid === uid) return r;
      if (r.kind === 'group') {
        const found = findRow(r, uid);
        if (found) return found;
      }
    }
    return g.unless?.uid === uid ? g.unless : null;
  }
  function bare(r) {
    switch (r.kind) {
      case 'part':
        return { part: r.parts.join('|'), does: r.does };
      case 'score':
        return { score: { atLeast: r.atLeast } };
      case 'game':
        return r.what === 'ball' ? { ball: r.ball } : { game: r.what };
      case 'balls':
        return { ballsInPlay: { atLeast: r.atLeast } };
      case 'credits':
        return { credits: { atLeast: r.atLeast } };
      case 'extra':
        return { extraBall: true };
      case 'tilt':
        return { tilt: true };
      case 'lamp':
        return r.on ? { lamp: r.number } : { lamp: r.number, turns: 'off' };
      case 'switch':
        return r.on ? { switch: r.number } : { switch: r.number, turns: 'off' };
      case 'solenoid':
        return { solenoid: r.number };
      case 'script':
        return { script: r.name };
      default:
        return copy(r.json);
    }
  }
  function groupJson(g) {
    const rows = g.rows.map(toJson).filter((x) => x !== null);
    if (!rows.length) return null;
    const c = { [g.join === 'anyorder' ? 'all' : g.join]: rows };
    if (g.join === 'then' || g.join === 'anyorder') {
      const within = g.join === 'anyorder' && g.within === 'none' ? 'game' : g.within;
      if (within !== 'none') c.within = within === 'seconds' ? { seconds: g.seconds } : within === 'onball' ? { ball: g.onBall || 1 } : within;
    }
    if (g.join === 'then' && g.strict) c.strict = true;
    return c;
  }
  function toJson(r) {
    if (r.kind === 'group') return groupJson(r);
    if (r.kind === 'hand') return copy(r.json);
    const c = bare(r);
    if (r.times <= 1 && r.within !== 'seconds' && r.within !== 'onball') return c;
    return { count: c, times: r.times, within: r.within === 'seconds' ? { seconds: r.seconds } : r.within === 'onball' ? { ball: r.onBall || 1 } : r.within };
  }
  function whenJson(b) {
    const rows = b.rows.filter((r) => r.kind !== 'group' || r.rows.length);
    const c = (rows.length === 1 ? toJson(rows[0]) : groupJson({ ...b, rows })) || {};
    return b.unless ? { ...c, unless: toJson(b.unless) } : c;
  }
  const newRow = (kind, extra = {}) => withId(counted({ kind, ...extra }));
  const newGroup = (join) => withId({ kind: 'group', join, rows: [newPart()], within: join === 'anyorder' ? 'game' : 'none', seconds: 60, onBall: 1, strict: false });
  const newPart = (parts = [], does = 'hit') => newRow('part', { parts, does });

  /** an ID made from a name: lower case, its words joined by dashes, one the pack doesn't use */
  function idFrom(name, taken) {
    const base =
      name
        .toLowerCase()
        .normalize('NFKD')
        .replace(/[̀-ͯ]/g, '')
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '') || 'achievement';
    let at = base;
    for (let n = 2; taken.has(at); n++) at = `${base}-${n}`;
    return at;
  }

  /** the pack as its file will say, and the pictures and sound chosen for it, by what they're for */
  function out() {
    const pack = copy(d.pack);
    pack.pack.version = d.next;
    const taken = new Set(d.items.filter((x) => !x.fresh).map((x) => x.a.id));
    const pictures = {};
    pack.achievements = d.items.map((x) => {
      const a = copy(x.a);
      if (x.fresh) {
        a.id = idFrom(a.name, taken);
        taken.add(a.id);
      }
      if (x.fresh || x.dirty) {
        a.when = whenJson(x.b);
        a.since = d.next;
      }
      if (d.pictures[x.k]) pictures[a.id] = d.pictures[x.k].path;
      return a;
    });
    if (d.pictures.pack) pictures.pack = d.pictures.pack.path;
    if (d.pictures.sound) pictures.sound = d.pictures.sound.path;
    return { from: d.from, pack, pictures };
  }

  /** what's still to do before it's tried or packed */
  const wrong = $derived.by(() => {
    if (!d) return [];
    const list = [];
    if (!(d.pack.pack.name || '').trim()) list.push('The pack has no name: open the pack above to name it.');
    if (!d.items.length) list.push('The pack has no achievements yet.');
    for (const x of d.items) {
      const name = (x.a.name || '').trim();
      if (!name) list.push('An achievement has no name.');
      if (leaves(x.b).some((r) => r.kind === 'part' && !r.parts.length)) list.push(`${name || 'A new achievement'}: no part picked for it yet.`);
      if (leaves(x.b).some((r) => r.kind === 'script' && !r.name.trim())) list.push(`${name || 'A new achievement'}: an event with no name.`);
      if (x.b.rows.some(function empty(r) { return r.kind === 'group' && (!r.rows.length || r.rows.some(empty)); })) list.push(`${name || 'A new achievement'}: a group with nothing in it.`);
    }
    return [...new Set(list)];
  });
  const packWants = $derived(!!d && !(d.pack.pack.name || '').trim());
  const total = $derived(d ? d.items.reduce((n, x) => n + Number(x.a.points || 0), 0) : 0);
  const usesParts = $derived(!!d && d.items.some((x) => leaves(x.b).some((r) => r.kind === 'part' && r.parts.length)));
  const onlyThisFile = $derived(!!d && !!thisFile && (d.pack.for.tables || []).some((t) => t.vps === thisFile.vps));

  function edited() {
    d.edits += 1;
    d.said = null;
  }
  function changed(x) {
    x.dirty = true;
    edited();
  }

  /** a picture's address as it shows: chosen now, else its own kept */
  const packPicture = () => d.pictures.pack?.url || kept?.pictures.pack || null;
  const pictureOf = (x) => d.pictures[x.k]?.url || (!x.fresh ? kept?.pictures[x.a.id] : null) || null;

  async function choosePicture(what) {
    try {
      const got = await invoke('ach_edit_picture');
      if (!got) return;
      d.pictures[what] = got;
      edited();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  async function chooseSound() {
    try {
      const got = await invoke('ach_edit_sound');
      if (!got) return;
      d.pictures.sound = got;
      edited();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  const soundName = $derived(d?.pictures.sound ? d.pictures.sound.path.split(/[\\/]/).pop() : d?.pack.pack.sound ? d.pack.pack.sound.split('/').pop() : null);
  function clearSound() {
    delete d.pictures.sound;
    delete d.pack.pack.sound;
    edited();
  }

  function switchTo(value) {
    leaving(() => (app.achDrafts[id] = begin(opened, value)));
  }

  function addAchievement() {
    const k = d.keys++;
    d.items.push({ k, a: { id: '', name: '', about: '', points: 5, since: d.next, when: {} }, b: Object.assign(newGroup('all'), { unless: null }), dirty: true, fresh: true });
    d.sel = d.items.length - 1;
    d.picking = null;
    d.part = null;
    edited();
  }

  let removing = $state(false);
  function removeAchievement() {
    const x = item;
    d.items.splice(d.items.indexOf(x), 1);
    d.sel = Math.min(d.sel, d.items.length - 1);
    d.picking = null;
    removing = false;
    edited();
  }

  // the parts a row counts
  const shared = (list) => list.map((n) => byName.get(n.toLowerCase())?.counts ?? EVENTS).reduce((a, c) => a.filter((e) => c.includes(e)), EVENTS);
  function said(does, list) {
    if (does === 'spin') return 'spins';
    if (does === 'slingshot') return 'fires its slingshot';
    if (does === 'any') return 'does anything';
    const kinds = new Set(list.map((n) => byName.get(n.toLowerCase())?.kind));
    if (kinds.size === 1) {
      const k = [...kinds][0];
      if (k === 'Trigger') return 'is rolled over';
      if (k === 'Kicker') return 'is entered';
      if (k === 'Gate') return 'is passed through';
    }
    return 'is hit';
  }
  // why a part can't be counted
  function whyNot(p) {
    const s = Object.fromEntries(p.settings);
    if (s.is_collidable === false) return 'the ball passes through it';
    if (p.kind === 'Flipper') return 'a flipper’s swings aren’t told to achievements';
    return 'nothing the ball does to it is told to achievements';
  }
  function hitsOff(p) {
    const s = Object.fromEntries(p.settings);
    return s.hit_event === false || s.use_hit_event === false;
  }

  /** a part put in a row, or taken out where it's in already (`toggle`) */
  function intoRow(row, p, toggle = true) {
    if (!p?.counts.length || !item || !row || row.kind !== 'part') return;
    const k = row.parts.findIndex((n) => n.toLowerCase() === p.name.toLowerCase());
    if (k >= 0) {
      if (!toggle) return;
      row.parts.splice(k, 1);
    } else row.parts.push(p.name);
    const common = shared(row.parts);
    if (row.does !== 'any' && !common.includes(row.does)) row.does = common[0] || 'hit';
    changed(item);
  }

  // a part chosen, on the playfield or in the list: in picking, in or out of the row; else what it is
  function choosePart(p) {
    if (d.picking !== null && item) {
      intoRow(findRow(item.b, d.picking), p);
      return;
    }
    d.part = p.i;
  }

  function countIt(p) {
    if (!item) return;
    const rows = item.b.rows;
    // a row no part's picked for yet takes it, else it's a row of its own
    const empty = rows.findIndex((r) => r.kind === 'part' && !r.parts.length);
    if (empty >= 0) {
      rows[empty].parts.push(p.name);
      rows[empty].does = p.counts[0];
    } else rows.push(newPart([p.name], p.counts[0]));
    d.part = null;
    changed(item);
  }

  // a part dragged from the list onto a row
  let dropRow = $state(null);
  function dropOn(e, row) {
    e.preventDefault();
    dropRow = null;
    const name = e.dataTransfer.getData('application/x-vpi-part');
    const p = name && byName.get(name.toLowerCase());
    if (!p) return;
    if (row.kind === 'part') intoRow(row, p, false);
    else countIt(p);
    d.picking = d.picking === row.uid ? row.uid : null;
  }

  // a part found by name for a row: the parts achievements can count whose names have what's typed
  let searching = $state(null);
  let searchText = $state('');
  const matches = $derived.by(() => {
    const q = searchText.trim().toLowerCase();
    if (!q) return [];
    return data.parts.filter((p) => p.counts.length && p.name.toLowerCase().includes(q)).slice(0, 12);
  });

  function addRow(kind, g = item.b) {
    if (kind.startsWith('group:')) {
      g.rows.push(newGroup(kind.slice(6)));
      changed(item);
      return;
    }
    const fresh = {
      part: newPart(),
      credits: newRow('credits', { atLeast: 4 }),
      score: newRow('score', { atLeast: 100000000 }),
      game: newRow('game', { what: 'started', ball: 2 }),
      balls: newRow('balls', { atLeast: 2 }),
      extra: newRow('extra'),
      tilt: newRow('tilt'),
      lamp: newRow('lamp', { number: 11, on: true }),
      switch: newRow('switch', { number: 11, on: true }),
      solenoid: newRow('solenoid', { number: 1 }),
      script: newRow('script', { name: '' }),
    }[kind];
    if (!fresh) return;
    g.rows.push(fresh);
    if (kind === 'part') d.picking = fresh.uid;
    changed(item);
  }
  function removeRow(g, k) {
    g.rows.splice(k, 1);
    if (g === item.b && !g.rows.length) g.rows.push(newPart());
    d.picking = null;
    changed(item);
  }
  // what starts the achievement over: none unless chosen, one condition, met each time it happens
  let unlessOpen = $state(null);
  function setUnless(kind) {
    const fresh = { part: newPart(), game: newRow('game', { what: 'ball', ball: 2 }), tilt: newRow('tilt'), switch: newRow('switch', { number: 11, on: true }), solenoid: newRow('solenoid', { number: 1 }), script: newRow('script', { name: '' }) }[kind];
    if (!fresh) return;
    item.b.unless = fresh;
    if (kind === 'part') d.picking = fresh.uid;
    changed(item);
  }
  function removeUnless() {
    item.b.unless = null;
    d.picking = null;
    changed(item);
  }

  // what the table can tell, for what's offered: each kind of condition, and why one can't be had
  function kinds(groups) {
    const c = opened?.can || {};
    const facts = c.rom ? (c.map ? 'its machine’s memory map doesn’t tell it' : 'no memory map knows its machine yet') : 'its script doesn’t tell its backglass';
    const list = [
      { v: 'part', label: 'A part on the playfield' },
      { v: 'score', label: 'The score', off: !c.score && facts },
      { v: 'game', label: 'A game starting or ending, or a ball begun', off: !c.game && !c.ball && facts },
      { v: 'balls', label: 'Balls in play at once (multiball)' },
      { v: 'extra', label: 'An extra ball earned', off: !c.extraBall && (c.rom ? facts : 'only a ROM machine tells it') },
      { v: 'credits', label: 'Credits on the machine', off: !c.credits && facts },
      { v: 'tilt', label: 'A tilt', off: !c.tilt && facts },
      { v: 'lamp', label: 'A lamp of the machine', off: !c.machine && 'ROM tables only' },
      { v: 'switch', label: 'A switch of the machine', off: !c.machine && 'ROM tables only' },
      { v: 'solenoid', label: 'A coil of the machine', off: !c.machine && 'ROM tables only' },
      { v: 'script', label: 'An event the table’s script tells', off: !c.events && 'its script tells none' },
    ];
    if (groups) list.push({ v: 'group:then', label: 'A group: one after another' }, { v: 'group:anyorder', label: 'A group: all of these, in any order' }, { v: 'group:any', label: 'A group: any one of these' });
    return list.map((o) => ({ ...o, label: o.off ? `${o.label} (${o.off})` : o.label }));
  }
  const unlessKinds = $derived(kinds(false).filter((o) => ['part', 'game', 'tilt', 'switch', 'solenoid', 'script'].includes(o.v)));
  const canSay = $derived.by(() => {
    const c = opened?.can;
    if (!c) return '';
    const told = [c.score && 'the score', (c.game || c.ball) && 'games and balls', c.credits && 'credits', c.extraBall && 'extra balls', c.tilt && 'tilts'].filter(Boolean);
    const machine = c.machine ? ' Its machine’s lamps, switches and coils too.' : '';
    const from = c.rom ? (c.map ? `its machine’s memory map (${c.rom})` : `${c.rom}: no memory map knows its machine yet`) : 'what its script tells its backglass';
    return `This table tells ${told.length ? told.join(', ') : 'nothing of its game'}, from ${from}; balls in play and its parts always.${machine}${c.events ? ' Its script tells events of its own.' : ''}`;
  });
  const coilName = (n) => {
    const c = opened?.can?.coils?.find((x) => x.n === n);
    return c ? (c.note ? `${c.name} (${c.note})` : c.name) : '';
  };
  const numbered = (re, kind) => [...new Set(data.parts.filter((p) => !kind || p.kind === kind).map((p) => re.exec(p.name)?.[1]).filter(Boolean).map(Number))].sort((a, b) => a - b);
  const switchNumbers = $derived(numbered(/^sw(\d+)/i));
  const lampNumbers = $derived(numbered(/^l(\d+)/i, 'Light'));
  const atLeast1 = (v) => Math.max(1, Math.round(Number(v)) || 1);

  function toggleThisFile(on) {
    const tables = (d.pack.for.tables || []).filter((t) => t.vps !== thisFile.vps);
    if (on) tables.push(copy(thisFile));
    d.pack.for.tables = tables;
    edited();
  }

  async function save() {
    if (busy || !d.edits) return;
    busy = true;
    try {
      await saveDraft(d);
      d.said = 'Saved as a draft, with no version yet: come back to it any time. Finalize and pack it when it’s ready.';
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      busy = false;
    }
  }

  async function pack() {
    if (wrong.length || busy) return;
    busy = true;
    try {
      const was = kept?.pack?.pack?.version;
      const packId = d.pack.pack.id;
      const saved = await invoke('ach_edit_save', { edited: out() });
      await invoke('ach_edit_draft_drop', { id: packId }).catch(() => {});
      const o = await invoke('ach_edit_open', { id });
      opened = o;
      const fits = o.packs.some((p) => p.pack.pack.id === packId);
      app.achDrafts[id] = begin(o, fits ? packId : (choices(o)[0]?.id ?? 'new'));
      app.achDrafts[id].said = was
        ? `Packed as version ${saved.version}: what’s been earned with ${was} stays earned, and its masteries keep their version.`
        : `Packed as version ${saved.version}: its achievements show on the table’s page and count as it plays.`;
      if (!fits) notice(`${saved.name} ${saved.version} is packed, but it isn’t made for this table: look in Player hub › Achievements.`);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      busy = false;
    }
  }

  let discarding = $state(false);
  async function discardDraft() {
    discarding = false;
    try {
      await invoke('ach_edit_draft_drop', { id: d.pack.pack.id });
      const o = await invoke('ach_edit_open', { id });
      opened = o;
      app.achDrafts[id] = begin(o, choices(o)[0]?.id ?? 'new');
      app.achDrafts[id].said = 'The draft went to the Recycle Bin.';
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function tryIt() {
    if (wrong.length || busy) return;
    busy = true;
    try {
      const sent = out();
      d.triedNames = Object.fromEntries(sent.pack.achievements.map((a) => [a.id, a.name]));
      await invoke('ach_edit_try', { id, edited: sent });
      d.tried = true;
      d.said = 'Playing it in softcore: the pause menu’s Achievements shows how far each has come. Nothing earned while trying is kept.';
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      busy = false;
    }
  }

  async function showTried() {
    const draft = app.achDrafts[id];
    if (!draft?.tried) return;
    draft.tried = false;
    const ids = await invoke('ach_edit_tried');
    const got = ids.map((x) => draft.triedNames?.[x]).filter(Boolean);
    draft.said = got.length ? `In your try you earned ${got.join(', ')}.` : 'In your try nothing was earned.';
  }

  const fmt = (n) => Number(n).toLocaleString('en-US');
  const json = $derived(item && d ? JSON.stringify(out().pack.achievements[d.sel], null, 2) : '');
  const glows = $derived.by(() => {
    if (!d) return [];
    const list = [];
    if (item) leaves(item.b).forEach((r) => r.kind === 'part' && r.parts.forEach((n) => byName.get(n.toLowerCase()) && list.push({ p: byName.get(n.toLowerCase()), strong: d.picking === r.uid || d.picking === null })));
    if (partShown && d.picking === null) list.push({ p: partShown, strong: true });
    return list;
  });
  const pickedRow = $derived(item && d?.picking !== null ? findRow(item.b, d.picking) : null);
  const pickedNames = $derived(pickedRow?.kind === 'part' ? pickedRow.parts : item ? leaves(item.b).flatMap((r) => (r.kind === 'part' ? r.parts : [])) : []);
</script>

{#snippet slot(src, label, onpick, big = false)}
  <button type="button" class="slot" class:big class:empty={!src} title={src ? `${label}: change it` : `${label}: add one`} aria-label={label} onclick={onpick}>
    {#if src}<img {src} alt="" />{:else}<Icon name="trophy" size={big ? 24 : 16} /><span class="plus"><Icon name="plus" size={big ? 13 : 10} width={3} /></span>{/if}
  </button>
{/snippet}

{#snippet leaf(r, g, k)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="row"
    class:active={d.picking === r.uid}
    class:dropping={dropRow === r.uid}
    ondragover={(e) => { if (e.dataTransfer.types.includes('application/x-vpi-part') && (r.kind === 'part' || r.kind !== 'hand')) { e.preventDefault(); dropRow = r.uid; } }}
    ondragleave={() => (dropRow = dropRow === r.uid ? null : dropRow)}
    ondrop={(e) => dropOn(e, r)}
  >
    {#if r.kind === 'part'}
      {@const events = shared(r.parts)}
      <div class="partline">
        {#each r.parts as pn, j (j)}
          <span class="chip-part">{pn}<button type="button" aria-label={`Take ${pn} out`} onclick={() => { r.parts.splice(j, 1); changed(item); }}>×</button></span>
        {:else}
          <span class="empty">No part yet: drag one here</span>
        {/each}
      </div>
      <div class="partline">
        <button type="button" class="btn small" class:go={d.picking === r.uid} aria-pressed={d.picking === r.uid} title="Then click objects on the playfield or in the list; again to take one out" onclick={() => (d.picking = d.picking === r.uid ? null : r.uid)}><Icon name="layers" size={13} />Pick object</button>
        {#if searching === r.uid}
          <span class="findpart">
            <Icon name="search" size={14} />
            <!-- svelte-ignore a11y_autofocus -->
            <input type="search" autofocus placeholder="A part’s name" aria-label="Find a part by name" bind:value={searchText} spellcheck="false" onkeydown={(e) => { if (e.key === 'Escape') { searching = null; searchText = ''; } else if (e.key === 'Enter' && matches[0]) { intoRow(r, matches[0], false); searchText = ''; } }} />
            {#if matches.length}
              <span class="found" role="listbox">
                {#each matches as p (p.i)}
                  <button type="button" role="option" aria-selected="false" onclick={() => { intoRow(r, p, false); searchText = ''; }}>{p.name}<span class="k">{p.kind}</span></button>
                {/each}
              </span>
            {/if}
          </span>
        {:else}
          <button type="button" class="btn small icon" title="Find a part by name" aria-label="Find a part by name" onclick={() => { searching = r.uid; searchText = ''; }}><Icon name="search" size={14} /></button>
        {/if}
        {#if events.length > 1 || r.does === 'any'}
          <select class="select" aria-label="What it does" value={r.does} onchange={(e) => { r.does = e.currentTarget.value; changed(item); }}>
            {#each r.does === 'any' && !events.includes('any') ? [...events, 'any'] : events as v (v)}<option value={v}>{said(v, r.parts)}</option>{/each}
          </select>
        {:else}
          <span>{said(r.does, r.parts)}</span>
        {/if}
      </div>
      {#if g}{@render countLine(r)}{/if}
    {:else if r.kind === 'score'}
      <span>A score of at least</span>
      <input type="number" class="big" min="1" step="1000000" value={r.atLeast} aria-label="Score" oninput={(e) => { r.atLeast = atLeast1(e.currentTarget.value); changed(item); }} />
      <span>{fmt(r.atLeast)}</span>
      {#if g}{@render countLine(r)}{/if}
    {:else if r.kind === 'game'}
      <select class="select" aria-label="Which" value={r.what} onchange={(e) => { r.what = e.currentTarget.value; changed(item); }}>
        <option value="started">A game starts</option>
        <option value="over">A game ends</option>
        <option value="ball">Ball … begins</option>
      </select>
      {#if r.what === 'ball'}<input type="number" min="1" max="10" value={r.ball} aria-label="Ball" oninput={(e) => { r.ball = atLeast1(e.currentTarget.value); changed(item); }} />{/if}
      {#if g}{@render countLine(r)}{/if}
    {:else if r.kind === 'balls'}
      <input type="number" min="1" max="10" value={r.atLeast} aria-label="Balls" oninput={(e) => { r.atLeast = atLeast1(e.currentTarget.value); changed(item); }} />
      <span>or more balls in play at once</span>
      {#if g}{@render countLine(r)}{/if}
    {:else if r.kind === 'credits'}
    <input type="number" min="1" max="99" value={r.atLeast} aria-label="Credits" oninput={(e) => { r.atLeast = atLeast1(e.currentTarget.value); changed(item); }} />
    <span>or more credits on the machine</span>
    {#if g}{@render countLine(r)}{/if}
  {:else if r.kind === 'extra'}
      <span>An extra ball is earned</span>
      {#if g}{@render countLine(r)}{/if}
    {:else if r.kind === 'tilt'}
      <span>The game is tilted</span>
      {#if g}{@render countLine(r)}{/if}
    {:else if r.kind === 'lamp' || r.kind === 'switch'}
      <span>The machine’s {r.kind}</span>
      <input type="number" min="0" max="999" list={r.kind === 'lamp' ? 'ach-lamps' : 'ach-switches'} value={r.number} aria-label="Its number" oninput={(e) => { r.number = Math.max(0, Math.round(Number(e.currentTarget.value)) || 0); changed(item); }} />
      <select class="select" aria-label="Which way" value={r.on ? 'on' : 'off'} onchange={(e) => { r.on = e.currentTarget.value === 'on'; changed(item); }}>
        <option value="on">{r.kind === 'lamp' ? 'lights' : 'closes'}</option>
        <option value="off">{r.kind === 'lamp' ? 'goes out' : 'opens'}</option>
      </select>
      {#if g}{@render countLine(r)}{/if}
      <span class="hint">By its number in the machine’s manual, as PinMAME numbers it: the box suggests those the table names.</span>
    {:else if r.kind === 'solenoid'}
      <span>The machine’s coil</span>
      <input type="number" min="0" max="999" list="ach-coils" value={r.number} aria-label="Its number" oninput={(e) => { r.number = Math.max(0, Math.round(Number(e.currentTarget.value)) || 0); changed(item); }} />
      <span>fires</span>{#if coilName(r.number)}<span class="coilname">{coilName(r.number)}</span>{/if}
      {#if g}{@render countLine(r)}{/if}
      <span class="hint">By its number in the machine’s manual, as PinMAME numbers it: the box suggests those the table names.</span>
    {:else if r.kind === 'script'}
      <span>The table tells</span>
      <input type="text" value={r.name} placeholder="Its event’s name" aria-label="The event’s name" oninput={(e) => { r.name = e.currentTarget.value; changed(item); }} />
      {#if g}{@render countLine(r)}{/if}
      <span class="hint">For tables written for achievements: their script tells its own events by name.</span>
    {:else}
      <span>Written by hand, kept as it is:</span>
      <pre>{JSON.stringify(r.json)}</pre>
    {/if}
    <button type="button" class="x" aria-label="Take this condition out" onclick={() => (g ? removeRow(g, k) : removeUnless())}>×</button>
  </div>
{/snippet}

{#snippet group(g, top)}
  <div class="group" class:nested={!top}>
    {#if !top || g.rows.length > 1}
      <div class="grouphead">
        <select class="select join" aria-label="How they go together" value={g.join} onchange={(e) => { g.join = e.currentTarget.value; if (g.join === 'anyorder' && g.within === 'none') g.within = 'game'; changed(item); }}>
          <option value="then">One after another</option>
          <option value="anyorder">All of these, in any order</option>
          <option value="all">All at the same moment</option>
          <option value="any">Any one of these</option>
        </select>
        {#if g.join === 'then' || g.join === 'anyorder'}
          <select class="select" aria-label="Done within" value={g.within} onchange={(e) => { g.within = e.currentTarget.value; changed(item); }}>
            {#if g.join === 'then'}<option value="none">no time limit</option>{/if}
            <option value="ball">in one ball</option>
            <option value="onball">on ball…</option>
            <option value="game">in one game</option>
            <option value="seconds">within some seconds</option>
          </select>
          {#if g.within === 'seconds'}
            <input type="number" min="1" value={g.seconds} aria-label="Seconds" oninput={(e) => { g.seconds = atLeast1(e.currentTarget.value); changed(item); }} /><span>seconds</span>
          {:else if g.within === 'onball'}
            <input type="number" min="1" max="10" value={g.onBall || 1} aria-label="Which ball" oninput={(e) => { g.onBall = atLeast1(e.currentTarget.value); changed(item); }} />
          {/if}
        {/if}
        {#if g.join === 'then'}
          <label class="inline"><input type="checkbox" checked={g.strict} onchange={(e) => { g.strict = e.currentTarget.checked; changed(item); }} />In exactly this order: anything else on the list starts it over</label>
        {/if}
      </div>
    {/if}
    <div class="rows">
      {#each g.rows as r, k (r.uid)}
        {#if r.kind === 'group'}
          <div class="grouprow">
            {@render group(r, false)}
            <button type="button" class="x" aria-label="Take this group out" onclick={() => removeRow(g, k)}>×</button>
          </div>
        {:else}
          {@render leaf(r, g, k)}
        {/if}
      {/each}
    </div>
    <select class="select add" aria-label={top ? 'Add a condition' : 'Add to this group'} value="" onchange={(e) => { addRow(e.currentTarget.value, g); e.currentTarget.value = ''; }}>
      <option value="">{top ? '+ Add a condition…' : '+ Add to this group…'}</option>
      {#each kinds(true) as o (o.v)}<option value={o.v} disabled={!!o.off}>{o.label}</option>{/each}
    </select>
  </div>
{/snippet}

{#snippet countLine(r)}
  <span class="times">
    <input type="number" min="1" value={r.times} aria-label="How many times" oninput={(e) => { r.times = atLeast1(e.currentTarget.value); changed(item); }} />
    <span>{r.times === 1 ? 'time' : 'times'}</span>
    <select class="select" aria-label="Within" value={r.within} onchange={(e) => { r.within = e.currentTarget.value; changed(item); }}>
      <option value="ball">in one ball</option>
      <option value="onball">on ball…</option>
      <option value="game">in one game</option>
      <option value="games">over all your games</option>
      <option value="seconds">within some seconds</option>
    </select>
    {#if r.within === 'seconds'}
      <input type="number" min="1" value={r.seconds} aria-label="Seconds" oninput={(e) => { r.seconds = atLeast1(e.currentTarget.value); changed(item); }} /><span>seconds</span>
    {:else if r.within === 'onball'}
      <input type="number" min="1" max="10" value={r.onBall || 1} aria-label="Which ball" oninput={(e) => { r.onBall = atLeast1(e.currentTarget.value); changed(item); }} />
    {/if}
  </span>
{/snippet}

<div class="editor" data-loading={!opened && !problem ? '' : undefined}>
  {#if opened}
    <datalist id="ach-coils">{#each opened.can?.coils || [] as c (c.n)}<option value={c.n} label={c.note ? `${c.name} (${c.note})` : c.name}></option>{/each}</datalist>
    <datalist id="ach-switches">{#each switchNumbers as n (n)}<option value={n}></option>{/each}</datalist>
    <datalist id="ach-lamps">{#each lampNumbers as n (n)}<option value={n}></option>{/each}</datalist>
  {/if}
  {#if problem}
    <p class="problem"><Icon name="alert" size={16} />{problem}</p>
  {:else if !opened || !d}
    <p class="quiet">Reading the table’s achievement packs…</p>
  {:else}
    <div class="topline">
      <div class="title-row">{@render header?.()}</div>
      <div class="tools">
        <button type="button" class="btn go" disabled={busy || !d.edits} title="Keeps it as a draft, with no version: as often as you like" onclick={save}><Icon name="check" size={15} />Save</button>
        <button type="button" class="btn" disabled={busy || wrong.length > 0} title={wrong.length ? 'Incomplete: open the pack (top right) to see what’s missing' : 'Plays the table with this pack, softcore, nothing kept'} onclick={tryIt}><Icon name="play" size={15} />Try it</button>
        <button type="button" class="btn" disabled={busy || wrong.length > 0} title={wrong.length ? 'Incomplete: open the pack (top right) to see what’s missing' : 'Makes it a version others can play with: its version number is set here'} onclick={pack}><Icon name="box" size={15} />Finalize and pack as {d.next}</button>
        <span class="state" class:unsaved={d.edits > 0}>{d.edits > 0 ? 'Changes not saved' : d.hasDraft ? 'Draft saved' : kept ? `Version ${kept.pack.pack.version}` : ''}</span>
      </div>
      <div class="packwrap">
        <button type="button" class="packhead" aria-expanded={d.details} onclick={() => (d.details = !d.details)}>
          {#if packPicture()}<img src={packPicture()} alt="" />{:else}<span class="no-pic"><Icon name="trophy" size={18} /></span>{/if}
          <span class="t">
            <b>{d.pack.pack.name || 'Name your pack'}</b>
            <span>{kept ? `version ${kept.pack.pack.version}` : 'not packed yet'}{d.hasDraft ? ' · draft' : ''} · {d.items.length} {d.items.length === 1 ? 'achievement' : 'achievements'} · {total} points</span>
          </span>
          <span class="pencil" title="The pack: which, its name and more"><Icon name="edit" size={15} /></span>
          {#if wrong.length}<span class="corner" title="Incomplete: open it to see what’s missing before you try or pack it"><Icon name="alert" size={13} width={2.6} /></span>{/if}
        </button>
      </div>
    </div>
    {#if d.said}<p class="said">{d.said}<button type="button" class="x2" aria-label="Close" onclick={() => (d.said = null)}>×</button></p>{/if}
    <div class="grid">
      <PartsPanel parts={data.parts} chosen={d.picking === null ? d.part : null} bind:hidden counting table={id} picked={pickedNames} onpick={choosePart} />

      <div class="middle">
        <div class="panel field">
          <PlayfieldView {id} bounds={data.bounds} {drawn} {glows} picking={d.picking !== null && !!item} onpart={(i) => choosePart(data.parts[i])}>
            {#if d.picking !== null && item}
              <span class="pickhint"><b>Pick objects to count</b> — on the playfield or in the list; click one again to take it out.</span>
              <button type="button" class="btn small go" onclick={() => (d.picking = null)}>Done</button>
            {/if}
          </PlayfieldView>
          {#if partShown && d.picking === null}
            {@const p = partShown}
            <div class="card">
              <span>
                <b>{p.name}</b> — {p.kind.toLowerCase()}.
                {#if p.counts.length}
                  Achievements can count each time it {said(p.counts[0], [p.name])}.{#if hitsOff(p)}
                    The table has its hits off: the player watches it for them itself.{/if}
                {:else}
                  Achievements can’t count it: {whyNot(p)}.
                {/if}
              </span>
              {#if p.counts.length && item}<button type="button" class="btn small go" onclick={() => countIt(p)}>Count it in this achievement</button>{/if}
            </div>
          {/if}
        </div>
      </div>

      <div class="right">
        {#if d.details}
          <div class="details" role="region" aria-label="The pack">
            {#if wrong.length}
              <div class="wrong">
                <b>Incomplete:</b>
                {#each wrong as w (w)}<span>{w}</span>{/each}
              </div>
            {/if}
            <label class="f">Pack
              <select class="select" value={d.which} onchange={(e) => switchTo(e.currentTarget.value)}>
                {#each choices(opened) as c (c.id)}<option value={c.id}>{c.label}</option>{/each}
                {#if !choices(opened).some((c) => c.id === d.which)}<option value={d.which}>{d.pack.pack.name || 'A new pack'} · not saved yet</option>{/if}
                <option value="new">A new pack of my own</option>
              </select>
            </label>
            <div class="pack-pic">
              {@render slot(packPicture(), 'The pack’s picture', () => choosePicture('pack'), true)}
              <div class="fields">
                <label class="f">Its name<input type="text" class:needs-it={packWants} value={d.pack.pack.name} placeholder="The table’s name, say" oninput={(e) => { d.pack.pack.name = e.currentTarget.value; edited(); }} /></label>
                <label class="f">Made by<input type="text" value={(d.pack.pack.authors || []).join(', ')} placeholder="Your name; several with commas" oninput={(e) => { d.pack.pack.authors = e.currentTarget.value.split(',').map((x) => x.trim()).filter(Boolean); edited(); }} /></label>
              </div>
            </div>
            <label class="f">What it’s about<input type="text" value={d.pack.pack.about || ''} placeholder="A few words" oninput={(e) => { d.pack.pack.about = e.currentTarget.value; edited(); }} /></label>
            <div class="f">Its sound when one’s earned
              <span class="soundrow">
                <span class="soundname">{soundName ?? 'The player’s own chime'}</span>
                <button type="button" class="btn small ghost" onclick={chooseSound}><Icon name="music" size={13} />Choose…</button>
                {#if soundName}<button type="button" class="btn small ghost" onclick={clearSound}>Use the chime</button>{/if}
              </span>
            </div>
            <p class="for">Made for {d.pack.for.game.name}{d.pack.for.game.maker ? ` (${[d.pack.for.game.maker, d.pack.for.game.year].filter(Boolean).join(' ')})` : ''}</p>
            {#if thisFile}
              <label class="tick"><input type="checkbox" checked={onlyThisFile} onchange={(e) => toggleThisFile(e.currentTarget.checked)} /><span>Only for this table file — its parts are named as this table names them</span></label>
            {/if}
            {#if usesParts && !(d.pack.for.tables || []).length}
              <p class="warn">Its parts are named as this table names them; other tables of {d.pack.for.game.name} may name them otherwise.</p>
            {/if}
            <div class="packbtns">
              {#if kept}<button type="button" class="btn small ghost" onclick={() => invoke('ach_edit_show', { id: d.pack.pack.id }).catch((e) => notice(String(e), { kind: 'problem' }))}><Icon name="folder" size={14} />Show its files</button>{/if}
              {#if d.hasDraft}
                {#if discarding}
                  <span class="ask">Throw the draft away? <button type="button" class="btn small" onclick={discardDraft}>Throw it away</button> <button type="button" class="btn small ghost" onclick={() => (discarding = false)}>Keep it</button></span>
                {:else}
                  <button type="button" class="btn small ghost" onclick={() => (discarding = true)}><Icon name="trash" size={14} />Throw the draft away…</button>
                {/if}
              {/if}
              <button type="button" class="btn small" onclick={() => (d.details = false)}>Done</button>
            </div>
          </div>
        {/if}
        <div class="achs">
          {#each d.items as x, n (x.k)}
            <div class="ach" class:on={n === d.sel}>
              <button
                type="button"
                class="achhead"
                aria-expanded={n === d.sel}
                onclick={() => {
                  d.sel = n === d.sel ? -1 : n;
                  d.picking = null;
                  removing = false;
                }}
              >
                {#if pictureOf(x)}<img src={pictureOf(x)} alt="" />{:else}<span class="no-pic small"><Icon name="trophy" size={14} /></span>{/if}
                <span class="t">
                  <b>{x.a.name || 'A new achievement'}{#if x.fresh}<span class="tag">new</span>{/if}</b>
                  <span>{x.a.about || '—'}</span>
                </span>
                <span class="pts">{x.a.points}</span>
              </button>
              {#if n === d.sel}
                <div class="edit">
                  <div class="top">
                    {@render slot(pictureOf(item), 'Its picture', () => choosePicture(item.k), true)}
                    <div class="fields">
                      <input type="text" class="name" value={item.a.name} placeholder="Its name: “Pop Goes the Martian”" aria-label="Its name" oninput={(e) => { item.a.name = e.currentTarget.value; edited(); }} />
                      <input type="text" value={item.a.about} placeholder="What to do, in a few words" aria-label="What to do" oninput={(e) => { item.a.about = e.currentTarget.value; edited(); }} />
                      <div class="inline">
                        <select class="select" aria-label="Points" value={String(item.a.points)} onchange={(e) => { item.a.points = Number(e.currentTarget.value); edited(); }}>
                          {#each [...new Set([...POINTS, item.a.points])].sort((a, b) => a - b) as p (p)}
                            <option value={String(p)}>{p} {p === 1 ? 'point' : 'points'}</option>
                          {/each}
                        </select>
                        <label class="inline"><input type="checkbox" checked={!!item.a.secret} onchange={(e) => { item.a.secret = e.currentTarget.checked; edited(); }} />Secret: its name hidden until it’s earned</label>
                      </div>
                    </div>
                  </div>
                  <h2>Earned when</h2>
                  {@render group(item.b, true)}
                  <div class="unless">
                    {#if item.b.unless}
                      <span class="unlesshead">It starts over each time this happens</span>
                      {@render leaf(item.b.unless, null, -1)}
                      <span class="hint">Everything it’s counted goes back to nothing, a count over all your games too. Take this out (×) and it counts on regardless.</span>
                    {:else if unlessOpen === item.k}
                      <select class="select add" aria-label="Start it over when" value="" onchange={(e) => { setUnless(e.currentTarget.value); e.currentTarget.value = ''; unlessOpen = null; }}>
                        <option value="">Start it over when…</option>
                        {#each unlessKinds as o (o.v)}<option value={o.v} disabled={!!o.off}>{o.label}</option>{/each}
                      </select>
                    {:else}
                      <button type="button" class="linkish" onclick={() => (unlessOpen = item.k)}>Optional: start it over when something happens, a ball going down an outlane, say…</button>
                    {/if}
                  </div>
                  <p class="note">{canSay} Over all your games: counted across every game played, kept from one play to the next.</p>
                  <details>
                    <summary>As the pack file has it</summary>
                    <pre class="json">{json}</pre>
                  </details>
                  {#if removing}
                    <div class="ask">
                      <span>Take “{(item.a.name || '').trim() || 'this achievement'}” out of the pack?{item.fresh ? '' : ' What’s been earned of it stays earned.'}</span>
                      <button type="button" class="btn small" onclick={removeAchievement}>Take it out</button>
                      <button type="button" class="btn small ghost" onclick={() => (removing = false)}>Keep it</button>
                    </div>
                  {:else}
                    <button type="button" class="btn small ghost remove" onclick={() => (removing = true)}><Icon name="trash" size={14} />Take it out of the pack</button>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
          <button type="button" class="btn small newach" onclick={addAchievement}><Icon name="plus" size={14} />New achievement</button>
        </div>
        <p class="note">
          Earned in hardcore while the table plays as its author made it; in softcore otherwise — kept for you, never counted for masteries.{#if opened.practice.length}
            This table plays softcore now: {opened.practice.join('; ')}.{/if}
        </p>
      </div>
    </div>
  {/if}
</div>

<style>
  .editor { display: flex; flex-direction: column; gap: 10px; }
  .problem { display: flex; gap: 8px; align-items: center; margin: 0; color: var(--bad); }
  .quiet { margin: 0; color: var(--faint); font-size: 13px; line-height: 1.5; }
  .topline { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; }
  .title-row { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; flex: 1 1 auto; min-width: 0; }
  .tools { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .packwrap { position: relative; width: 340px; max-width: 100%; margin-left: auto; }
  .grid { display: grid; grid-template-columns: 250px minmax(0, 1fr) 440px; gap: 14px; align-items: start; }
  .panel { background: var(--card); border: 1px solid var(--card-line); border-radius: 14px; min-width: 0; }
  .btn.go { background: var(--accent-bg); border-color: var(--accent); color: #fff; }
  /* the middle: Save, Try it and Finalize above the playfield */
  .middle { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .state { margin-left: 4px; color: var(--faint); font-size: 13px; }
  .state.unsaved { color: var(--warn); font-weight: 600; }
  .wrong { display: flex; flex-direction: column; gap: 2px; padding: 8px 12px; border-radius: 10px; background: #2a2210; border: 1px solid #5a4a1c; color: var(--fg-soft); font-size: 12.5px; line-height: 1.45; }
  .wrong b { color: var(--warn); }
  .said { margin: 0; padding: 8px 12px; border-radius: 10px; background: #13202c; border: 1px solid #1e3a50; color: var(--fg-soft); font-size: 13px; line-height: 1.5; display: flex; gap: 10px; align-items: center; }
  .field { padding: 10px; display: flex; flex-direction: column; gap: 8px; }
  .pickhint { color: var(--fg-soft); font-size: 13px; }
  .pickhint b { color: var(--accent); }
  .card { margin: 0; padding: 10px 12px; border-radius: 10px; background: var(--panel); border: 1px solid var(--panel-line); color: var(--fg-soft); font-size: 13px; line-height: 1.5; display: flex; flex-wrap: wrap; gap: 8px; align-items: center; justify-content: space-between; }
  .card b { font-family: Consolas, ui-monospace, monospace; color: var(--fg); }
  /* the right: the pack's strip, then its achievements */
  .right { display: flex; flex-direction: column; gap: 10px; min-width: 0; max-height: calc(100vh - 225px); overflow: auto; padding-right: 2px; }
  .right > * { flex-shrink: 0; }
  .packhead { position: relative; width: 100%; display: grid; grid-template-columns: 40px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 7px 10px; border: 1px solid var(--card-line); background: var(--card); color: var(--fg); font: inherit; text-align: left; cursor: pointer; border-radius: 12px; }
  .packhead:hover { background: var(--hover); }
  .packhead[aria-expanded='true'] { border-color: var(--accent); }
  .corner { position: absolute; top: -7px; right: -7px; width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; background: var(--warn); color: #1a1405; border: 2px solid var(--bg); }
  /* the pack's details, open at the top of the right column, the achievements below them */
  .details { padding: 14px; border-radius: 12px; background: #151a24; border: 1px solid var(--accent); display: flex; flex-direction: column; gap: 10px; }
  .x2 { margin-left: auto; border: 0; background: transparent; color: var(--faint); font-size: 16px; cursor: pointer; }
  .packhead img, .packhead .no-pic { width: 40px; height: 40px; border-radius: 9px; object-fit: cover; }
  .packhead .t { min-width: 0; }
  .packhead b { display: flex; align-items: center; gap: 8px; font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .packhead .t > span { display: block; color: var(--faint); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pencil { color: var(--faint); padding: 4px; }
  .pack-pic { display: grid; grid-template-columns: 64px minmax(0, 1fr); gap: 12px; align-items: start; }
  .details .for { margin: 0; color: var(--fg-soft); font-size: 13px; }
  .packbtns { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .soundrow { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; font-weight: 400; }
  .soundname { color: var(--fg-soft); font-size: 13px; }
  label.f, div.f { display: flex; flex-direction: column; gap: 4px; color: var(--faint); font-size: 12px; font-weight: 600; }
  input[type='text'], input[type='number'] { height: 32px; box-sizing: border-box; padding: 0 9px; border-radius: 8px; border: 1px solid var(--control-line); background: var(--bg); color: var(--fg); font: inherit; font-size: 14px; min-width: 0; }
  input.needs-it { border-color: var(--warn); }
  .no-pic { display: grid; place-items: center; background: var(--control); color: var(--faint); }
  .no-pic.small { width: 30px; height: 30px; border-radius: 8px; }
  /* a picture's place: dotted with a plus while it has none */
  .slot { position: relative; width: 40px; height: 40px; padding: 0; border-radius: 10px; border: 1px solid transparent; background: transparent; cursor: pointer; display: grid; place-items: center; color: var(--faint); overflow: visible; }
  .slot.big { width: 64px; height: 64px; border-radius: 14px; }
  .slot img { width: 100%; height: 100%; border-radius: inherit; object-fit: cover; }
  .slot.empty { border: 2px dashed var(--control-line); background: color-mix(in srgb, var(--control) 50%, transparent); }
  .slot.empty:hover { border-color: var(--accent); color: var(--accent); }
  .plus { position: absolute; right: -5px; bottom: -5px; width: 18px; height: 18px; border-radius: 50%; display: grid; place-items: center; background: var(--accent-bg); color: #fff; border: 2px solid var(--card); }
  .slot.big .plus { width: 22px; height: 22px; }
  .inline { display: flex; flex-wrap: wrap; gap: 8px 10px; align-items: center; color: var(--fg-soft); font-size: 13px; }
  .inline input[type='checkbox'], .tick input { accent-color: var(--accent); }
  .tick { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 8px; align-items: start; color: var(--fg-soft); font-size: 13px; line-height: 1.45; font-weight: 400; }
  .tick input { margin: 2px 0 0; }
  .warn { margin: 0; color: var(--warn); font-size: 12px; line-height: 1.5; }
  .achs { display: flex; flex-direction: column; gap: 4px; }
  .ach { border-radius: 12px; border: 1px solid transparent; }
  .ach.on { border-color: var(--accent); background: var(--card); }
  .achhead { width: 100%; display: grid; grid-template-columns: 30px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 7px 8px; border: 0; border-radius: 11px; background: transparent; color: var(--fg); text-align: left; font: inherit; cursor: pointer; }
  .achhead:hover { background: var(--hover); }
  .ach.on .achhead { background: color-mix(in srgb, var(--accent) 16%, transparent); }
  .achhead img { width: 30px; height: 30px; border-radius: 8px; object-fit: cover; }
  .achhead .t { min-width: 0; }
  .achhead .t b { display: block; font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .achhead .t > span { display: block; color: var(--faint); font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .achhead .pts { font: 600 12px Consolas, ui-monospace, monospace; color: var(--good); }
  .newach { align-self: flex-start; margin-top: 4px; }
  .tag { margin-left: 6px; padding: 1px 6px; border-radius: 999px; background: #3a2c0d; color: var(--warn); font-size: 10px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; vertical-align: 1px; }
  .edit { padding: 6px 12px 14px; display: flex; flex-direction: column; gap: 12px; }
  .top { display: grid; grid-template-columns: 64px minmax(0, 1fr); gap: 12px; align-items: start; }
  .fields { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  input.name { font-size: 16px; font-weight: 600; }
  h2 { margin: 2px 0 0; font: 700 17px var(--display); letter-spacing: 0.04em; text-transform: uppercase; color: var(--fg-soft); }
  .join, .add { align-self: flex-start; }
  .rows { display: flex; flex-direction: column; gap: 8px; }
  .row { position: relative; display: flex; flex-wrap: wrap; gap: 8px; align-items: center; padding: 10px 36px 10px 12px; border-radius: 10px; background: var(--panel); border: 1px solid var(--panel-line); color: var(--fg-soft); font-size: 13px; }
  .row.active { border-color: var(--accent); }
  .row.dropping { border-color: var(--good); background: color-mix(in srgb, var(--good) 10%, var(--panel)); }
  .partline { display: flex; flex-wrap: wrap; gap: 6px 8px; align-items: center; width: 100%; }
  .times { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; width: 100%; }
  .row input[type='number'] { width: 76px; }
  .row input.big { width: 150px; }
  .row pre { margin: 0; font: 12px Consolas, ui-monospace, monospace; color: var(--accent-soft); white-space: pre-wrap; word-break: break-word; }
  .x { position: absolute; top: 6px; right: 6px; width: 24px; height: 24px; display: grid; place-items: center; border: 0; border-radius: 6px; background: transparent; color: var(--faint); cursor: pointer; font-size: 16px; line-height: 1; }
  .x:hover { background: var(--hover); color: var(--fg); }
  .chip-part { display: inline-flex; align-items: center; gap: 4px; padding: 3px 4px 3px 9px; border-radius: 999px; background: var(--accent-bg); color: #fff; font: 600 12px Consolas, ui-monospace, monospace; }
  .chip-part button { border: 0; background: transparent; color: #9ccbe8; cursor: pointer; font-size: 14px; line-height: 1; padding: 0 4px; }
  .empty { color: var(--warn); }
  .hint { width: 100%; color: var(--faint); font-size: 12px; }
  .findpart { position: relative; display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 8px; border-radius: 8px; border: 1px solid var(--accent); background: var(--bg); color: var(--faint); }
  .findpart input { width: 150px; border: 0; outline: 0; background: transparent; color: var(--fg); font: 13px Consolas, ui-monospace, monospace; }
  .found { position: absolute; top: 32px; left: -1px; z-index: 20; min-width: 220px; max-height: 240px; overflow: auto; padding: 4px; border-radius: 10px; background: #151a24; border: 1px solid var(--control-line); box-shadow: 0 14px 30px rgba(0, 0, 0, 0.5); display: flex; flex-direction: column; }
  .found button { display: flex; justify-content: space-between; gap: 10px; padding: 6px 8px; border: 0; border-radius: 6px; background: transparent; color: var(--fg); font: 13px Consolas, ui-monospace, monospace; text-align: left; cursor: pointer; }
  .found button:hover { background: var(--selected); }
  .found .k { color: var(--faint); font: 11px var(--sans, inherit); }
  details summary { cursor: pointer; color: var(--faint); font-size: 13px; font-weight: 600; }
  details[open] summary { margin-bottom: 8px; }
  pre.json { margin: 0; padding: 12px; border-radius: 10px; background: var(--bg); border: 1px solid var(--line-soft); color: var(--accent-soft); font: 12px/1.5 Consolas, ui-monospace, monospace; white-space: pre-wrap; word-break: break-word; }
  .note { margin: 0; color: var(--faint); font-size: 12px; line-height: 1.5; }
  .ask { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); color: var(--fg-soft); font-size: 13px; font-weight: 400; }
  .remove { align-self: flex-start; margin-left: -8px; color: var(--faint); }
  .group { display: flex; flex-direction: column; gap: 8px; }
  .group.nested { padding: 8px 10px; border-radius: 10px; border-left: 3px solid var(--accent); background: color-mix(in srgb, var(--accent) 6%, transparent); }
  .grouphead { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; color: var(--fg-soft); font-size: 13px; }
  .grouphead input[type='number'] { width: 72px; }
  .grouprow { position: relative; }
  .grouprow > .x { position: absolute; top: 6px; right: 6px; }
  .unless { display: flex; flex-direction: column; gap: 6px; }
  .linkish { align-self: flex-start; text-align: left; padding: 0; border: 0; background: none; color: var(--faint); font: inherit; font-size: 13px; text-decoration: underline; text-underline-offset: 2px; cursor: pointer; }
  .linkish:hover { color: var(--fg-soft); }
  .unlesshead { color: var(--warn); font-size: 12px; font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase; }
  .coilname { color: var(--accent-soft); font-size: 13px; }
  @media (max-width: 1300px) {
    .grid { grid-template-columns: 220px minmax(0, 1fr); }
    .right { grid-column: 1 / -1; max-height: none; }
  }
  @media (prefers-reduced-motion: no-preference) {
    .ach, .row { transition: background 0.12s, border-color 0.12s; }
  }
</style>
