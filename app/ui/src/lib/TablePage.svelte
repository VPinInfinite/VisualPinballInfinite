<script>
  // A table's own page (a full page): its picture large with its other media, PLAY and the
  // small tools, VPS's facts, what it holds, and every file VPS lists for it; the previous and next
  // table by the arrows, the arrow keys, or a controller's LB and RB: a video's own first picture
  // in the strip; a picture or video deleted, or one of VPS's hidden, on a right-click; the user's
  // plays — a play log and their own details under the links; and in a wide window, the high
  // scores, the play log and VPS's files up the right-hand side; each kind of picture or video one
  // stack, the one used (a blue check) on top; the play log under the links; the high scores to the right of the table, What it holds and
  // More on VPS beside each other below, their tops in line, with no gap under the pictures.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import VpsFiles from './VpsFiles.svelte';
  import TableSettings from './TableSettings.svelte';
  import HighScores from './HighScores.svelte';
  import PlayLog, { lasted } from './PlayLog.svelte';
  import MakeMedia from './MakeMedia.svelte';
  import VpsPick from './VpsPick.svelte';
  import ChangeRom from './ChangeRom.svelte';
  import BackglassPick from './BackglassPick.svelte';
  import MakeAltSound from './MakeAltSound.svelte';
  import ColourIt from './ColourIt.svelte';
  import ScriptEditor from './ScriptEditor.svelte';
  import PupLayout from './PupLayout.svelte';
  import Screenshots from './Screenshots.svelte';
  import EditDetails from './EditDetails.svelte';
  import Stars from './Stars.svelte';
  import { tick, untrack } from 'svelte';
  import MediaStack from './MediaStack.svelte';
  import ScreensPage from './Screens.svelte';
  import { app, savePrefs, tableById, goLibrary, openTable, play, setFavourite, setRating, showFile, openLink, notice, problems, chooseVps, testTable, renamePacks, togglePlaylist, newPlaylist, inspect, vpidbPacks, loadVpidbPacks } from './store.svelte.js';

  // its achievement packs, for What it holds' Achievements row: each with its kind on VPIDB, how
  // many of its achievements are earned, and a bin that deletes it for good, asked first
  let achPacks = $state([]);
  let askingPack = $state(null);
  async function loadPacks(id) {
    achPacks = (await invoke('achievements_for', { id }).catch(() => ({ packs: [] }))).packs;
  }
  $effect(() => {
    const id = app.tableId;
    if (!id) return;
    loadPacks(id);
    loadVpidbPacks();
    const off = listen('library-changed', () => loadPacks(id));
    return () => off.then((f) => f());
  });
  const PACK_KIND = { official: 'Official', community: 'Community pack', sample: 'Sample pack', mod: 'Mod' };
  const onlinePacks = $derived(Object.fromEntries(Object.values(vpidbPacks.byGame || {}).flat().map((o) => [o.id, o])));
  async function deletePack() {
    const p = askingPack;
    askingPack = null;
    try {
      await invoke('achievements_remove', { id: p.id });
      notice(`${p.name} is deleted. What you earned with it stays earned.`, { kind: 'done' });
      await loadPacks(app.tableId);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  async function addPack() {
    try {
      const name = await invoke('achievements_add');
      if (name) notice(`${name} is added.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // the front ends a rating can come from, by the key the library gives
  const FRONT_NAMES = { Popper: 'PinUP Popper', PinballX: 'PinballX', PinballY: 'PinballY' };

  const t = $derived(tableById(app.tableId));
  // the front ends a picture can be from, in words
  const FRONT_END = { Popper: 'PinUP Popper', PinballX: 'PinballX', PinballY: 'PinballY' };
  const order = $derived(app.order.filter((id) => tableById(id)));
  const at = $derived(Math.max(0, order.indexOf(app.tableId)));
  const prev = $derived(order.length > 1 ? tableById(order[(at + order.length - 1) % order.length]) : null);
  const next = $derived(order.length > 1 ? tableById(order[(at + 1) % order.length]) : null);

  let shown = $state(0);
  let failed = $state({});
  let vps = $state([]);
  let vpsRead = $state(false);
  // opened from a manager to find one kind of file on VPS: that kind's list, once VPS's are read
  $effect(() => {
    const kind = app.findOnOpen;
    if (!kind || !vpsRead) return;
    app.findOnOpen = null;
    tick().then(() => find(kind));
  });
  let openKinds = $state({});
  let settingsOpen = $state(false);
  // its script edited: its options and its whole script
  let scriptOpen = $state(false);
  // the make-media window (offered on every table)
  let mediaOpen = $state(false);
  // its VPS entry chosen by hand (after Not this one, or any time)
  let pickOpen = $state(false);
  // its ROM changed
  let romOpen = $state(false);
  let backglassOpen = $state(false);
  // an AltSound pack made from its ROM: whether one can be, asked as the page opens, and the window
  // asking first
  let altOffer = $state(null);
  let altMakeOpen = $state(false);
  // its display coloured: the Colour it… window
  let colourOpen = $state(false);
  $effect(() => {
    const id = t?.id;
    altOffer = null;
    if (!id || !t.rom || t.holds.altsound) return;
    invoke('altsound_offer', { id })
      .then((o) => {
        if (t?.id === id) altOffer = o;
      })
      .catch(() => {});
  });
  // its screenshots
  let shotsOpen = $state(false);
  let detailsOpen = $state(false);
  // one kind of its media opened, where it has several (a stack, the one used on top)
  let stackOpen = $state(null);
  // its own screens, the Screens page for it alone (its settings' View, Custom)
  let screensOpen = $state(false);
  // its ROM checked file by file as the page opens: [look, what it says]
  let romCheck = $state(null);
  let more = $state(false);
  // the … menu showing the user's playlists, and a new one's name
  let plMenu = $state(false);
  let plName = $state('');
  let moreBox = $state();
  // how PLAY plays it (`play_mode`): its packs, and what makes it softcore; Play Hardcore's menu
  let playMode = $state(null);
  let playMenu = $state(false);
  let playBox = $state();
  async function loadPlayMode(id) {
    playMode = await invoke('play_mode', { id }).catch(() => null);
  }
  $effect(() => {
    const id = t?.id;
    if (!id) return;
    playMenu = false;
    loadPlayMode(id);
    const off = listen('library-changed', () => loadPlayMode(id));
    return () => off.then((f) => f());
  });

  // its misnamed PinUP pack's rename, asked first
  let fixPack = $state(false);
  // its PinUP pack's Layout window, and whether the pack plays for it: { on, usual }
  let layoutOpen = $state(false);
  let pupOn = $state(null);

  $effect(() => {
    const id = app.tableId;
    const pack = tableById(id)?.holds.pupPack;
    pupOn = null;
    if (pack) invoke('table_pup', { id }).then(([on, usual]) => app.tableId === id && (pupOn = { on, usual })).catch(() => {});
  });

  async function undoPackRename(path) {
    try {
      await invoke('pup_pack_undo_rename', { path });
      notice(`Its PinUP pack is ${t.pupRenamedFrom} again.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function setPupOn(on) {
    const id = t.id;
    try {
      await invoke('set_table_pup', { id, on });
      pupOn = { ...pupOn, on };
      notice(on ? `${t.short} plays its PinUP pack again.` : `${t.short} plays without its PinUP pack now; turn it back on here any time.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // one of its files being removed from its row, asked first: what the engine's plan says of it
  let asking = $state(null);

  $effect(() => {
    void app.tableId;
    fixPack = false;
    asking = null;
    layoutOpen = false;
  });

  async function askRemove(p) {
    try {
      const item = (await invoke('remove_plan', { id: t.id })).find((i) => i.key === p.remove);
      if (item) asking = item;
      else notice('It isn’t there any more.', { kind: 'problem' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function doRemove() {
    const item = asking;
    asking = null;
    try {
      const done = await invoke('remove', { id: t.id, keys: [item.key] });
      notice(done.problems.length ? done.problems.join(' ') : `Moved to the Recycle Bin: ${item.name}.`, { kind: done.problems.length ? 'problem' : 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // what removing a file means, said before it goes
  const AFTER = { rom: 'The game won’t start without its ROM.' };
  const afterRemoving = (item) =>
    [item.key.startsWith('helper:') ? 'Its script may not run without it.' : AFTER[item.key], item.shared.length ? `Also used by ${item.shared.join(', ')}, which needs it too.` : '']
      .filter(Boolean)
      .join(' ');

  // what says its ROM changed: its zip there or not, and the background check's verdict, made
  // again whenever the zip's size or date changes (a new one dropped in)
  const romKey = $derived(t ? `${t.holds.rom || ''}|${t.romCheck ? t.romCheck.join('|') : ''}` : '');
  $effect(() => {
    const id = app.tableId;
    void romKey;
    // the background check's verdict at once, then a fresh one: for another table, or when its ROM
    // changed, not each time the library's read again
    untrack(() => {
      romCheck = tableById(id)?.romCheck ?? null;
      if (tableById(id)?.holds.rom) {
        invoke('rom_check', { id })
          .then((c) => app.tableId === id && (romCheck = c))
          .catch((e) => app.tableId === id && (romCheck = ['warn', String(e)]));
      }
    });
  });

  // another table: its page from the top. Only then — the library read again (a choice made, media
  // put in PinUP Popper, a check done) leaves the page as it is, its window open (else
  // every read would reset the picture shown and close the media window)
  $effect(() => {
    void app.tableId;
    shown = 0;
    more = false;
    stackOpen = null;
  });

  // its VPS files, read again when its match changes (a guess confirmed)
  const sureNow = $derived(!!t?.onVps.sure);
  $effect(() => {
    const id = app.tableId;
    void sureNow;
    vps = [];
    vpsRead = false;
    invoke('vps_files', { id }).then((files) => {
      if (app.tableId === id) {
        vps = files;
        vpsRead = true;
        openKinds = { tables: true };
      }
    });
  });

  const media = $derived(t ? t.media.filter((m) => !failed[m.url]) : []);
  const hero = $derived(media[Math.min(shown, media.length - 1)]);
  // its media by kind, in the order they come, the one used first in each
  const groups = $derived.by(() => {
    const out = [];
    for (const m of media) {
      let g = out.find((x) => x.kind === m.kind);
      if (!g) out.push((g = { kind: m.kind, label: m.label, items: [] }));
      g.items.push(m);
    }
    return out;
  });
  // a kind's tile opens its window, one alone as well: there it's shown large, with PinUP Popper's part
  const showGroup = (g) => (stackOpen = g.kind);

  // the media a front end shows that it has none of, an empty square each so it's plain what's still
  // to get: those our player makes open Make media, the rest VPS's files of their kind where it has some
  const WANTED = [
    ['playfield', 'Playfield', 'make'],
    ['playfield-video', 'Playfield video', 'make'],
    ['backglass', 'Backglass', 'make'],
    ['dmd', 'Score display', 'make'],
    ['topper', 'Topper', 'toppers'],
    ['wheel', 'Wheel', 'make'],
    ['gameinfo', 'Flyer', null],
    ['gamehelp', 'Instruction card', 'rules'],
  ];
  const missing = $derived.by(() => {
    if (!t) return [];
    const has = (k) => groups.some((g) => g.kind === k || (k !== 'playfield' && g.kind === `${k}-video`));
    // what our player found it has none of: no score display wanted; a backglass from VPS's
    // backglass files, which our player draws from
    const none = t.none || [];
    return WANTED.filter(([k]) => !has(k) && !(k === 'dmd' && none.includes('dmd'))).map((w) => (w[0] === 'backglass' && none.includes('backglass') ? [w[0], w[1], 'backglasses'] : w));
  });
  function getMissing([, , how]) {
    if (how === 'make') mediaOpen = true;
    else if (how && vpsCount(how)) find(how);
  }
  // the main picture's arrows: through all its pictures and videos in the strip's order, kind by
  // kind, round from the last to the first
  const ordered = $derived(groups.flatMap((g) => g.items));
  function step(by) {
    const at = Math.max(0, ordered.indexOf(hero));
    shown = Math.max(0, media.indexOf(ordered[(at + by + ordered.length) % ordered.length]));
  }

  function go(other) {
    if (other) openTable(other.id);
  }

  function typing(e) {
    const el = e.target;
    return el && (el.tagName === 'INPUT' || el.tagName === 'SELECT' || el.tagName === 'TEXTAREA' || el.isContentEditable);
  }

  // one of its windows open: the keys are that window's
  const windowOpen = $derived(settingsOpen || scriptOpen || mediaOpen || pickOpen || romOpen || layoutOpen || shotsOpen || detailsOpen || screensOpen || !!stackOpen);

  function keys(e) {
    if (app.page !== 'table' || typing(e) || windowOpen) return;
    if (e.key === 'ArrowLeft') { go(prev); e.preventDefault(); }
    else if (e.key === 'ArrowRight') { go(next); e.preventDefault(); }
    else if (e.key === 'Escape') goLibrary();
  }

  // a controller's shoulder buttons (LB, RB), read while this page shows and the window has focus
  $effect(() => {
    let frame = 0;
    let held = { lb: true, rb: true };
    const poll = () => {
      const pads = navigator.getGamepads ? navigator.getGamepads() : [];
      let lb = false;
      let rb = false;
      for (const p of pads) {
        if (!p) continue;
        lb ||= p.buttons[4]?.pressed;
        rb ||= p.buttons[5]?.pressed;
      }
      if (lb && !held.lb) go(prev);
      if (rb && !held.rb) go(next);
      held = { lb, rb };
      frame = requestAnimationFrame(poll);
    };
    frame = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(frame);
  });

  const vpsCount = (k) => t?.vpsHas?.[k] || 0;
  const lists = (k) => (t?.guess ? 'Not looked for on VPS until its match is confirmed' : !t?.onVps.game ? 'Not on VPS' : vpsCount(k) ? `VPS lists ${vpsCount(k)}` : 'VPS lists none');
  const fileName = (p) => (p ? p.split('\\').pop() : '');
  const size = (b) => (b > 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);

  function find(kind) {
    openKinds = { ...openKinds, [kind]: true };
    document.getElementById(`vps-${kind}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  const later = (what) => () => notice(`${what} comes in a later build.`);

  const parts = $derived.by(() => {
    if (!t) return [];
    const h = t.holds;
    const out = [];
    const match = t.guess ? ['VPS match to confirm', 'warn'] : t.onVps.game ? ['Matched on VPS', 'good'] : ['Not on VPS', 'none'];
    out.push({ icon: 'file', kind: 'Table', detail: `${t.file}.vpx · ${size(t.size)}`, state: match[0], look: match[1], actions: [['Show', () => showFile(t, 'table')]] });
    // other backglasses kept beside it: which one plays is chosen in Change…
    const kept = h.backglasses.length;
    const change = kept ? [['Change…', () => (backglassOpen = true)]] : [];
    out.push(h.backglass
      ? { icon: 'image', kind: 'Backglass', remove: 'backglass', detail: kept ? `${fileName(h.backglass)} · ${kept} more kept beside it` : fileName(h.backglass), state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'backglass')], ...change] }
      : kept
        ? { icon: 'image', kind: 'Backglass', detail: `None plays; ${kept} kept beside it`, state: 'None plays', look: 'none', actions: change }
        : { icon: 'image', kind: 'Backglass', detail: lists('backglasses'), state: 'Not added', look: 'none', actions: vpsCount('backglasses') ? [['Find', () => find('backglasses')]] : [] });
    if (t.rom) {
      // checked file by file once the page opens; a set that isn't right offers VPS's ROMs
      const [look, said] = romCheck || ['good', ''];
      const state = !romCheck ? 'Checking…' : look === 'good' ? 'Complete' : look === 'warn' ? 'Needs a look' : 'Not right';
      out.push(h.rom
        ? { icon: 'chip', kind: 'ROM', remove: 'rom', detail: said && look === 'good' ? `${fileName(h.rom)}: ${said}` : fileName(h.rom), title: said, note: look !== 'good' ? said : '', state, look, actions: [['Show', () => showFile(t, 'rom')], ...(look !== 'good' && vpsCount('roms') ? [['Find', () => find('roms')]] : []), ['Change', () => (romOpen = true)]] }
        : { icon: 'chip', kind: 'ROM', detail: `${t.rom}.zip, which its script asks for`, state: 'Missing', look: 'warn', actions: [...(vpsCount('roms') ? [['Find', () => find('roms')]] : []), ['Change', () => (romOpen = true)]] });
    } else {
      out.push({ icon: 'chip', kind: 'ROM', detail: 'None needed: the rules are in its script', state: 'Not needed', look: 'good', actions: [] });
    }
    out.push(h.pupPack
      ? pupRow(h)
      : h.pupMisnamed
        // there, but under a name its script doesn't hand PinUP: the player won't find it until it's renamed
        ? { icon: 'layers', kind: 'PinUP pack', remove: 'pup-misnamed', detail: `There as ${fileName(h.pupMisnamed)}; its script asks for ${t.pupName}`, title: h.pupMisnamed, note: 'The player finds a pack by the name the table’s script asks for, so this one won’t play until it’s renamed.', state: 'Named wrong', look: 'warn', fix: 'pack', actions: [['Fix', () => (fixPack = true)]] }
        : { icon: 'layers', kind: 'PinUP pack', detail: lists('pupPacks'), state: 'Not added', look: 'none', actions: vpsCount('pupPacks') ? [['Find', () => find('pupPacks')]] : [] });
    if (t.rom) {
      // none: VPS's to find, and one made from its ROM where a pack made so would play (else why not)
      const make = altOffer?.can ? [['Make one from its ROM…', () => (altMakeOpen = true)]] : [];
      out.push(h.altsound
        ? altRow(h)
        : { icon: 'music', kind: 'AltSound', detail: lists('altSound'), note: altOffer && !altOffer.can && h.rom ? altOffer.why : '', state: 'Not added', look: 'none', actions: [...(vpsCount('altSound') ? [['Find', () => find('altSound')]] : []), ...make] });
      // coloured here: its display recorded as it plays, coloured in PPUC's editor, put in place
      const colourIt = ['Color it…', () => (colourOpen = true)];
      out.push(h.colour
        ? { icon: 'palette', kind: 'Color DMD', remove: 'colour', detail: `${h.colour.kind} coloring`, state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'colour')], colourIt] }
        : { icon: 'palette', kind: 'Color DMD', detail: lists('colour'), state: 'Not added', look: 'none', actions: [...(vpsCount('colour') ? [['Find', () => find('colour')]] : []), colourIt] });
    }
    out.push(h.pov
      ? { icon: 'eye', kind: 'POV', remove: 'pov', detail: fileName(h.pov), state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'pov')]] }
      : { icon: 'eye', kind: 'POV', detail: lists('pov'), state: 'Not added', look: 'none', actions: vpsCount('pov') ? [['Find', () => find('pov')]] : [] });
    // the folder's autovpp.vpp, which every table beside it reads, isn't this one's to remove
    if (h.physics) out.push({ icon: 'gauge', kind: 'Its physics', remove: fileName(h.physics).toLowerCase() === 'autovpp.vpp' ? null : 'physics', detail: `${fileName(h.physics)}, read before it plays, as VPX reads it`, state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'physics')]] });
    if (h.settings) out.push({ icon: 'sliders', kind: 'Its settings', remove: 'settings', detail: fileName(h.settings), state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'settings')]] });
    if (h.script) out.push({ icon: 'code', kind: 'Its script', remove: 'script', detail: `${fileName(h.script)}, played in place of its own`, state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'script')]] });
    return out;
  });

  // its PinUP pack's row, checked as the library's read (`library::pup_pack`): the layout in place,
  // Layout… where it offers some, and its switch for this table alone
  function pupRow(h) {
    const c = t.pupCheck;
    const look = c ? c.look : 'good';
    const name = fileName(h.pupPack);
    const setup = c?.layout ? ` · ${c.layout}` : c?.setup === 'own' ? ' · its own setup' : '';
    const detail = look === 'none' ? `${name}: ${c.text.charAt(0).toLowerCase()}${c.text.slice(1)}` : `${name}${setup}`;
    const actions = [['Show', () => showFile(t, 'pupPack')], ...(c?.layouts ? [['Layout…', () => (layoutOpen = true)]] : [])];
    const toggle = look !== 'none' && pupOn ? { on: pupOn.on, set: setPupOn } : null;
    // renamed here for this table: said, with the way back
    const renamed = t.pupRenamedFrom ? `Renamed from ${t.pupRenamedFrom} for this table; it may not fit it.` : '';
    if (t.pupRenamedFrom) actions.push(['Undo rename', () => undoPackRename(h.pupPack)]);
    const said = look === 'warn' || look === 'bad' ? c.text : '';
    return { icon: 'layers', kind: 'PinUP pack', remove: 'pup', detail, stateTitle: c?.text, note: [said, renamed].filter(Boolean).join(' '), state: c ? c.state : 'Added', look, actions, toggle };
  }

  // its script's changes carried over to its new version, or its new script played as it is
  function resolveScript(carry) {
    invoke('script_update', { id: t.id, carry })
      .then((said) => notice(said, { kind: 'done' }))
      .catch((e) => notice(String(e), { kind: 'problem' }));
  }

  // its AltSound pack's row, checked as the library's read (`library::altsound`)
  function altRow(h) {
    const [look, said] = t.altCheck || ['good', ''];
    const state = !t.altCheck ? 'Added' : look === 'good' ? 'Complete' : look === 'warn' ? 'Needs a look' : 'Not right';
    // one made from its ROM said so, with the page of its sounds rom2altsound wrote beside them
    const name = t.altMade ? `${fileName(h.altsound)}, made from its ROM` : fileName(h.altsound);
    const listen = t.altMade ? [['Listen', () => invoke('altsound_listen', { id: t.id }).catch((e) => notice(String(e), { kind: 'problem' }))]] : [];
    return { icon: 'music', kind: 'AltSound', remove: 'altsound', detail: said && look === 'good' ? `${name}: ${said}` : name, note: look !== 'good' ? said : '', state, look, actions: [['Show', () => showFile(t, 'altSound')], ...listen] };
  }

  // a few names, then how many more
  const names = (paths, n = 3) => paths.slice(0, n).map(fileName).join(', ') + (paths.length > n ? `, and ${paths.length - n} more` : '');
  const inFolder = (p) => (p.toLowerCase().includes('\\scripts\\') ? 'with VPX’s scripts' : 'beside the table');
  // a companion's second copy, which nothing reads: [what it is, why]
  const COMPANION = {
    directb2s: ['A second backglass', 'only the one named as the table shows'],
    pov: ['A second POV', 'only the one named as the table is read'],
    ini: ['Second settings', 'only the file named as the table is read'],
    vbs: ['A second script', 'only the one named as the table plays'],
    vpp: ['Second physics', 'only the file named as the table is read'],
  };
  const READ = ['txt', 'pdf', 'md', 'rtf', 'doc', 'docx', 'htm', 'html'];
  const ADDED = {
    music: 'Music', helperScript: 'Its scripts', nvram: 'Machine’s memory', pinmameSettings: 'Machine’s settings', dmdFolder: 'DMD pictures', backglass: 'Backglass',
    pov: 'POV', physics: 'Physics', tableSettings: 'Settings', script: 'Script', table: 'Table', rom: 'ROM', colouring: 'Coloring', media: 'Media', readme: 'Read-me', manual: 'Manual',
  };

  // what it holds beyond the rows above: the music its script plays, the scripts it loads that
  // aren't VPX's own, its DMD's pictures, files beside it named for it, and what the drop window
  // added for it that fits none of them (an "Other" section at the bottom)
  // the kinds of what the drop window added that VPS lists files of, for a Find beside them
  const VPS_KIND = { music: 'sounds', media: 'mediaPacks', readme: 'rules' };
  const findOn = (k) => (k && vpsCount(k) ? [['Find', () => find(k)]] : []);

  const others = $derived.by(() => {
    if (!t) return [];
    const h = t.holds;
    const out = [];
    if (h.music.length) {
      out.push({ icon: 'music', kind: 'Its music', remove: 'music', detail: names(h.music), state: `${h.music.length} ${h.music.length === 1 ? 'file' : 'files'}`, look: 'good', actions: [['Show', () => showFile(t, 'music:0')], ...findOn('sounds')] });
    }
    // a folder its script names for its DMD that isn't there: what it shows from there won't show
    for (const f of h.foldersMissing) {
      out.push({ icon: 'folder', kind: 'DMD pictures', detail: `${f}, which its script names for its DMD’s pictures, isn’t beside it`, state: 'Missing', look: 'warn', actions: [] });
    }
    h.helpers.forEach((x, n) =>
      out.push(x.path
        ? { icon: 'code', kind: 'Its scripts', remove: `helper:${x.path.toLowerCase()}`, detail: `${x.name}, ${inFolder(x.path)}`, state: 'Added', look: 'good', actions: [['Show', () => showFile(t, `helper:${n}`)]] }
        : { icon: 'code', kind: 'Its scripts', detail: `${x.name}, which its script loads, isn’t beside it or with VPX’s scripts`, state: 'Missing', look: 'warn', actions: [] }),
    );
    h.folders.forEach((f, n) => out.push({ icon: 'folder', kind: 'DMD pictures', remove: `folder:${f.toLowerCase()}`, detail: `${fileName(f)}, its DMD’s pictures and fonts`, state: 'Added', look: 'good', actions: [['Show', () => showFile(t, `folder:${n}`)]] }));
    // pictures played in place of its own of the same names, the table file as it is
    if (h.pictures.length) {
      out.push({ icon: 'image', kind: 'Its pictures', remove: 'pictures', detail: `${names(h.pictures)}, played in place of its own of the same ${h.pictures.length === 1 ? 'name' : 'names'}`, title: 'Kept beside the table; the table file stays as it is', state: 'Added', look: 'good', actions: [['Show', () => showFile(t, 'pictures:0')]] });
    }
    h.beside.forEach((f, n) => {
      const ext = f.split('.').pop().toLowerCase();
      const [what, why] = COMPANION[ext] || [READ.includes(ext) ? 'To read' : 'Named for it', ''];
      out.push({ icon: 'file', kind: 'Beside it', remove: `beside:${f.toLowerCase()}`, detail: `${what}: ${fileName(f)}`, title: why ? `${fileName(f)}: ${why}` : fileName(f), state: COMPANION[ext] ? 'Not used' : 'Kept', look: 'none', actions: [['Show', () => showFile(t, `beside:${n}`)]] });
    });
    // the drop window's, a row for each kind
    const kinds = {};
    t.other.forEach((f, n) => (kinds[f.kind] ||= []).push({ ...f, n }));
    for (const [kind, files] of Object.entries(kinds)) {
      const when = new Date(Math.max(...files.map((f) => f.at))).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
      out.push({ icon: 'inbox', kind: ADDED[kind] || 'Added', detail: names(files.map((f) => f.path)), state: `Added ${when}`, look: 'good', actions: [['Show', () => showFile(t, `other:${files[0].n}`)], ...findOn(VPS_KIND[kind])] });
    }
    return out;
  });

  const facts = $derived.by(() => {
    if (!t) return [];
    return [
      ['Players', t.players ? String(t.players) : null],
      ['Board', t.board],
      ['Designers', t.designers.join(', ')],
      ['Theme', t.theme.join(', ')],
      ['ROM', t.rom || 'None'],
      ['This release', t.features.join(', ')],
    ].filter(([, v]) => v);
  });

  // the user's own: when they last played it, how often, for how long in all, and their best score
  const mine = $derived.by(() => {
    const p = t?.played;
    if (!p?.count) return [];
    const best = Math.max(0, ...p.log.flatMap((g) => g.scores));
    return [
      ['Last played', new Date(p.last).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' })],
      ['Times played', String(p.count)],
      ['Time played', p.seconds ? lasted(p.seconds) : null],
      ['Best score', best ? best.toLocaleString() : null],
    ].filter(([, v]) => v);
  });

  // a picture's or video's own menu, on a right-click: show its file, delete it, or hide VPS's
  let menu = $state(null);

  // the big picture's size, for one turned on its side to fill it (its height is the facts' beside
  // it in a wide window, which a turned picture can't measure by itself)
  let heroW = $state(0);
  let heroH = $state(0);

  function menuFor(e, m) {
    e.preventDefault();
    menu = { x: Math.min(e.clientX, innerWidth - 280), y: Math.min(e.clientY, innerHeight - 260), m, confirm: false };
  }

  // a picture or video turned by quarters, in VPI alone (its file as it is): `by` degrees more, or
  // to `to`
  async function turnMedia(m, { by = 0, to = null } = {}) {
    menu = null;
    try {
      await invoke('turn_media', { id: t.id, pick: m.pick, turn: to ?? ((m.turn || 0) + by) % 360 });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // the file behind a picture, as `show_file` names it: PinUP Popper's by its place in the list, ours by kind
  const fileOf = (m) => {
    const [, kind, n] = m.url.match(/\/(t|o)\/[^/]+\/([^/?]+)/) || [];
    return kind === 't' ? `media:${n}` : kind === 'o' ? `own:${n}` : null;
  };

  async function removeMedia(m) {
    menu = null;
    try {
      const done = await invoke('delete_media', { id: t.id, url: m.url });
      shown = 0;
      notice(done === 'hidden' ? `VPS’s ${m.label.toLowerCase()} is hidden from ${t.short}’s page.` : `The ${m.label.toLowerCase()} went to the Recycle Bin.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function unhide() {
    try {
      await invoke('unhide_media', { id: t.id });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  const matchLine = $derived(
    !t ? null
    : t.readError ? ['alert', 'Its file couldn’t be read: ' + t.readError, 'bad']
    : t.guess ? null
    : !t.onVps.game ? ['help', t.onVps.found ? 'Not on VPS, as you chose: its facts come from its own file' : 'Not on VPS: its facts come from its own file', 'none']
    : ['check', `Found on VPS ${t.onVps.found}`, 'good'],
  );

  const date = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '');

  function clickAway(e) {
    if (playMenu && playBox && !playBox.contains(e.target)) playMenu = false;
    if (more && moreBox && !moreBox.contains(e.target)) {
      more = false;
      plMenu = false;
    }
    if (menu && !e.target.closest?.('.ctx')) menu = null;
  }
</script>

<!-- a row's buttons, together at its right: its switch, its buttons, the bin -->
{#snippet buttons(p)}
  <span class="pactions">
    {#if p.toggle}<label class="ptoggle" title="Plays its pack for this table: kept in its own settings"><input type="checkbox" checked={p.toggle.on} onchange={(e) => p.toggle.set(e.currentTarget.checked)} />Enable</label>{/if}
    {#each p.actions as [label, run] (label)}
      <button type="button" class="btn small" onclick={run}>{label}</button>
    {/each}
    {#if p.remove}<button type="button" class="btn small bin" aria-label="Remove its {p.kind.toLowerCase()}…" title="Move it to the Recycle Bin" onclick={() => askRemove(p)}><Icon name="trash" size={15} /></button>{/if}
  </span>
{/snippet}

<svelte:window onkeydown={(e) => (menu && e.key === 'Escape' ? (menu = null) : keys(e))} onclick={clickAway} />

{#if t}
  <div class="subbar">
    <button type="button" class="btn ghost back" onclick={goLibrary}><Icon name="left" />Library</button>
    <div class="nav">
      {#if prev}
        <button type="button" class="btn small step" aria-label="Previous table: {prev.name}" onclick={() => go(prev)}><Icon name="chevL" />{prev.short}</button>
        <span class="pos">{at + 1} of {order.length}</span>
        <button type="button" class="btn small step" aria-label="Next table: {next.name}" onclick={() => go(next)}>{next.short}<Icon name="chevR" /></button>
      {/if}
    </div>
    <div class="hint" aria-hidden="true"><span class="key">LB</span><span class="key">RB</span><span>or the arrow keys</span></div>
  </div>

  <main class="tp">
    <div class="top">
      <div class="left">
        <div class="hero" class:turn-frame={hero?.turn} bind:clientWidth={heroW} bind:clientHeight={heroH} style:--turn-w={heroH ? `${heroH}px` : null} style:--turn-h={heroW ? `${heroW}px` : null} role="presentation" oncontextmenu={(e) => hero && menuFor(e, hero)}>
          {#if hero}
            <!-- one kept on its side (a front end's playfield for a cabinet) shown upright -->
            {#if hero.video}
              <!-- svelte-ignore a11y_media_has_caption -->
              <video src={hero.url} class:turned={hero.turn === 90 || hero.turn === 270} class:ccw={hero.turn === 270} class:upside={hero.turn === 180} autoplay muted={!(hero.sound && app.videoSound)} loop playsinline onerror={() => (failed = { ...failed, [hero.url]: true })}></video>
              <!-- its sound, off until turned on (then on for every video, this run) -->
              {#if hero.sound}
                <button type="button" class="sound" aria-pressed={app.videoSound} aria-label={app.videoSound ? 'Turn its sound off' : 'Turn its sound on'} title={app.videoSound ? 'Its sound is on' : 'Its sound is off'} onclick={() => (app.videoSound = !app.videoSound)}><Icon name={app.videoSound ? 'speaker' : 'mute'} size={18} /></button>
              {/if}
            {:else}
              <img class="behind" src={hero.url} alt="" aria-hidden="true" />
              <img class="front" class:contain={hero.contain} class:turned={hero.turn === 90 || hero.turn === 270} class:ccw={hero.turn === 270} class:upside={hero.turn === 180} src={hero.url} alt="{hero.label} of {t.name}" onerror={() => (failed = { ...failed, [hero.url]: true })} />
            {/if}
            <span class="label">{hero.label}{(hero.copy || hero.from) === 'ours' ? ' · drawn by our player' : !hero.copy && FRONT_END[hero.from] ? ` · from ${FRONT_END[hero.from]}` : ''}</span>
            {#if ordered.length > 1}
              <button type="button" class="step prev" aria-label="The picture before" onclick={() => step(-1)}><Icon name="chevL" size={24} /></button>
              <button type="button" class="step next" aria-label="The next picture" onclick={() => step(1)}><Icon name="chevR" size={24} /></button>
              <span class="count">{ordered.indexOf(hero) + 1} of {ordered.length}</span>
            {/if}
          {:else}
            <div class="named">
              <span class="display">{t.name}</span>
              {#if app.makingIds.includes(t.id)}
                <span class="making">Our player is drawing it, with no window: about half a minute, with a preview video…</span>
              {:else}
                <button type="button" class="btn" onclick={() => (mediaOpen = true)}><Icon name="image" size={16} />Make its media…</button>
                <span class="making">VPS has no pictures of it: our player can draw it, its backglass and its score display, film a preview and make a wheel, with no window</span>
              {/if}
            </div>
          {/if}
        </div>
        {#if media.length || t.screenshots || missing.length}
          <div class="strip" role="group" aria-label="Its pictures and videos">
            {#each groups as g (g.kind)}
              {@const m = g.items[0]}
              {@const many = g.items.length > 1}
              <button type="button" class="thumb" aria-pressed={hero?.kind === g.kind} aria-label={many ? `Its ${g.items.length} ${g.label.toLowerCase()}s` : `Open the ${g.label.toLowerCase()}`} onclick={() => showGroup(g)} oncontextmenu={(e) => (many ? e.preventDefault() : menuFor(e, m))}>
                <span class="frame" class:stack={many} class:on={hero?.kind === g.kind} class:turn-frame={m.turn}>
                  {#if m.video}
                    <!-- its own picture half a second in, as a still -->
                    <!-- svelte-ignore a11y_media_has_caption -->
                    <video src="{m.url}#t=0.5" class:turned={m.turn === 90 || m.turn === 270} class:ccw={m.turn === 270} class:upside={m.turn === 180} muted preload="metadata" playsinline tabindex="-1" onerror={() => (failed = { ...failed, [m.url]: true })}></video>
                    <span class="vid"><Icon name="play" size={18} fill /></span>
                  {:else}
                    <img src={m.url} alt="" loading="lazy" class:contain={m.contain} class:turned={m.turn === 90 || m.turn === 270} class:ccw={m.turn === 270} class:upside={m.turn === 180} onerror={() => (failed = { ...failed, [m.url]: true })} />
                  {/if}
                  {#if many}<span class="count">{g.items.length}</span>{/if}
                </span>
                <span class="cap">{g.label}{many ? ` (${g.items.length})` : ''}</span>
              </button>
            {/each}
            {#if t.screenshots}
              <!-- its screenshots, a stack of pictures, the newest on top -->
              <button type="button" class="thumb" aria-label="Its screenshots: {t.screenshots}" onclick={() => (shotsOpen = true)}>
                <span class="frame stack">
                  {#if t.screenshot}<img src={t.screenshot} alt="" loading="lazy" />{:else}<span class="vid"><Icon name="image" size={18} /></span>{/if}
                  <span class="count">{t.screenshots}</span>
                </span>
                <span class="cap">Screenshots ({t.screenshots})</span>
              </button>
            {/if}
            {#each missing as w (w[0])}
              {@const canGet = w[2] === 'make' || (w[2] && vpsCount(w[2]))}
              <button type="button" class="thumb missing" disabled={!canGet} title={w[2] === 'make' ? `No ${w[1].toLowerCase()} yet: Make media draws one` : canGet ? `No ${w[1].toLowerCase()} yet: VPS has some` : `No ${w[1].toLowerCase()} yet`} onclick={() => getMissing(w)}>
                <span class="frame empty"><Icon name="plus" size={18} /></span>
                <span class="cap">{w[1]}</span>
              </button>
            {/each}
          </div>
        {/if}
        {#if t.hidden}
          <p class="hidden-note">{t.hidden === 1 ? 'One of VPS’s pictures is' : `${t.hidden} of VPS’s pictures are`} hidden from this page. <button type="button" class="link" onclick={unhide}>Show {t.hidden === 1 ? 'it' : 'them'} again</button></p>
        {/if}
      </div>

      <div class="right">
        <div class="kind">
          {#if t.badge}<span class="badge">{t.badge}</span>{/if}
          <span>{[t.maker, t.year, t.kind].filter(Boolean).join(' · ')}</span>
        </div>
        <h1 class="display">{t.name}</h1>
        {#if t.version || t.authors.length}
          <div class="by">{t.version ? `Version ${t.version}` : ''}{t.version && t.authors.length ? ' · ' : ''}{t.authors.length ? `by ${t.authors.join(', ')}` : ''}</div>
        {/if}
        <div class="rating">
          <Stars value={t.rating || 0} onrate={(n) => setRating(t, n)} size={20} label="Your rating" />
          <span class="ratingnote">{t.rating ? (FRONT_NAMES[t.ratingFrom] ? `Its rating in ${FRONT_NAMES[t.ratingFrom]}` : 'Your rating') : 'Not rated yet'}</span>
        </div>
        <div class="buttons">
          <div class="playgroup" bind:this={playBox}>
            <button
              type="button"
              class="playbtn"
              class:split={playMode?.packs}
              title={playMode?.packs ? (playMode.softcore.length ? `Plays Softcore: ${playMode.softcore.join('; ')}` : 'Plays Hardcore: as its author made it') : undefined}
              onclick={() => play(t)}
            >
              <Icon name="play" size={20} fill />PLAY
            </button>
            {#if playMode?.packs}
              <button type="button" class="playmore" aria-label="Play Hardcore or Softcore" title="Play Hardcore or Softcore" aria-expanded={playMenu} onclick={() => (playMenu = !playMenu)}><Icon name="chevD" size={18} /></button>
              {#if playMenu}
                <div class="menu playmenu" role="menu">
                  <p class="pmnow">{playMode.softcore.length ? `PLAY plays it Softcore: ${playMode.softcore.join('; ')}.` : 'PLAY plays it Hardcore: as its author made it.'}</p>
                  <button type="button" role="menuitem" onclick={() => { playMenu = false; play(t, null, 'hardcore'); }}>
                    <Icon name="play" size={16} fill />
                    <span class="pm"><b>Play Hardcore</b><span>As its author made it: your table settings, script edits and physics changes left out for this game. Screens, sound and controls stay yours.</span></span>
                  </button>
                  <button type="button" role="menuitem" onclick={() => { playMenu = false; play(t, null, 'softcore'); }}>
                    <Icon name="play" size={16} />
                    <span class="pm"><b>Play Softcore</b><span>Its achievements kept as softcore this game, never counted for masteries: for a guest, say.</span></span>
                  </button>
                </div>
              {/if}
            {/if}
          </div>
          <button type="button" class="btn square" class:fav={t.favourite} aria-pressed={t.favourite} aria-label={t.favourite ? 'A favourite: take it off' : 'Make it a favourite'} title="Favourite" onclick={() => setFavourite(t, !t.favourite)}><Icon name="heart" size={19} fill={t.favourite} /></button>
          <button type="button" class="btn square" aria-label="Edit its details" title="Edit its details" onclick={() => (detailsOpen = true)}><Icon name="edit" size={19} /></button>
          <button type="button" class="btn square" aria-label="Table settings" title="Table settings" onclick={() => (settingsOpen = true)}><Icon name="sliders" size={19} /></button>
          <button type="button" class="btn square" aria-label="Edit its script" title="Edit its script" onclick={() => (scriptOpen = true)}><Icon name="code" size={19} /></button>
          <button type="button" class="btn square" aria-label="Show its files" title="Show its files" onclick={() => showFile(t, 'table')}><Icon name="folder" size={19} /></button>
          <button type="button" class="btn square" aria-label="Make media" title={app.makingIds.includes(t.id) ? 'Making its media…' : 'Make media'} disabled={app.makingIds.includes(t.id)} onclick={() => (mediaOpen = true)}><Icon name="film" size={19} /></button>
          <button type="button" class="btn square" aria-label="Its achievements" title="Its achievements" onclick={() => { app.prefs.scoreCard = 'achievements'; savePrefs(); tick().then(() => document.getElementById('achievements')?.scrollIntoView({ behavior: 'smooth', block: 'start' })); }}><Icon name="trophy" size={19} /></button>
          <div class="morebox" bind:this={moreBox}>
            <button type="button" class="btn square" aria-label="More" title="More" aria-expanded={more} onclick={() => (more = !more)}><Icon name="more" size={19} /></button>
            {#if more}
              <div class="menu" role="menu">
                {#if plMenu}
                  <!-- the user's playlists, each ticked that holds it -->
                  <button type="button" role="menuitem" class="back" onclick={() => (plMenu = false)}><Icon name="chevL" size={14} />Add to a playlist</button>
                  {#each app.playlists as pl (pl.id)}
                    <button type="button" role="menuitemcheckbox" aria-checked={pl.tables.includes(t.key)} onclick={() => togglePlaylist(pl, t)}>
                      <span class="tick">{#if pl.tables.includes(t.key)}<Icon name="check" size={14} width={2.6} />{/if}</span>{pl.name}
                    </button>
                  {/each}
                  <form class="newpl" onsubmit={(e) => { e.preventDefault(); if (plName.trim()) { newPlaylist(plName, t); plName = ''; } }}>
                    <input class="select" bind:value={plName} placeholder="A new playlist’s name" aria-label="A new playlist’s name" />
                    <button type="submit" class="btn small">Make</button>
                  </form>
                {:else}
                <button type="button" role="menuitem" onclick={() => (plMenu = true)}>Add to a playlist…</button>
                {#if t.vpsLink}<button type="button" role="menuitem" onclick={() => { more = false; openLink(t.vpsLink); }}>Open on VPS’s site</button>{/if}
                <button type="button" role="menuitem" disabled={app.makingIds.includes(t.id)} onclick={() => { more = false; mediaOpen = true; }}>Make media with our player…</button>
                <button type="button" role="menuitem" onclick={() => { more = false; inspect(t.id); }}>Open in the table inspector</button>
                <button type="button" role="menuitem" onclick={() => { more = false; invoke('show_player_log').catch((e) => notice(String(e), { kind: 'problem' })); }}>Show the player’s last log</button>
                <button type="button" role="menuitem" onclick={() => { more = false; app.report = { table: t }; }}>Report a problem with it…</button>
                <button type="button" role="menuitem" onclick={() => { more = false; navigator.clipboard.writeText(t.path).then(() => notice('Its file’s place is copied.', { kind: 'done' })); }}>Copy where its file is</button>
                <button type="button" role="menuitem" class="bad" onclick={() => { more = false; app.removing = t; }}>Remove from library…</button>
                {/if}
              </div>
            {/if}
          </div>
        </div>
        {#if t.scriptUpdate}
          <!-- updated since its script was changed here: its changes carried over to its new script, or not -->
          <div class="match warn script-update">
            <Icon name="code" size={18} />
            <span>
              It was updated since its script was changed here, and the script beside it, with your changes, still plays in its new script’s place. Carry your changes over to its new script?
              <button type="button" class="btn small" onclick={() => resolveScript(true)}>Carry them over</button>
              <button type="button" class="btn small ghost" onclick={() => resolveScript(false)}>Play its new script as it is</button>
            </span>
          </div>
        {/if}
        {#if matchLine}
          <div class="match {matchLine[2]}"><Icon name={matchLine[0]} size={18} width={2.4} /><span>{matchLine[1]} <button type="button" class="link change" onclick={() => (pickOpen = true)}>{t.onVps.game ? 'Not the right one?' : 'Find it on VPS'}</button></span></div>
        {/if}
        <div class="tested">
          {#if app.testingIds.includes(t.id)}
            <div class="match none"><Icon name="clock" size={18} /><span>Testing: running it with no window for about 15 seconds…</span></div>
          {:else if t.tested}
            <div class="match {t.tested.report.look}"><Icon name={t.tested.report.look === 'good' ? 'check' : 'alert'} size={18} width={2.4} /><span>Tested {date(t.tested.at)}: {t.tested.report.text}</span></div>
            <button type="button" class="btn small ghost" onclick={() => testTable(t.id)}>Test again</button>
          {:else}
            <button type="button" class="btn small" onclick={() => testTable(t.id)} title="Runs it for about 15 seconds with no window: its physics, its script, and its machine for a ROM table"><Icon name="play" size={14} />Test it</button>
            <span class="soft-note">About 15 seconds, no window: its script and, for a ROM table, its machine</span>
          {/if}
        </div>
        {#if t.guess}
          <div class="guess">
            <div class="match warn"><Icon name="help" size={18} /><span>Is this <strong>{t.guess.name}</strong>{t.guess.maker || t.guess.year ? ` (${[t.guess.maker, t.guess.year].filter(Boolean).join(' ')})` : ''} on VPS? It’s the nearest VPS lists, but not near enough to be sure.</span></div>
            <div class="choose">
              <button type="button" class="btn small" onclick={() => chooseVps(t, true)}><Icon name="check" size={15} />Yes, that’s it</button>
              <button type="button" class="btn small" onclick={() => (pickOpen = true)}>Not this one</button>
            </div>
          </div>
        {/if}
        <!-- a missing ROM said at the top too; the rest show in their own rows -->
        {#each problems(t).filter((p) => p.kind === 'missing' && p.text.startsWith('Its ROM')) as p (p.text)}
          <div class="match warn"><Icon name="alert" size={18} /><span>{p.text}</span></div>
        {/each}
        <dl class="facts">
          {#each facts as [k, v] (k)}
            <div><dt>{k}</dt><dd>{v}</dd></div>
          {/each}
        </dl>
        <div class="links">
          {#if t.vpsLink}<button type="button" class="btn small" onclick={() => openLink(t.vpsLink)}><Icon name="ext" size={14} />On VPS</button>{/if}
          {#if t.ipdb}<button type="button" class="btn small" onclick={() => openLink(t.ipdb)}><Icon name="ext" size={14} />On IPDB</button>{/if}
        </div>
        {#if mine.length}
          <div class="mine">
            <h2 class="section-title">Your plays</h2>
            <dl class="facts">
              {#each mine as [k, v] (k)}
                <div><dt>{k}</dt><dd>{v}</dd></div>
              {/each}
            </dl>
          </div>
        {/if}
        <PlayLog {t} />
      </div>

      <div class="scores-cell">
        <HighScores {t} />
      </div>
    </div>

    <div class="bottom">
      <section class="panel holds" aria-labelledby="holds">
        <h2 id="holds">What it holds</h2>
        <div class="rows">
        {#each parts as p (p.kind)}
          <div class="part">
            <span class="picon"><Icon name={p.icon} size={17} /></span>
            <span class="pkind">{p.kind}</span>
            <span class="pdetail" title={p.title || p.detail}>{p.detail}</span>
            <span class="pstate {p.look}" title={p.stateTitle}><Icon name={p.look === 'good' ? 'check' : p.look === 'warn' || p.look === 'bad' ? 'alert' : 'minus'} size={15} width={2.4} />{p.state}</span>
            {@render buttons(p)}
            {#if p.note}<p class="pnote {p.look}">{p.note}</p>{/if}
            {#if asking && p.remove && asking.key === p.remove}
              <div class="pask gone">
                <span>Move {asking.name} to the Recycle Bin? {afterRemoving(asking)}</span>
                <button type="button" class="btn small" onclick={doRemove}>Remove</button>
                <button type="button" class="btn small ghost" onclick={() => (asking = null)}>Cancel</button>
              </div>
            {/if}
            {#if p.fix === 'pack' && fixPack}
              <div class="pask">
                <span>Rename the folder {fileName(t.holds.pupMisnamed)} to {t.pupName}? It stays where it is; PinUP Popper finds it by the same name.</span>
                <button type="button" class="btn small" onclick={() => { fixPack = false; renamePacks([t.id]); }}>Rename it</button>
                <button type="button" class="btn small ghost" onclick={() => (fixPack = false)}>Cancel</button>
              </div>
            {/if}
          </div>
        {/each}
        <!-- its achievement packs, each on a line of its own with a bin -->
        {#each achPacks as p, i (p.id)}
          {@const earned = p.achievements.filter((a) => a.earned).length}
          <div class="part">
            <span class="picon" class:blank={i > 0}>{#if i === 0}<Icon name="trophy" size={17} />{/if}</span>
            <span class="pkind">{i === 0 ? 'Achievements' : ''}</span>
            <span class="pdetail" title={p.about || p.name}>{p.name} {p.version}{onlinePacks[p.id] ? ` · ${PACK_KIND[onlinePacks[p.id].kind] || 'Community pack'}` : vpidbPacks.byGame ? ' · only on this PC' : ''}</span>
            <span class="pstate {earned ? 'good' : 'none'}"><Icon name={earned ? 'check' : 'minus'} size={15} width={2.4} />{earned} of {p.achievements.length} earned</span>
            <span class="pactions"><button type="button" class="btn small bin" aria-label="Delete {p.name}…" title="Delete this achievement pack" onclick={() => (askingPack = p)}><Icon name="trash" size={15} /></button></span>
            {#if askingPack?.id === p.id}
              <div class="pask gone">
                <span>Delete the achievement pack {p.name}? It’s deleted for good and can’t be recovered, not even from the Recycle Bin. It goes from every table it’s made for. What you’ve earned with it stays earned.{onlinePacks[p.id] ? '' : ' VPIDB doesn’t have it, so this may be the only copy.'}</span>
                <button type="button" class="btn small danger" onclick={deletePack}>Delete it</button>
                <button type="button" class="btn small ghost" onclick={() => (askingPack = null)}>Keep it</button>
              </div>
            {/if}
          </div>
        {:else}
          <div class="part">
            <span class="picon"><Icon name="trophy" size={17} /></span>
            <span class="pkind">Achievements</span>
            <span class="pdetail">No achievement pack</span>
            <span class="pstate none"><Icon name="minus" size={15} width={2.4} />Not added</span>
            <span class="pactions"><button type="button" class="btn small" onclick={addPack}>Add a pack…</button></span>
          </div>
        {/each}
        <h3 class="other">Other</h3>
        {#each others as p, i (i)}
          <div class="part">
            <span class="picon"><Icon name={p.icon} size={17} /></span>
            <span class="pkind">{p.kind}</span>
            <span class="pdetail" title={p.title || p.detail}>{p.detail}</span>
            <span class="pstate {p.look}"><Icon name={p.look === 'good' ? 'check' : p.look === 'warn' || p.look === 'bad' ? 'alert' : 'minus'} size={15} width={2.4} />{p.state}</span>
            {@render buttons(p)}
            {#if asking && p.remove && asking.key === p.remove}
              <div class="pask gone">
                <span>Move {asking.name} to the Recycle Bin? {afterRemoving(asking)}</span>
                <button type="button" class="btn small" onclick={doRemove}>Remove</button>
                <button type="button" class="btn small ghost" onclick={() => (asking = null)}>Cancel</button>
              </div>
            {/if}
          </div>
        {:else}
          <p class="none-else">Nothing else: no music, scripts of its own or other files for it.</p>
        {/each}
        </div>
        <p class="drop">To add files for {t.short} — a backglass, a ROM, a pack, a POV, music — drop them anywhere on this page. Each goes where it belongs, and you see where before anything is copied.</p>
      </section>

      <div class="side">
        <section class="panel more" aria-labelledby="more">
          <h2 id="more">More on VPS</h2>
          {#if t.guess}
            <p class="quiet">Once its VPS match is confirmed above, everything VPS lists for it shows here.</p>
          {:else if !t.onVps.game}
            <p class="quiet">VPS doesn’t list this table, so there’s nothing more to fetch for it.</p>
          {:else if !vpsRead}
            <p class="quiet">Looking…</p>
          {:else if !vps.length}
            <p class="quiet">VPS lists nothing more for it.</p>
          {:else}
            <VpsFiles kinds={vps} bind:open={openKinds} game={t.onVps?.game ?? null} />
          {/if}
        </section>
      </div>
    </div>
  </main>
{:else}
  <main><p class="quiet">That table isn’t in the library any more.</p><button type="button" class="btn" onclick={goLibrary}>Back to the library</button></main>
{/if}


{#if t && settingsOpen}
  <TableSettings {t} onclose={() => (settingsOpen = false)} onfindvps={() => { settingsOpen = false; pickOpen = true; }} onscreens={() => { settingsOpen = false; screensOpen = true; }} />
{/if}
{#if t && mediaOpen}
  <MakeMedia {t} onclose={() => (mediaOpen = false)} />
{/if}
{#if t && pickOpen}
  <VpsPick {t} onclose={() => (pickOpen = false)} />
{/if}
{#if t && romOpen}
  <ChangeRom {t} onclose={() => (romOpen = false)} />
{/if}
{#if t && backglassOpen}
  <BackglassPick {t} onclose={() => (backglassOpen = false)} />
{/if}
{#if t && scriptOpen}
  <ScriptEditor
    {t}
    onclose={() => (scriptOpen = false)}
    onsettings={() => {
      scriptOpen = false;
      settingsOpen = true;
    }}
    onrom={() => {
      scriptOpen = false;
      romOpen = true;
    }}
  />
{/if}
{#if t && altMakeOpen && altOffer?.can}
  <MakeAltSound {t} rom={altOffer.rom} onclose={() => (altMakeOpen = false)} />
{/if}
{#if colourOpen && t}
  <ColourIt {t} onclose={() => (colourOpen = false)} />
{/if}
{#if t && layoutOpen && t.holds.pupPack}
  <PupLayout path={t.holds.pupPack} title={t.short} tableId={t.id} onclose={() => (layoutOpen = false)} />
{/if}
{#if t && shotsOpen}
  <Screenshots {t} onclose={() => (shotsOpen = false)} />
{/if}
{#if t && detailsOpen}
  <EditDetails {t} onclose={() => (detailsOpen = false)} />
{/if}
{#if t && stackOpen}
  <MediaStack {t} kind={stackOpen} onclose={() => (stackOpen = null)} />
{/if}
{#if t && screensOpen}
  <div class="panel-scrim" role="presentation" onclick={(e) => e.target === e.currentTarget && (screensOpen = false)}>
    <aside class="side-panel" aria-label="Screens for {t.short}">
      <ScreensPage table={t} onclose={() => (screensOpen = false)} />
    </aside>
  </div>
{/if}

{#if menu}
  {@const file = fileOf(menu.m)}
  <div class="ctx" role="menu" style="left: {menu.x}px; top: {menu.y}px">
    {#if menu.confirm}
      <p>Delete the {menu.m.label.toLowerCase()}? It goes to the Recycle Bin, where you can get it back.</p>
      <div class="row">
        <button type="button" class="btn small danger" onclick={() => removeMedia(menu.m)}>Delete</button>
        <button type="button" class="btn small ghost" onclick={() => (menu = null)}>Cancel</button>
      </div>
    {:else}
      <button type="button" role="menuitem" onclick={() => turnMedia(menu.m, { by: 90 })}>Turn it a quarter right</button>
      <button type="button" role="menuitem" onclick={() => turnMedia(menu.m, { by: 270 })}>Turn it a quarter left</button>
      {#if menu.m.turn}<button type="button" role="menuitem" onclick={() => turnMedia(menu.m, { to: 0 })}>Show it as its file has it</button>{/if}
      <hr />
    {/if}
    {#if menu.confirm}
    {:else if menu.m.from === 'VPS'}
      <button type="button" role="menuitem" onclick={() => removeMedia(menu.m)}>Hide it from this page</button>
      <p class="why">It’s one of VPS’s pictures, so it can’t be deleted, only hidden.</p>
    {:else}
      {#if file}<button type="button" role="menuitem" onclick={() => { const f = file; menu = null; showFile(t, f); }}>Show the file</button>{/if}
      <button type="button" role="menuitem" class="bad" onclick={() => (menu.confirm = true)}>Delete…</button>
    {/if}
  </div>
{/if}
<style>
  .subbar {
    position: sticky; top: 61px; z-index: 10; display: flex; flex-wrap: wrap; align-items: center; gap: 10px 16px;
    padding: 10px 28px; background: rgba(13, 16, 22, 0.94); border-bottom: 1px solid var(--line-soft); backdrop-filter: blur(6px);
  }
  .back { padding-left: 8px; }
  .nav { flex: 1 1 200px; display: flex; justify-content: center; align-items: center; gap: 10px; }
  .step { max-width: 260px; overflow: hidden; text-overflow: ellipsis; }
  .pos { color: var(--faint); font-size: 13px; min-width: 64px; text-align: center; }
  .hint { display: flex; align-items: center; gap: 6px; color: var(--faint); font-size: 12px; }
  .key { padding: 2px 7px; border-radius: 5px; border: 1px solid #39425a; color: var(--fg-soft); font-weight: 700; font-size: 11px; }

  main { padding: 24px 28px 44px; display: flex; flex-direction: column; gap: 24px; }
  .top { display: flex; flex-wrap: wrap; gap: 28px; }
  .left { flex: 1 1 560px; min-width: 0; max-width: 980px; display: flex; flex-direction: column; gap: 14px; }
  .hero {
    position: relative; height: min(72vh, 860px); min-height: 360px; border-radius: 16px; overflow: hidden;
    background: var(--picture-bg); border: 1px solid var(--card-line);
  }
  .hero .behind { position: absolute; inset: -40px; width: calc(100% + 80px); height: calc(100% + 80px); object-fit: cover; filter: blur(28px) brightness(0.35) saturate(1.2); }
  .hero .front { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
  .hero .front.contain { padding: 8%; }
  .hero video { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; background: #000; }
  .hero .sound { position: absolute; right: 14px; bottom: 14px; width: 38px; height: 38px; padding: 0; border-radius: 50%; border: 1px solid var(--control-line); background: rgba(8, 10, 15, 0.82); color: var(--fg-soft); display: flex; align-items: center; justify-content: center; cursor: pointer; }
  .hero .sound:hover { color: var(--fg); border-color: var(--accent); }
  .hero .sound[aria-pressed='true'] { color: var(--accent); border-color: var(--accent); }
  .hero .label { position: absolute; left: 14px; bottom: 14px; padding: 4px 10px; border-radius: 7px; background: rgba(8, 10, 15, 0.82); font-size: 12px; font-weight: 600; }
  /* the arrows and the count, shown while the mouse is over the picture */
  .hero .step {
    position: absolute; top: 50%; transform: translateY(-50%); width: 46px; height: 46px; padding: 0; border-radius: 50%;
    display: flex; align-items: center; justify-content: center; border: 1px solid var(--control-line); background: rgba(8, 10, 15, 0.78); color: var(--fg);
    opacity: 0; transition: opacity 0.15s;
  }
  .hero .step:hover { background: var(--selected); border-color: var(--accent); }
  .hero .step.prev { left: 14px; }
  .hero .step.next { right: 14px; }
  .hero .count { position: absolute; right: 14px; top: 14px; padding: 4px 10px; border-radius: 7px; background: rgba(8, 10, 15, 0.82); font-size: 12px; font-weight: 600; opacity: 0; transition: opacity 0.15s; }
  .hero:hover .step, .hero:hover .count, .hero .step:focus-visible { opacity: 1; }
  .named { position: absolute; inset: 0; display: flex; flex-direction: column; gap: 18px; align-items: center; justify-content: center; padding: 30px; text-align: center; background: radial-gradient(circle at 50% 35%, #1b2a44 0%, #0f141d 70%); }
  .named .display { font-size: 64px; line-height: 1; text-transform: uppercase; color: #dbe7ff; text-shadow: 0 2px 16px rgba(76, 194, 255, 0.35); }
  .named .making { max-width: 420px; font-size: 13px; color: var(--muted); }
  .strip { display: flex; flex-wrap: wrap; gap: 12px; }
  .thumb { display: flex; flex-direction: column; gap: 6px; padding: 0; border: 0; background: transparent; color: var(--fg-soft); font-size: 12px; text-align: left; }
  .frame { position: relative; display: block; width: 132px; height: 90px; border-radius: 10px; overflow: hidden; background: var(--picture-bg); outline: 1px solid var(--control-line); }
  .frame.on { outline: 2px solid var(--accent); outline-offset: 2px; }
  .thumb:hover .frame { outline-color: var(--accent); }
  .frame img, .frame video { display: block; width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
  .frame img.contain { object-fit: contain; padding: 6px; }
  .vid {
    position: absolute; left: 50%; top: 50%; width: 34px; height: 34px; margin: -17px 0 0 -17px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center; color: #fff; background: rgba(8, 10, 15, 0.7); padding-left: 2px;
  }
  .cap { max-width: 132px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* a kind it has none of: an empty square, dashed */
  .frame.empty { display: flex; align-items: center; justify-content: center; background: transparent; outline: 1px dashed var(--control-line); color: var(--faint); }
  .thumb.missing { color: var(--faint); }
  .thumb.missing:hover:not(:disabled) .frame { outline-color: var(--accent); color: var(--accent); }
  .thumb.missing:disabled { cursor: default; }
  /* a table's own screens, from the right */
  .panel-scrim { position: fixed; inset: 0; z-index: 45; background: rgba(5, 7, 10, 0.55); display: flex; justify-content: flex-end; }
  .side-panel { width: min(1180px, 94vw); height: 100%; overflow: auto; background: var(--bg); border-left: 1px solid var(--line); box-shadow: -20px 0 50px rgba(0, 0, 0, 0.5); }
  /* a stack: the screenshots', or a kind with several pictures, the one on top used */
  .frame.stack { overflow: visible; background: #1b2232; box-shadow: 4px -4px 0 -1px #141a26, 4px -4px 0 0 var(--control-line), 8px -8px 0 -1px #10151f, 8px -8px 0 0 var(--control-line); margin: 8px 8px 0 0; width: 124px; height: 82px; }
  .frame.stack img, .frame.stack video { border-radius: 10px; }
  .count { position: absolute; right: 6px; bottom: 6px; min-width: 20px; height: 20px; padding: 0 6px; border-radius: 999px; background: rgba(8, 10, 15, 0.85); color: var(--fg); font-size: 11px; font-weight: 700; display: inline-flex; align-items: center; justify-content: center; }
  .hidden-note { margin: 0; color: var(--faint); font-size: 12px; }
  .link { padding: 0; border: 0; background: none; color: var(--accent); font: inherit; cursor: pointer; }
  .link:hover { text-decoration: underline; }
  .link.change { margin-left: 6px; font-size: 13px; color: var(--muted); }
  .mine { display: flex; flex-direction: column; gap: 10px; padding-top: 14px; border-top: 1px solid var(--line-soft); }
  .scores-cell { display: flex; flex-direction: column; min-width: 0; }
  .mine h2 { margin: 0; }
  .ctx {
    position: fixed; z-index: 40; width: 260px; padding: 6px; border-radius: 12px; background: #151a24; border: 1px solid var(--control-line);
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55); display: flex; flex-direction: column; gap: 2px;
  }
  .ctx > button { padding: 9px 12px; border: 0; border-radius: 8px; background: transparent; text-align: left; color: var(--fg); }
  .ctx > button:hover { background: var(--selected); }
  .ctx > button.bad { color: #f0a8a8; }
  .ctx hr { width: 100%; margin: 4px 0; border: 0; border-top: 1px solid var(--line); }
  .ctx p { margin: 4px 8px 8px; font-size: 13px; color: var(--fg-soft); line-height: 1.45; }
  .ctx p.why { color: var(--faint); font-size: 12px; margin-top: 2px; }
  .ctx .row { display: flex; gap: 8px; padding: 0 6px 6px; }
  .danger { background: #4a1d26; border-color: #7a2c3a; }
  .danger:hover:not(:disabled) { background: #5a2430; }

  .right { flex: 1 1 380px; max-width: 560px; display: flex; flex-direction: column; gap: 16px; }
  .kind { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; color: var(--muted); font-size: 14px; }
  .badge { padding: 2px 8px; border-radius: 6px; background: #1f2532; color: var(--fg); font-size: 11px; font-weight: 700; letter-spacing: 0.04em; }
  h1 { font-size: 52px; line-height: 0.98; text-transform: uppercase; overflow-wrap: anywhere; }
  .by { color: var(--fg-soft); font-size: 14px; }
  .rating { display: flex; align-items: center; gap: 10px; margin: -2px 0 2px -2px; }
  .ratingnote { color: var(--faint); font-size: 13px; }
  .buttons { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .playbtn {
    display: inline-flex; align-items: center; gap: 10px; height: 54px; padding: 0 30px; border: 0; border-radius: 14px;
    background: var(--play); color: var(--play-fg); font-size: 18px; font-weight: 800; letter-spacing: 0.04em;
  }
  .playbtn:hover { background: var(--play-hover); }
  .playgroup { position: relative; display: inline-flex; }
  .playbtn.split { border-radius: 14px 0 0 14px; }
  .playmore { height: 54px; width: 40px; display: grid; place-items: center; border: 0; border-left: 1px solid rgba(0, 0, 0, 0.25); border-radius: 0 14px 14px 0; background: var(--play); color: var(--play-fg); }
  .playmore:hover { background: var(--play-hover); }
  .menu.playmenu { left: 0; right: auto; top: 60px; width: 360px; }
  .playmenu button { align-items: flex-start; }
  .playmenu button > :global(svg) { flex: 0 0 auto; margin-top: 3px; }
  .pm { display: flex; flex-direction: column; gap: 3px; }
  .pm b { color: var(--fg); }
  .pm span { color: var(--fg-soft); font-size: 13px; line-height: 1.45; }
  .pmnow { margin: 6px 12px 4px; color: var(--fg-soft); font-size: 12.5px; line-height: 1.45; }
  .fav { color: #ff7aa2; }
  .morebox { position: relative; }
  .menu {
    position: absolute; top: 52px; right: 0; z-index: 15; min-width: 240px; padding: 6px; border-radius: 12px;
    background: #151a24; border: 1px solid var(--control-line); box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55); display: flex; flex-direction: column;
  }
  .menu button { padding: 9px 12px; border: 0; border-radius: 8px; background: transparent; text-align: left; }
  .menu button:hover { background: var(--selected); }
  .menu button { display: flex; align-items: center; gap: 8px; }
  .menu .back { color: var(--muted); font-size: 13px; }
  .menu button.bad { color: #f0a8a8; }
  .menu .tick { display: inline-flex; width: 16px; color: var(--good); }
  .newpl { display: flex; gap: 6px; padding: 6px 6px 4px; border-top: 1px solid var(--line-soft); margin-top: 4px; }
  .newpl input { flex: 1; min-width: 0; height: 32px; }
  .match { display: flex; align-items: flex-start; gap: 8px; font-size: 14px; }
  .match.good { color: var(--good); }
  .match.warn { color: var(--warn); }
  .match.bad { color: var(--bad); }
  .match.none { color: var(--muted); }
  .tested { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; }
  .soft-note { color: var(--faint); font-size: 13px; }
  .guess { display: flex; flex-direction: column; gap: 10px; padding: 12px 14px; border-radius: 12px; background: #221d10; border: 1px solid #4a3d1a; }
  .guess strong { color: #fff3d6; }
  .choose { display: flex; flex-wrap: wrap; gap: 8px; padding-left: 26px; }
  .facts { margin: 4px 0 0; display: flex; flex-direction: column; gap: 10px; font-size: 14px; }
  .facts div { display: flex; gap: 14px; }
  .facts dt { flex: 0 0 110px; color: var(--faint); }
  .facts dd { flex: 1; min-width: 0; margin: 0; }
  .links { display: flex; flex-wrap: wrap; gap: 8px; }

  .bottom { display: flex; flex-wrap: wrap; gap: 24px; align-items: flex-start; }
  .panel { padding: 18px 20px; border-radius: 14px; background: var(--panel); border: 1px solid var(--panel-line); }
  .panel h2 { margin: 0 0 12px; font-size: 16px; font-weight: 700; }
  .holds { flex: 3 1 560px; min-width: 0; }
  /* every row on the same columns — icon, kind, its file, its state, its buttons — the file's name
     cut short where it's long, whole on hover */
  .rows { display: grid; grid-template-columns: 32px minmax(90px, 138px) minmax(0, 1fr) max-content max-content; column-gap: 14px; }
  .part { grid-column: 1 / -1; display: grid; grid-template-columns: subgrid; align-items: center; row-gap: 10px; padding: 10px 2px; border-top: 1px solid #1f2532; }
  .pnote { grid-column: 3 / -1; margin: -4px 0 2px; font-size: 13px; color: var(--muted); }
  .pnote.bad { color: #f0a8a8; }
  .picon.blank { background: transparent; }
  .pask { grid-column: 3 / -1; margin: 0 0 2px; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--control); }
  .pask span { flex: 1 1 320px; font-size: 13px; color: var(--fg-soft); }
  .pask.gone { background: #2a1820; }
  .pask.gone span { color: #f3c4c4; }
  .pactions .bin { padding-left: 8px; padding-right: 8px; color: var(--muted); }
  .pactions .bin:hover { color: #f0a8a8; }
  .picon { display: inline-flex; align-items: center; justify-content: center; width: 32px; height: 32px; border-radius: 8px; background: #1b2130; color: var(--mark-fg); }
  .pkind { font-weight: 600; }
  .pdetail { min-width: 0; color: var(--muted); font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pstate { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; white-space: nowrap; }
  .pstate.good { color: var(--good); }
  .pstate.warn { color: var(--warn); }
  .pstate.bad { color: var(--bad); }
  .pstate.none { color: var(--faint); }
  .ptoggle { display: inline-flex; align-items: center; gap: 6px; margin-right: 4px; font-size: 13px; color: var(--fg-soft); white-space: nowrap; }
  .ptoggle input { accent-color: var(--accent); }
  .pactions { display: flex; align-items: center; gap: 6px; justify-content: flex-end; }
  .drop { margin: 14px 0 0; color: var(--faint); font-size: 13px; line-height: 1.6; }
  .other { grid-column: 1 / -1; margin: 18px 0 4px; font-size: 12px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); }
  .none-else { grid-column: 1 / -1; margin: 0; padding: 10px 2px; border-top: 1px solid #1f2532; color: var(--faint); font-size: 13px; }
  .side { flex: 2 1 420px; min-width: 0; display: flex; flex-direction: column; gap: 24px; }
  .more { min-width: 0; max-height: 640px; display: flex; flex-direction: column; }
  .quiet { color: var(--muted); margin: 0; }

  /* a window of middling width: its pictures and its facts side by side; below them what it holds,
     and beside that its high scores above VPS's files, the tops in line */
  @media (min-width: 1180px) {
    main.tp {
      display: grid; align-items: start; column-gap: 28px; row-gap: 24px;
      grid-template-columns: minmax(520px, 1.5fr) minmax(380px, 1fr);
      grid-template-areas: 'left right' 'holds scores' 'holds side';
    }
    .top, .bottom { display: contents; }
    .left { grid-area: left; max-width: none; align-self: stretch; }
    .right { grid-area: right; max-width: none; }
    .scores-cell { grid-area: scores; }
    .holds { grid-area: holds; }
    .side { grid-area: side; }
    /* the pictures as tall as the facts beside them: no gap under them */
    .hero { flex: 1 0 auto; height: auto; min-height: min(72vh, 860px); }
  }

  /* a wide window (no empty upper right; the high scores to the right of the table and What it
     holds and More on VPS lined up): three columns — its pictures, its facts with the user's plays, its high scores — and below them what it holds beside VPS's files */
  @media (min-width: 1880px) {
    main.tp {
      grid-template-columns: minmax(560px, 1.6fr) minmax(380px, 1fr) minmax(420px, 1fr);
      grid-template-areas: 'left right scores' 'holds holds side';
    }
    .scores-cell { align-self: stretch; }
    .scores-cell :global(.scores) { flex: 1; }
    .more { max-height: none; }
  }
</style>
