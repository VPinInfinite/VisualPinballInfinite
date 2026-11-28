// The screens' state: the library as the app's engine side tells it, which screen shows, the
// library's filters and the user's preferences (kept by the engine side in Library\library.json).
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { drawWheel } from './wheel.js';

export const app = $state({
  // a video's sound heard (the speaker on a table's video): off until turned on, for this run
  videoSound: false,
  status: { phase: 'starting', done: 0, of: 0 },
  tables: [],
  /** the tables an achievement pack kept is made for, by ID */
  packed: [],
  loaded: false,
  /** library | table | browser | tools | editor | hub | settings */
  page: 'library',
  /** the settings page shown: screens, sound, controls and the rest */
  settingsPage: 'screens',
  /** what VPX's log said of the last game that the player can do something about: { id, notes }, shown on its table's page until dismissed */
  lastNotes: null,
  /** VPX 10.8.0 plays the tables, keeping its own settings apart from the newer VPX's (Settings › Library) */
  vpxOlder: false,
  /** the window comparing the newer VPX and 10.8.0 shows: { choosing: a line to switch to, or null, current, onchosen } */
  vpxChoice: null,
  /** each line of VPX found for the library (`vpx_status`'s installed) */
  vpxInstalled: [],
  /** the line of the VPX build the app is made for, which Get VPX brings (`vpx_status`'s madeForLine) */
  vpxMadeForLine: '10.9.0',
  /** Future Pinball as the app finds it (`fp_state`): its folder, where it's found, tagging the
   *  settings pages VPX's, Future Pinball's or both's */
  fp: null,
  /** the Tools page shown: roms, pinup, media, colour, altsound */
  toolsPage: 'roms',
  /** the player hub's page shown: scores, achievements, stats */
  hubPage: 'scores',
  /** the table the table editor's inspector shows, or none (its stages show) */
  inspectId: null,
  /** what the inspector shows of it: its parts, or its achievements (the achievements editor) */
  inspectMode: 'parts',
  /** the achievements editor's packs being made, by table ID, kept while the app runs */
  achDrafts: {},
  /** a kind of file to find on VPS as a table's page opens (a manager's Find on VPS) */
  findOnOpen: null,
  /** the tab a settings page opens on, where it has tabs */
  settingsTab: null,
  /** the table whose page shows */
  tableId: null,
  /** the tables its previous and next go through: the library's list as it was when it was opened */
  order: [],
  /** the library's group shown: all, fav, played, added, attention, a smart playlist, or g:<group> */
  pick: 'all',
  query: '',
  has: [],
  notices: [],
  prefs: { picture: 'playfield', tile: 210, view: 'tiles', sort: 'name', groupBy: 'maker' },
  /** the app's test mode (vpi-app --shot): the user's preferences left as they are */
  testing: false,
  /** the tables being tested ("Test it"), by ID */
  testingIds: [],
  /** the tables our player is making pictures of */
  makingIds: [],
  /** what runs in the background, as the engine side tells it: [{ id, label, done, of }] */
  tasks: [],
  /** the user's own playlists: [{ id, name, tables: [table keys] }], in their order */
  playlists: [],
  /** the notices at the top right that fade by themselves: what was added */
  toasts: [],
  /** tables just added, for the library to show once it has them: { ids, until } */
  reveal: null,
  /** the table the library has just shown, lit for a moment */
  glow: null,
  /** the table whose page was left for the library, for the library to show where it is */
  backTo: null,
  /** Tools › PinUP packs to ask at once about fitting every pack to the screens (after they changed) */
  pupFitAsk: false,
  /** a table renamed, its page to follow it to its new ID once the library has it: { from, to } */
  follow: null,
  /** PinUP Popper's media folder is there */
  popper: false,
  /** the front ends whose lists the ratings given here can go in too, by name */
  ratingFronts: [],
  /** the first rating given, the user asked whether ratings go in those lists too: { name } */
  askRatings: null,
  /** Report a problem, open: { table, open, list } — table null when not about one table; open, a
   *  sent report's key to show first; list, the reports sent shown first */
  report: null,
  /** the reports sent from here, the newest first, each with the team's replies and how many are
   *  new (`reports_list`) */
  reports: [],
  /** cabinet mode shown (`CabinetMode`) */
  cabinet: false,
  /** the test mode's pause menu, shown over the window as if a game were paused: its table, by part of its name */
  pauseTest: null,
  /** a table being removed from the library (its Remove window open), from its page or a right-click on its tile */
  removing: null,
  /** first setup asked for again (Settings › General) */
  setupAgain: false,
  /** first setup showing: it stays until it's done, though the library is read once its folders
   *  are kept (its last three steps had been skipped) */
  setupOpen: false,
  /** the app's window full screen (F11) */
  fullscreen: false,
});

/** The user's playlists kept as they are now. */
export async function savePlaylists() {
  if (app.testing) return;
  try {
    await invoke('save_playlists', { playlists: $state.snapshot(app.playlists) });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** A new playlist of the user's, with `t` in it if given; its ID. */
export function newPlaylist(name, t = null) {
  const id = `p${Date.now().toString(36)}`;
  app.playlists.push({ id, name: name.trim() || 'New playlist', tables: t ? [t.key] : [] });
  savePlaylists();
  return id;
}

/** A table put in, or taken off, one of the user's playlists: any of its builds taken off, the one
 *  shown put in (`buildsOf`). */
export function togglePlaylist(pl, t) {
  const keys = buildsOf(t).map((b) => b.key);
  pl.tables = keys.some((k) => pl.tables.includes(k)) ? pl.tables.filter((k) => !keys.includes(k)) : [...pl.tables, t.key];
  savePlaylists();
}

/** Whether one of the user's playlists holds a table: any of its builds. */
export const inPlaylist = (pl, t) => buildsOf(t).some((b) => pl.tables.includes(b.key));

/** The library shows a table with more than one table file once, its table files as builds on its
 *  page (Settings › Library); else each table file on its own. */
export const combined = () => app.prefs.tableBuilds === 'combined';

// each table's table files by its group, made again when the library's tables change
let groupsOf = null;
let groupMap = new Map();
function groups() {
  const tables = app.tables;
  if (groupsOf !== tables) {
    groupsOf = tables;
    groupMap = new Map();
    for (const t of tables) {
      const g = groupMap.get(t.group);
      if (g) g.push(t);
      else groupMap.set(t.group, [t]);
    }
  }
  return groupMap;
}

/** A table's builds: the table files of its table, where the library shows each table once;
 *  else the table file alone. */
export function buildsOf(t) {
  if (!t) return [];
  if (!combined()) return [t];
  return groups().get(t.group) || [t];
}

// the library's tables by name in lower case, made again when its tables change
let namesOf = null;
let nameMap = new Map();
/** The other tables named as `t` is, its own builds left out where a table shows once: a tile tells
 *  two of one name apart by their version and first author. */
export function sameName(t) {
  const tables = app.tables;
  if (namesOf !== tables) {
    namesOf = tables;
    nameMap = new Map();
    for (const x of tables) {
      const k = x.name.toLowerCase();
      const n = nameMap.get(k);
      if (n) n.push(x);
      else nameMap.set(k, [x]);
    }
  }
  return (nameMap.get(t.name.toLowerCase()) || []).filter((x) => x.id !== t.id && !(combined() && x.group === t.group));
}

/** Two versions compared as people read them: 4.2 after 4.1a, 1.10 after 1.9. */
export function versionOrder(a, b) {
  const parts = (v) => String(v || '').toLowerCase().match(/\d+|[a-z]+/g) || [];
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if (x[i] === undefined) return -1;
    if (y[i] === undefined) return 1;
    const [nx, ny] = [/^\d+$/.test(x[i]), /^\d+$/.test(y[i])];
    if (nx && ny) {
      const d = Number(x[i]) - Number(y[i]);
      if (d) return d;
    } else if (x[i] !== y[i]) return nx ? 1 : ny ? -1 : x[i].localeCompare(y[i]);
  }
  return 0;
}

/** The build of a table its tile shows and plays: its main build, else the one played last, else
 *  the newest version, else the one added last. */
export function shownBuild(builds) {
  return (
    builds.find((b) => b.mainBuild) ||
    [...builds].sort((a, b) => (b.played?.last || 0) - (a.played?.last || 0) || versionOrder(b.ownVersion || b.version, a.ownVersion || a.version) || b.added - a.added)[0]
  );
}

/** Tables as the library shows them: each table once, as the build it shows, where it shows
 *  each table once; else every table file. */
export function oneEach(tables) {
  if (!combined()) return tables;
  const by = new Map();
  for (const t of tables) {
    const g = by.get(t.group);
    if (g) g.push(t);
    else by.set(t.group, [t]);
  }
  return [...by.values()].map((g) => (g.length > 1 ? shownBuild(g) : g[0]));
}

/** A table's heart, stars and dates as the library shows them: its table's, any of its builds'. */
export const isFavourite = (t) => buildsOf(t).some((b) => b.favourite);
export const ratingOf = (t) => t.rating || Math.max(0, ...buildsOf(t).map((b) => b.rating || 0)) || null;
export const lastPlayed = (t) => Math.max(0, ...buildsOf(t).map((b) => b.played?.last || 0));
export const addedOf = (t) => Math.max(...buildsOf(t).map((b) => b.added || 0));

/** A build's name on its tab: what its table file's name says past the table's own name and its
 *  maker and year, else its version and first author, else its file's name. */
export function buildLabel(b) {
  let rest = b.file;
  const name = (b.name || '').toLowerCase();
  if (name && rest.toLowerCase().startsWith(name)) rest = rest.slice(name.length);
  rest = rest.replace(/^\s*\([^)]*\)/, '').replace(/^[\s_\-–—.,]+/, '').trim();
  if (rest) return rest;
  return [b.ownVersion || b.version, b.authors?.[0]].filter(Boolean).join(' · ') || b.file;
}

/** The playlists made from VPS's tags: a table release's features (SSF, VR,
 *  FastFlips …) and a game's themes, each with the library's tables that have it, the most first. */
export function tagPlaylists(tables) {
  const by = new Map();
  for (const t of tables) {
    for (const [kind, tag] of [...t.features.map((f) => ['feature', f]), ...t.theme.map((x) => ['theme', x])]) {
      const id = `tag:${kind}:${tag}`;
      if (!by.has(id)) by.set(id, { id, label: tag, kind, list: [] });
      by.get(id).list.push(t);
    }
  }
  // what plays them, VPX or Future Pinball, where the library holds both
  const formats = new Map();
  for (const t of tables) {
    const f = t.format === 'fp' ? 'fp' : 'vpx';
    if (!formats.has(f)) formats.set(f, { id: `tag:format:${f}`, label: f === 'fp' ? 'Future Pinball' : 'VPX', kind: 'format', list: [] });
    formats.get(f).list.push(t);
  }
  const both = formats.size > 1 ? [...formats.values()] : [];
  return [...both, ...[...by.values()].filter((g) => g.list.length >= 2).sort((a, b) => b.list.length - a.list.length || a.label.localeCompare(b.label))];
}

/** PinUP packs named so the player won't find them renamed to what their tables' scripts ask for. */
export async function renamePacks(ids) {
  try {
    const n = await invoke('rename_packs', { ids });
    notice(n === 1 ? 'The PinUP pack is renamed: its table finds it now.' : `${n} PinUP packs are renamed: their tables find them now.`, { kind: 'done' });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** The VPS browser: every machine VPS lists, its filters and search (the browser's own, apart from
 * the library's), and the machine shown beside the cards. */
export const vps = $state({
  machines: [],
  refreshed: null,
  loaded: false,
  /** the machine shown in the panel, by its VPS ID */
  sel: null,
  query: '',
  maker: 'all',
  era: 'all',
  type: 'all',
  format: 'VPX',
  sort: 'newest',
  notMine: false,
  has: [],
  /** the top bar's list of the downloads waiting, and of the tasks, open: the panel's page out of
   *  sight meanwhile, as the web view is drawn over the app's own page and would hide them */
  inboxOpen: false,
  tasksOpen: false,
  /** what the browser shows: every machine (`all`), what's to get for the user's tables (`yours`),
   *  tables picked for the user (`picks`) */
  view: 'all',
  /** the page the in-app browser shows, as the web view tells it, and the screen showing it:
   *  { url, loading, from: 'browser' | 'table' } */
  web: null,
  /** the page a Get asked for, numbered, with the VPS machine it's for: { url, game, n } */
  ask: null,
  /** the files downloaded in the panel, waiting to be imported: [{ name, path, size, at, downloading }] */
  inbox: [],
});

let webAsks = 0;
/** A file's page shown in the in-app browser: `game`, the VPS machine it's for (what's downloaded
 *  there then finds its table); `from`, the screen showing it (`browser`, `table`). */
export function openWeb(url, game, from) {
  vps.ask = { url, game: game ?? null, n: ++webAsks };
  vps.web = { url, loading: true, from };
}
export function closeWeb() {
  vps.web = null;
}

/** The files downloaded in the panel, waiting to be imported, all put in the drop window, which
 *  sorts them by table: each one got for a machine the library has a table of dropped on that
 *  table, as if onto its page — those for no table first, so a table they bring is there for the
 *  rest to join. */
export async function importWaiting() {
  const ready = vps.inbox.filter((w) => !w.downloading);
  const tableOf = (game) => (game && app.tables.find((t) => t.onVps?.sure && t.onVps.game === game)?.id) || null;
  const groups = new Map();
  for (const w of ready) {
    const id = tableOf(w.game);
    groups.set(id, [...(groups.get(id) || []), w.path]);
  }
  const order = [...groups].sort(([a], [b]) => (a === null ? -1 : b === null ? 1 : 0));
  for (const [id, paths] of order) await dropFiles(paths, id);
}

/** Every machine VPS lists, read again: which are in the library changes with it. */
export async function loadVps() {
  try {
    const b = await invoke('vps_browser');
    vps.machines = b.machines;
    vps.refreshed = b.refreshed;
    vps.loaded = app.loaded;
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** The drop window: a drop's downloads as the engine side sorted them out, and the user's choices
 * for each — its rows ticked, where unsorted rows go — while it shows. */
export const drop = $state({
  open: false,
  busy: false,
  error: '',
  downloads: [],
  choices: [],
  /** what's done with a file already there when a row has no choice of its own (each row with one
   * there asks) */
  already: 'skip',
  progress: null,
  results: null,
  /** files being dragged over the window, and the table's ID under them */
  dragging: false,
  over: null,
});

function setDownloads(downloads, reset = -1) {
  // the user's choices kept for the downloads already shown, by where they came from and the table
  // they're for (one tied to a table since, a table having joined the drop, starts afresh)
  const at = (d) => `${d.source}|${d.forTableId || ''}`;
  const kept = new Map(drop.downloads.map((d, i) => [at(d), drop.choices[i]]));
  // what's done with what's there already, as the settings say (ask each time unless the user chose)
  const first = app.prefs.alreadyDefault || 'ask';
  const fit = (r) => (first === 'keepBoth' && isPack(r) ? 'append' : first === 'append' && !isPack(r) ? 'replace' : first);
  const asked = (d) => Object.fromEntries(d.rows.map((r, k) => [k, r]).filter(([, r]) => r.state === 'replace' || r.state === 'some').map(([k, r]) => [k, fit(r)]).filter(([, v]) => v !== 'ask'));
  // a media card's place to begin with: the app's guess, else the table's own media
  const placed = (d) => Object.fromEntries(d.rows.map((r, k) => [k, r]).filter(([, r]) => r.media).map(([k, r]) => [k, r.media.guess || 'tableMedia']));
  drop.choices = downloads.map((d, i) => (i !== reset && kept.get(at(d))) || { include: d.rows.map((r) => r.include), put: placed(d), already: asked(d), keepWindows: {}, fresh: true });
  // a file going where one ticked before it goes (two backglasses for one table): left unticked
  const taken = new Set();
  downloads.forEach((d, i) =>
    d.rows.forEach((r, k) => {
      const c = drop.choices[i];
      if (!r.to || !c.include[k]) return;
      const place = r.to.toLowerCase();
      if (taken.has(place) && c.fresh) c.include[k] = false;
      taken.add(place);
    }),
  );
  for (const c of drop.choices) delete c.fresh;
  drop.downloads = downloads;
}

/** A drop's row for a pack — a folder of files — rather than a single file. */
export const isPack = (r) => r.files > 1 || ['pupPack', 'altSound', 'dmdFolder', 'folder'].includes(r.kind);

/** Files dropped: sorted out by the engine side and shown, onto the drop window when it's open
 * (joining what's there), else for the table they were dropped on, if any. */
export async function dropFiles(paths, tableId = null) {
  // achievement packs added at once, the rest to the drop window
  try {
    const [added, rest] = await invoke('achievements_drop', { paths });
    for (const a of added) notice(a.startsWith('not added') ? `An achievement pack was ${a}` : `The achievement pack ${a} is added: see it on its table’s page.`, { kind: a.startsWith('not added') ? 'problem' : 'done' });
    paths = rest;
  } catch {
    // the drop window sorts them all
  }
  if (!paths.length) return;
  const join = drop.open && !drop.results;
  if (!join) Object.assign(drop, { downloads: [], choices: [], results: null });
  Object.assign(drop, { open: true, busy: true, error: '' });
  try {
    setDownloads(await invoke('inspect_drop', { paths, tableId, join }));
  } catch (e) {
    drop.error = String(e);
  }
  drop.busy = false;
}

/** A download sorted again for the library's table the user chose. */
export async function chooseDropTable(index, tableId) {
  drop.busy = true;
  try {
    setDownloads(await invoke('choose_drop_table', { index, tableId }), index);
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
  drop.busy = false;
}

/** A download locked with a password opened with the one typed, and sorted out again: true where it
 * opened it, else what was wrong (the password kept only where it opened it). */
export async function unlockDrop(index, password) {
  drop.busy = true;
  try {
    setDownloads(await invoke('unlock_drop', { index, password }), index);
    return true;
  } catch (e) {
    return String(e);
  } finally {
    drop.busy = false;
  }
}

/** One download taken out of the drop before Add (the file itself left where it is); the
 * window closed when none is left. */
export async function removeFromDrop(index) {
  drop.busy = true;
  try {
    setDownloads(await invoke('remove_from_drop', { index }));
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
  drop.busy = false;
  if (!drop.downloads.length) closeDrop();
}

const sizeOf = (bytes) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`);

/** The table a download's files went to, by name: the one it brought, or the one it was for — the
 * library's, or one the drop brought. */
function tableOf(res) {
  const d = drop.downloads[res.index];
  if (res.tables[0]) return res.tables[0].name;
  if (d?.table) return d.table.game || d.table.stem;
  const brought = d?.forTableId?.startsWith('new:') && drop.downloads.find((x) => x.table && `new:${x.table.stem}` === d.forTableId)?.table;
  if (brought) return brought.game || brought.stem;
  return tableById(d?.forTableId)?.name || d?.forTable || res.name;
}

/** What a drop added, a line a table: what of it went in, how many files, how big. */
function addedLines(results) {
  const per = new Map();
  for (const res of results) {
    const who = tableOf(res);
    const d = drop.downloads[res.index];
    const choice = drop.choices[res.index];
    const t = per.get(who) || { who, isNew: false, kinds: [], copied: 0, bytes: 0, replaced: 0, keptBoth: 0, tested: false };
    t.isNew ||= res.tables.some((x) => x.new);
    for (const [k, r] of (d?.rows || []).entries()) if (choice?.include[k] && r.state !== 'same' && !t.kinds.includes(r.label.toLowerCase())) t.kinds.push(r.label.toLowerCase());
    const a = res.added;
    t.copied += a.copied;
    t.bytes += a.bytes;
    t.replaced += a.replaced;
    t.keptBoth += a.keptBoth;
    t.tested ||= res.tested.some(([, look]) => look === 'good');
    per.set(who, t);
  }
  return [...per.values()].map((t) => {
    const files = t.copied === 0 ? 'nothing new' : t.copied === 1 ? '1 file' : `${t.copied} files`;
    const bits = [`${t.kinds.length ? `${t.kinds.join(', ')}: ` : ''}${files}${t.bytes ? `, ${sizeOf(t.bytes)}` : ''}`];
    if (t.replaced) bits.push(`${t.replaced} replaced, the old in the Recycle Bin`);
    if (t.keptBoth) bits.push(`${t.keptBoth} kept beside the old, numbered`);
    if (t.tested) bits.push('tested: it works');
    return { who: t.who, isNew: t.isNew, what: bits.join(' · ') };
  });
}

/** The ticked files copied into place, the window following along; when all went well, the window
 * closes and a notice at the top right says what was added, fading; either
 * way the library shows the tables added once it has them. */
export async function addDrop() {
  Object.assign(drop, { busy: true, error: '', progress: { download: 0, of: drop.downloads.length, name: '', done: 0, total: 0 } });
  const stop = await listen('drop-progress', (e) => (drop.progress = e.payload));
  let results = null;
  try {
    results = await invoke('add_drop', { choices: $state.snapshot(drop.choices), already: drop.already, test: !!app.prefs.testNewTables });
  } catch (e) {
    drop.error = String(e);
  }
  stop();
  Object.assign(drop, { busy: false, progress: null });
  if (!results) return;
  const fresh = results.flatMap((r) => r.tables).filter((t) => t.new).map((t) => t.id);
  if (fresh.length) {
    app.reveal = { ids: fresh, until: Date.now() + 60000 };
    app.page = 'library';
  }
  const wrong = (r) => r.problem || r.added?.problems.length || r.tested.some(([, look]) => look !== 'good') || r.tables.some((t) => !t.sure);
  if (results.some(wrong)) {
    drop.results = results;
    return;
  }
  const lines = addedLines(results);
  closeDrop();
  const one = lines[0];
  if (lines.length === 1) toast(one.isNew ? `Added ${one.who}` : `Added to ${one.who}`, [one.what]);
  else toast(`Added to ${lines.length} tables`, lines.map((l) => `${l.who} — ${l.what}`));
}

let toastCount = 0;

/** A notice at the top right that fades by itself after a few seconds (held while the pointer is
 * on it): a title and its lines. */
export function toast(title, lines = []) {
  app.toasts.push({ id: ++toastCount, title, lines });
}

export function dropToast(id) {
  const i = app.toasts.findIndex((t) => t.id === id);
  if (i >= 0) app.toasts.splice(i, 1);
}

export function closeDrop() {
  if (!drop.results) invoke('forget_drop');
  Object.assign(drop, { open: false, downloads: [], choices: [], results: null, error: '', busy: false });
}

/** "Test it": the table run in VPX, unseen and unheard, for about 10 seconds, what it found kept on its page. */
export async function testTable(id) {
  if (app.testingIds.includes(id)) return;
  app.testingIds.push(id);
  try {
    const tested = await invoke('test_table', { id });
    const t = tableById(id);
    if (t) t.tested = tested;
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
  app.testingIds = app.testingIds.filter((x) => x !== id);
}

/** Everything the make-media window offers, the playfield cut as VPS's pictures are. */
export const ALL_MEDIA = { kinds: ['playfield', 'video', 'backglass', 'dmd', 'wheel'] };

/** A table's media made: our player's pictures and video with no window, then the wheel drawn from
 *  its pictures, as `wanted` asks. */
/** A table's media made (`quiet`: no notice of its own, one of many being made). */
export async function makePictures(id, wanted = ALL_MEDIA, { quiet = false } = {}) {
  if (app.makingIds.includes(id)) return;
  app.makingIds.push(id);
  try {
    await invoke('make_pictures', { id, wanted });
    if (wanted.kinds.includes('wheel')) {
      // drawn from its pictures as they are now, a backglass just made among them
      await reload();
      const t = tableById(id);
      if (t) await invoke('save_wheel', { id, png: Array.from(await drawWheel(t)) });
    }
    const t = tableById(id);
    if (!quiet) notice(`The media for ${t ? t.short : 'it'} are made: they’re on its page.`, { kind: 'done' });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
  app.makingIds = app.makingIds.filter((x) => x !== id);
}

/** A Future Pinball table's media recorded from the game itself, played on the user's screens in
 *  its attract mode (our player draws VPX's tables): its playfield's picture and video, its
 *  backglass's and score display's pictures; the wheel drawn after from them, where `wheel`. */
export async function recordMedia(id, { wheel = false } = {}) {
  if (app.makingIds.includes(id)) return;
  app.makingIds.push(id);
  let off = null;
  try {
    const done = new Promise((resolve) => {
      off = listen('media-recorded', (e) => {
        if (e.payload?.id === id) resolve(e.payload);
      });
    });
    await invoke('record_media', { id });
    const r = await done;
    if (r.problem) {
      notice(`${r.name}’s media weren’t recorded: ${r.problem}`, { kind: 'problem' });
    } else {
      if (wheel) {
        await reload();
        const t = tableById(id);
        if (t) await invoke('save_wheel', { id, png: Array.from(await drawWheel(t)) });
      }
      notice(`${tableById(id)?.short ?? r.name}’s media are recorded: they’re on its page.`, { kind: 'done' });
    }
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  } finally {
    off?.then((f) => f());
  }
  app.makingIds = app.makingIds.filter((x) => x !== id);
}

/** How far a table's picture or video at `url` is turned to stand upright, clockwise: 90 or 270 for
 *  one lying on its side (a front end's made for a cabinet's screen laid on its side), else 0. */
export const turnOf = (t, url) => (url && t?.media?.find((m) => m.url === url)?.turn) || 0;

// cabinet mode's volume, 0 to 100, one for every table: as set, else none where its sound was
// turned off before there was a volume
export const cabinetVolume = (p) => (Number.isFinite(p.cabinetVolume) ? p.cabinetVolume : p.cabinetSound === false ? 0 : 80);
// the screens whose video gives cabinet mode its one sound, the first with sound playing: as set,
// else the backglass's, the topper's, then the playfield's (the table's music when none has sound)
export const SOUND_SCREENS = ['backglass', 'topper', 'playfield'];
export const cabinetSoundOrder = (p) => {
  const set = Array.isArray(p.cabinetSoundOrder) ? p.cabinetSoundOrder.filter((s) => SOUND_SCREENS.includes(s)) : [];
  return [...new Set([...set, ...SOUND_SCREENS])];
};
// a volume of 0 to 100 as a media element's (0 to 1): squared, so each step sounds about as big
export const loudnessOf = (v) => (Math.max(0, Math.min(100, v)) / 100) ** 2;
// a media element kept at a volume (0 to 1)
export function volumeAt(el, v) {
  el.volume = v;
  return { update: (n) => (el.volume = n) };
}

export function tableById(id) {
  return app.tables.find((t) => t.id === id);
}

let noticeCount = 0;

/** A notice at the window's foot: info and done ones go by themselves, problems stay until closed. */
export function notice(text, { kind = 'info', action = null } = {}) {
  const id = ++noticeCount;
  app.notices.push({ id, text, kind, action });
  // a problem and a new version stay until closed
  if (kind !== 'problem' && kind !== 'update') setTimeout(() => dismiss(id), 7000);
  return id;
}

/** A new version looked for on the releases (the one-click updater): said when found, with its
 *  Install button; `asked` (Settings › General's button) says so when there's none, or why it
 *  couldn't look. */
export async function lookForUpdate(asked = false) {
  try {
    const found = await invoke('update_check');
    if (found) {
      notice(`Visual Pinball Infinite ${found.version} is ready.`, { kind: 'update', action: { label: 'Install it', run: installUpdate } });
    } else if (asked) {
      notice('This is the newest version.', { kind: 'done' });
    }
  } catch (e) {
    if (asked) notice(`Couldn’t look for a new version: ${e}`, { kind: 'problem' });
  }
}

/** The reports sent from here, as kept: the top bar counts their new replies. */
export async function loadReports() {
  try {
    app.reports = await invoke('reports_list');
  } catch {
    app.reports = [];
  }
}

/** The team's replies to the reports sent from here looked for — only where one can still be
 *  answered — and a new one said once, with a button to read it (`quiet`: taken in alone). */
export async function checkReports({ quiet = false } = {}) {
  if (app.testing || !app.reports.some((r) => r.answerable && !r.closed)) return;
  const before = new Map(app.reports.map((r) => [r.key, r.unread]));
  try {
    app.reports = await invoke('reports_check');
  } catch {
    return;
  }
  const fresh = app.reports.filter((r) => r.unread > (before.get(r.key) ?? 0));
  if (quiet || !fresh.length || app.report) return;
  const one = fresh.length === 1 ? fresh[0] : null;
  notice(one ? `The Visual Pinball Infinite team replied to your report${one.table ? ` on ${one.table}` : ''}.` : 'The Visual Pinball Infinite team replied to your reports.', {
    kind: 'info',
    action: { label: 'Read it', run: () => (app.report = { table: null, open: one?.key ?? null, list: true }) },
  });
}

/** The team's replies not opened yet, across the reports sent. */
export const unreadReplies = () => app.reports.reduce((n, r) => n + (r.unread || 0), 0);

/** A notice to everyone the team has put up, shown once as the app starts, its link a button. */
export async function lookForNotices() {
  if (app.testing) return;
  let found = [];
  try {
    found = await invoke('notices_new');
  } catch {
    return;
  }
  for (const n of found) {
    const link = n.text.match(/https:\/\/[^\s<>"')\]]+/)?.[0];
    notice(`From the Visual Pinball Infinite team: ${n.text}`, {
      kind: 'update',
      action: link ? { label: 'Open the link', run: () => invoke('open_link', { url: link }).catch((e) => notice(String(e), { kind: 'problem' })) } : null,
    });
  }
}

/** The new version downloaded and installed; the app closes for its installer. */
export async function installUpdate() {
  notice('Downloading the new version… the app closes to install it.', { kind: 'info' });
  try {
    await invoke('update_install');
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

export function dismiss(id) {
  const i = app.notices.findIndex((n) => n.id === id);
  if (i >= 0) app.notices.splice(i, 1);
}

// the page the app opens on chosen once, as the library first reads whole
let openedOn = false;

// the library asked for again, each time it changes: only the newest reply taken, as a reply asked
// for earlier can come after it — one passed over waits for the newest, so whoever waits on a
// reload finds the library as it is
let reloadAsked = 0;
let reloading = Promise.resolve();
function reload() {
  const ask = ++reloadAsked;
  reloading = invoke('library').then((reply) => (ask === reloadAsked ? take(reply) : reloading));
  return reloading;
}

function take(reply) {
  app.status = reply.status;
  app.tables = reply.tables;
  app.packed = reply.packed || [];
  app.playlists = reply.playlists;
  app.popper = reply.popper;
  app.ratingFronts = reply.ratingFronts || [];
  app.loaded = reply.status.phase === 'ready';
  // in the same moment as the tables, so the page never shows a table that's gone
  applyFollow();
  // Settings › General's The app opens on: the last table's page, where it's still in the library
  if (app.loaded && !openedOn) {
    openedOn = true;
    const last = app.prefs.lastTable;
    if (app.prefs.opensOn === 'lastTable' && !app.testing && app.page === 'library' && last && app.tables.some((t) => t.id === last)) openTable(last);
  }
}

/** A table renamed (Overwrite with VPS's names, or its names put back): its page and the library's
 * order follow it to its new ID once the library has it, never before (switched at once, the page
 * would ask for a table not there yet). */
export function follow(from, to) {
  if (!to || from === to) return;
  app.follow = { from, to };
  applyFollow();
}

function applyFollow() {
  const f = app.follow;
  if (!f || !app.tables.some((t) => t.id === f.to)) return;
  app.order = app.order.map((x) => (x === f.from ? f.to : x));
  if (app.tableId === f.from) app.tableId = f.to;
  app.follow = null;
}

let prefsTimer = null;

/** The preferences kept, a moment after the last change. */
export function savePrefs() {
  if (app.testing) return;
  clearTimeout(prefsTimer);
  prefsTimer = setTimeout(() => {
    invoke('set_prefs', { prefs: $state.snapshot(app.prefs) }).catch((e) => notice(String(e), { kind: 'problem' }));
  }, 400);
}

export async function start() {
  await listen('library-status', (e) => {
    app.status = e.payload;
  });
  await listen('library-changed', () => reload());
  await listen('tasks', (e) => {
    app.tasks = e.payload;
  });
  app.tasks = await invoke('tasks_now');
  // a table set to play another ROM of its machine, the one its script asks for not here
  await listen('rom-chosen', (e) => {
    const { name, from, to } = e.payload;
    notice(`${name} plays ${to}, a version of its machine you have: its script asked for ${from}, which isn’t here. Its page’s ROM row changes it.`, { kind: 'done' });
  });
  // the VPS browser's web page and its downloads, waiting to be imported
  await listen('inbox', (e) => (vps.inbox = e.payload));
  await listen('web-page', (e) => {
    if (vps.web) vps.web = { ...vps.web, ...e.payload };
  });
  await listen('web-download', (e) => {
    const { name, success } = e.payload;
    if (success) toast(`Downloaded ${name}`, ['Waiting for import: Import in the VPS browser puts it in place.']);
    else notice(`${name || 'A download'} didn’t finish.`, { kind: 'problem' });
  });
  invoke('inbox_list').then((l) => (vps.inbox = l)).catch(() => {});
  // a machine's first start, unseen, before its first game in VPX
  await listen('vpx-first-start', (e) => notice(`Starting ${e.payload}’s machine for the first time, which takes a few seconds. The game follows.`, { kind: 'info' }));
  await listen('player-ended', (e) => {
    const { id, name, code, earned, problem, notes } = e.payload;
    if (problem) {
      // Windows' VBScript off: its page in Windows' Settings opened from the notice
      if (problem.startsWith("Windows' VBScript")) {
        notice(problem, { kind: 'problem', action: { label: 'Open Windows’ settings', run: () => invoke('open_optional_features').catch((e) => notice(String(e), { kind: 'problem' })) } });
        return;
      }
      // the DirectX End-User Runtime missing, which VPX 10.8.0 loads: Microsoft's page for it
      if (problem.includes("Microsoft's DirectX End-User Runtime")) {
        const url = 'https://www.microsoft.com/en-us/download/details.aspx?id=35';
        notice(problem, { kind: 'problem', action: { label: 'Open Microsoft’s page', run: () => invoke('open_link', { url }).catch((e) => notice(String(e), { kind: 'problem' })) } });
        return;
      }
      // VPX ended before it started: Report a problem offered too, the table in it
      const table = app.tables.find((t) => t.id === id) || null;
      notice(problem, code === null || code === undefined ? { kind: 'problem' } : { kind: 'problem', action: { label: 'Report it', run: () => (app.report = { table }) } });
      return;
    }
    // what VPX's log said that the player can do something about, on the table's page
    app.lastNotes = notes?.length ? { id, notes } : null;
    // the achievements the game earned: as it played, else counted once VPX had quit
    if (earned?.length) notice(`${earned.length === 1 ? 'Achievement earned' : `${earned.length} achievements earned`}: ${earned.join(', ')}`, { kind: 'done' });
    if (code === 0 || code === null) return;
    // a problem during play: Report a problem offered at once, the table and its log in it
    const table = app.tables.find((t) => t.id === id) || null;
    notice(`VPX stopped with a problem while playing ${name}. A report of it, with VPX's log, helps us look into it.`, {
      kind: 'problem',
      action: { label: 'Report it', run: () => (app.report = { table }) },
    });
  });
  const kept = await invoke('prefs');
  if (kept && typeof kept === 'object') Object.assign(app.prefs, kept);
  // the window opens full screen when it was left so (the engine side made it so)
  app.fullscreen = !!app.prefs.fullscreen && !(await invoke('test_mode'));
  await reload();
}

/** The app's window full screen, or not (F11, the top bar's button, Settings › General), and kept
 *  so for the next time it opens. */
/** Full screen on or off, from what the window really is (cabinet mode turns it full screen too). */
export async function toggleFullscreen() {
  const now = await invoke('is_fullscreen').catch(() => app.fullscreen);
  await setFullscreen(!now);
}

export async function setFullscreen(on) {
  try {
    await invoke('set_fullscreen', { on });
    app.fullscreen = on;
    app.prefs.fullscreen = on;
    savePrefs();
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

export function openTable(id, order) {
  app.tableId = id;
  if (order) app.order = order;
  app.page = 'table';
  window.scrollTo(0, 0);
  // remembered for The app opens on's last table's page
  if (app.prefs.lastTable !== id) {
    app.prefs.lastTable = id;
    savePrefs();
  }
}

/** The library; from a table's page, shown where that table is, not at its top. */
export function goLibrary() {
  if (app.page === 'table') app.backTo = app.tableId;
  app.page = 'library';
}

/** The table editor's parts hidden by the user, for each table by its ID, by their names: not drawn,
 *  not chosen, until shown again. */
export const hiddenParts = (id) => app.prefs.hiddenParts?.[id] ?? [];
export function hidePart(id, name, hide) {
  const now = hiddenParts(id).filter((n) => n !== name);
  if (hide) now.push(name);
  app.prefs.hiddenParts = { ...(app.prefs.hiddenParts || {}), [id]: now };
  savePrefs();
}

/** A table opened in the table editor's inspector: its parts, or its achievements. */
export function inspect(id, mode = 'parts') {
  app.inspectId = id;
  app.inspectMode = mode;
  app.page = 'editor';
  window.scrollTo(0, 0);
}

// where the user has been in the app, for the mouse's back and forward buttons: each screen
// by what names it — a library group, a table, a settings page, the VPS browser
const trail = { past: [], ahead: [], last: null, going: false };

function where() {
  const at = { page: app.page };
  if (app.page === 'library') at.pick = app.pick;
  if (app.page === 'table') at.tableId = app.tableId;
  if (app.page === 'settings') at.settingsPage = app.settingsPage;
  if (app.page === 'tools') at.toolsPage = app.toolsPage;
  if (app.page === 'hub') at.hubPage = app.hubPage;
  if (app.page === 'editor') {
    at.inspectId = app.inspectId;
    at.inspectMode = app.inspectMode;
  }
  return at;
}

/** The screen shown now remembered, the one before it kept to go back to (an effect calls it). */
export function remember() {
  const now = where();
  const last = trail.last;
  trail.last = now;
  if (!last || JSON.stringify(last) === JSON.stringify(now)) return;
  if (trail.going) {
    trail.going = false;
    return;
  }
  trail.past.push(last);
  if (trail.past.length > 100) trail.past.shift();
  trail.ahead = [];
}

function goTo(at) {
  trail.going = true;
  if (at.pick !== undefined) app.pick = at.pick;
  if (at.tableId !== undefined && app.tables.some((t) => t.id === at.tableId)) app.tableId = at.tableId;
  if (at.settingsPage !== undefined) app.settingsPage = at.settingsPage;
  if (at.toolsPage !== undefined) app.toolsPage = at.toolsPage;
  if (at.hubPage !== undefined) app.hubPage = at.hubPage;
  if (at.inspectId !== undefined) app.inspectId = app.tables.some((t) => t.id === at.inspectId) ? at.inspectId : null;
  if (at.inspectMode !== undefined) app.inspectMode = at.inspectMode;
  const page = at.tableId !== undefined && !app.tables.some((t) => t.id === at.tableId) ? 'library' : at.page;
  if (page === 'library' && app.page === 'table') app.backTo = app.tableId;
  app.page = page;
  window.scrollTo(0, 0);
}

/** Back to the screen before (the mouse's back button). */
export function goBack() {
  const to = trail.past.pop();
  if (!to) return;
  trail.ahead.push(where());
  goTo(to);
}

/** Forward again, after going back (the mouse's forward button). */
export function goForward() {
  const to = trail.ahead.pop();
  if (!to) return;
  trail.past.push(where());
  goTo(to);
}

/** Achievement packs being made with changes not saved: a page, a table or a pack left, or the app
 *  closed, asks first (`UnsavedAsk`): `ask` the going on, `closing` the app's close. */
export const guard = $state({ ask: null, closing: false });
/** the drafts with changes not saved, by their table */
export const unsavedDrafts = () => Object.entries(app.achDrafts).filter(([, d]) => d?.edits > 0);
/** what's done once the editor's changes are saved or let go, else at once */
export function leaving(go, { closing = false } = {}) {
  if (!unsavedDrafts().length) return go();
  guard.closing = closing;
  guard.ask = { go };
}
/** A pack being made saved as a draft, no version made: the editor's state whole, its chosen
 *  pictures and sound by their files (copied beside the draft). When it was saved. */
export async function saveDraft(d) {
  const draft = $state.snapshot(d);
  draft.files = Object.fromEntries(Object.entries(draft.pictures || {}).map(([k, v]) => [k, { path: v.path }]));
  for (const k of ['pictures', 'picking', 'part', 'said', 'tried', 'triedNames', 'edits']) delete draft[k];
  const at = await invoke('ach_edit_draft_save', { draft });
  d.edits = 0;
  d.savedAt = at;
  d.hasDraft = true;
  return at;
}
export async function leaveSaving() {
  const go = guard.ask?.go;
  try {
    for (const [, d] of unsavedDrafts()) await saveDraft(d);
  } catch (e) {
    notice(String(e), { kind: 'problem' });
    return;
  }
  guard.ask = null;
  go?.();
}
export function leaveDropping() {
  const go = guard.ask?.go;
  for (const [id] of unsavedDrafts()) delete app.achDrafts[id];
  guard.ask = null;
  go?.();
}
// the window's close told to the app's side while a draft holds changes
$effect.root(() => {
  $effect(() => {
    invoke('set_unsaved', { on: unsavedDrafts().length > 0 }).catch(() => {});
  });
});

/** The coloring programs (other people's), as `colour_programs` says: null until asked. */
export const colourPrograms = $state({ list: null, installing: null });
export async function loadColourPrograms() {
  colourPrograms.list = await invoke('colour_programs', { chosen: null }).catch(() => []);
}
/** A coloring program opened — installed from its makers' release the first time. */
export async function openColourProgram(p) {
  try {
    let at = p.installed;
    if (!at) {
      colourPrograms.installing = p.key;
      notice(`Installing ${p.name} from its makers’ page on GitHub…`);
      at = await invoke('colour_program_install', { key: p.key });
      notice(`${p.name} is installed. It’s a program of its own, by other people.`, { kind: 'done' });
      await loadColourPrograms();
    }
    await invoke('colour_program_open', { path: at });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  } finally {
    colourPrograms.installing = null;
  }
}

/** PLAY; `mode`, 'hardcore' (Play Hardcore: the table as its author made it, what the user's
 * settings change of how it plays left out for this game) or 'softcore' (Play Softcore: its
 * achievements kept as softcore, a guest playing, say), else as it stands. */
export async function play(t, preset = null, mode = null) {
  try {
    // cabinet mode stays behind the player; the desktop app steps aside once its window shows
    await invoke('play', { id: t.id, preset, stay: !!app.cabinet, mode });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** A table made a favourite (the build shown), or none of its builds a favourite any more. */
export async function setFavourite(t, on) {
  try {
    for (const b of on ? [t] : buildsOf(t).filter((x) => x.favourite || x.id === t.id)) {
      await invoke('set_favourite', { id: b.id, on });
      b.favourite = on;
    }
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** A table rated, one to five stars (0: cleared). The first rating, where a front end's list could
 * hold ratings too and the user hasn't said whether it should, asks (`askRatings`). */
export async function setRating(t, stars) {
  try {
    // one rating for a table: each of its builds' (a front end lists each table file)
    const problems = [];
    for (const b of buildsOf(t)) {
      problems.push(...(await invoke('set_rating', { id: b.id, stars: stars || null })));
      b.rating = stars || null;
      b.ratingFrom = stars ? 'ours' : null;
    }
    for (const p of new Set(problems)) notice(p, { kind: 'problem' });
    if (stars && app.ratingFronts.length && app.prefs.ratingsToFronts === undefined && !app.testing) app.askRatings = { name: t.short };
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** The user's word on putting ratings in the front ends' lists too, kept; yes puts every rating
 * given here in them now. */
export async function answerRatings(yes) {
  app.askRatings = null;
  app.prefs.ratingsToFronts = yes;
  savePrefs();
  if (!yes) return;
  try {
    const problems = await invoke('ratings_to_fronts');
    for (const p of problems) notice(p, { kind: 'problem' });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** A table hidden from the library (shown only in the Hidden smart playlist), or shown again. */
export async function setHidden(t, on) {
  try {
    // a table shown once is hidden whole: each of its builds
    for (const b of on ? buildsOf(t) : [t]) {
      await invoke('set_hidden', { id: b.id, on });
      b.hiddenTable = on;
    }
    if (on) notice(`${t.short} is hidden. It shows in the Hidden playlist, which Settings › Library › Playlists turns on.`, { kind: 'done' });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

/** The user's word on a guessed VPS match: confirmed, or the table not on VPS. */
export async function chooseVps(t, confirm) {
  try {
    await invoke('choose_vps', { id: t.id, confirm });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

export async function showFile(t, what) {
  try {
    await invoke('show_file', { id: t.id, what });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

export async function openLink(url) {
  try {
    await invoke('open_link', { url });
  } catch (e) {
    notice(String(e), { kind: 'problem' });
  }
}

// what the library's groups and filters read of a table

export const decade = (y) => (y ? `${Math.floor(y / 10) * 10}s` : '');

export function needsAttention(t) {
  return problems(t).length > 0;
}

/** Needs attention's groups (split by kind, the files not right among them). */
export const PROBLEM_KINDS = [
  ['match', 'VPS match to confirm'],
  ['missing', 'Files missing'],
  ['wrong', 'Files not right'],
  ['test', 'Failed its test'],
  ['read', 'Couldn’t be read'],
  ['twice', 'Here twice'],
];

/** What needs a look at a table: each problem's kind (Needs attention's groups), how bad (`warn`,
 *  or `bad`: it won't play as it should), and what it is, in words. A table VPS doesn't list (one's
 *  own, a test table) needs nothing for that: it has a group of its own. */
export function problems(t) {
  const out = [];
  const add = (kind, look, text) => out.push({ kind, look, text });
  if (t.readError) add('read', 'bad', 'Its file couldn’t be read');
  if (t.guess) add('match', 'warn', 'Its VPS match is a guess, to confirm');
  if (t.rom && !t.holds.rom) add('missing', 'bad', `Its ROM (${t.rom}) isn’t installed. ${t.onVps.game ? 'Get it from the table’s VPS page and drop it onto the app.' : 'Drop it onto the app once you have it.'}`);
  for (const h of t.holds.helpers) if (!h.path) add('missing', 'bad', `A script it loads, ${h.name}, isn’t there`);
  for (const f of t.holds.foldersMissing) add('missing', 'warn', `The folder its script names for its DMD’s pictures, ${f}, isn’t there`);
  // the check's words name the ROM themselves
  if (t.romCheck && t.romCheck[0] !== 'good') add('wrong', t.romCheck[0], t.romCheck[1]);
  if (t.altCheck && t.altCheck[0] !== 'good') add('wrong', t.altCheck[0], t.altCheck[1]);
  // its PinUP pack: one with no layout chosen shows nothing; an empty folder has nothing to play
  if (t.pupCheck && (t.pupCheck.look === 'warn' || t.pupCheck.look === 'bad')) add('wrong', t.pupCheck.look, t.pupCheck.text);
  if (t.holds.pupMisnamed) add('wrong', 'warn', `Its PinUP pack is there as ${t.holds.pupMisnamed.split('\\').pop()}, a name its script doesn’t look for`);
  if (t.tested && t.tested.report.look !== 'good') add('test', t.tested.report.look === 'bad' ? 'bad' : 'warn', `Its test: ${t.tested.report.text}`);
  const twin = t.twin && app.tables.find((x) => x.id === t.twin);
  if (twin) add('twice', 'warn', `Its table file is here twice: ${twin.file} looks like the same one saved again. Its page asks which to keep.`);
  return out;
}

/** Why a table needs attention, in words. */
export function reasons(t) {
  return problems(t).map((p) => p.text);
}

export const isVpw = (t) => t.authors.some((a) => /^vpw$|vpin ?workshop/i.test(a)) || /\bvpw\b/i.test(t.file);
export const isOriginal = (t) => t.badge === 'Original';

export const hasTests = {
  b2s: (t) => !!t.holds.backglass,
  rom: (t) => !!t.holds.rom,
  pup: (t) => !!t.holds.pupPack,
  alt: (t) => !!t.holds.altsound,
  colour: (t) => !!t.holds.colour,
  ach: (t) => app.packed.includes(t.id),
  // made for VR, as VPS tags its release: a room and cabinet in the headset, or a switch to them
  vr: (t) => t.format !== 'fp' && t.features.some((f) => ['VR', 'Hybrid', 'Mixed Reality', 'MR'].includes(f)),
};

/** VPS's tags for a table's file that a player picks a table by, as VPS writes them, each said in
 * plain words. */
export const VPS_TAGS = [
  ['VR', 'Made to play in virtual reality too'],
  ['FSS', 'Full single screen: plays on one screen, its backglass shown with it'],
  ['Hybrid', 'VR, desktop and cabinet in one file'],
  ['MOD', 'A modified version of another table'],
  ['Retheme', 'Another machine’s table with a theme of its own'],
  ['Kids', 'Suited to children'],
  ['Adult', 'Adult content'],
  ['nFozzy', 'nFozzy’s physics: flippers and rubbers closer to a real machine’s'],
  ['Fleep', 'Fleep’s sounds: a real machine’s mechanical sounds'],
  ['SSF', 'Surround sound feedback: its sounds placed around a cabinet’s speakers'],
  ['DOF', 'Fires a cabinet’s toys'],
];

/** A table's tags: the files it holds, an achievement pack made for it, then VPS's for its table
 * file. */
export function tagsOf(t) {
  const out = [];
  // what plays it: VPX, or Future Pinball
  out.push(t.format === 'fp' ? { label: 'FP', kind: 'format', title: 'A Future Pinball table, played in Future Pinball' } : { label: 'VPX', kind: 'format', title: 'A Visual Pinball X table, played in VPX' });
  if (t.holds.backglass) out.push({ label: 'B2S', kind: 'has', title: 'Its backglass file' });
  if (t.holds.rom) out.push({ label: 'ROM', kind: 'has', title: 'Its ROM' });
  if (t.holds.pupPack) out.push({ label: 'PUP', kind: 'has', title: 'Its PinUP pack' });
  if (t.holds.altsound) out.push({ label: 'AltSound', kind: 'has', title: 'Its AltSound pack' });
  if (t.holds.colour) out.push({ label: 'Color', kind: 'has', title: 'Its color DMD' });
  if (app.packed.includes(t.id)) out.push({ label: 'Achievements', kind: 'ach', title: 'An achievement pack made for it' });
  for (const [label, title] of VPS_TAGS) if (t.features.includes(label)) out.push({ label, kind: 'vps', title: `${title} (VPS)` });
  return out;
}

export function groupOf(t, by) {
  if (!t.onVps.game && !t.maker) return 'Not on VPS';
  if (by === 'decade') return decade(t.year) || 'Year unknown';
  if (by === 'kind') return t.kind || 'Type unknown';
  return t.maker || 'Maker unknown';
}
