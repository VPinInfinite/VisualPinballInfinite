<script>
  // Settings: Screens. Your screens drawn as they sit on your desk, the player's
  // windows on them — the playfield, the backglass, the score display and the topper — dragged
  // and sized there, or on the real screens themselves (Arrange); a job for each window, ready-made
  // layouts for the screens you have, and what decides what shows where. Saved into the settings
  // file the player reads when a table starts, under VPX's own names. On the page:
  // the table's view near the top, above the layouts; each window's card folded until clicked,
  // what decides how its score display shows inside it; a layout lit while the windows are as it
  // put them, saved or not, and chosen again to go back to what was there before it. Presets: Save as
  // preset keeps the page as a preset of its own, which a table can
  // play with; and the page for one table alone (`table`: its settings' View, Custom), saved into
  // its own settings file over the usual.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import { app, notice, savePrefs } from './store.svelte.js';
  import { KINDS, ORDER, shapeName } from './windows.js';

  let { table = null, onclose = null } = $props();

  let view = $state(null);
  // the presets kept, and a new one's name while it's asked for
  let presets = $state([]);
  let naming = $state(false);
  let presetName = $state('');
  let renaming = $state(null);
  let draft = $state(null);
  let saved = $state('');
  let selected = $state('playfield');
  // the windows' cards opened, all folded when the page opens
  let open = $state({});
  let arranging = $state(false);
  let busy = $state(false);
  let problem = $state('');
  let mapEl = $state(null);

  const changed = $derived(!!draft && JSON.stringify(draft) !== saved);

  // each window's shape, its width over its height: kept while its corner is dragged or a size is
  // typed, if "Keep its shape" is ticked (remembered with the app's preferences); "Reset its
  // shape" puts back the one it had when the page opened or a layout was chosen
  let shapes = $state({});
  const keeps = (id) => app.prefs.keepShape?.[id] ?? true;

  function setKeep(id, on) {
    app.prefs.keepShape = { ...(app.prefs.keepShape || {}), [id]: on };
    savePrefs();
  }

  function noteShapes() {
    const out = {};
    for (const w of draft.windows) out[w.id] = shapeOf(w);
    shapes = out;
  }

  function shapeOf(w) {
    const r = deskRect(w);
    return r.w / r.h;
  }

  const reshaped = (w) => w.output !== 'off' && !w.fullscreen && shapes[w.id] > 0 && Math.abs(shapeOf(w) / shapes[w.id] - 1) > 0.01;

  async function load() {
    problem = '';
    try {
      const v = await invoke('screens_view', { table: table?.id ?? null });
      presets = await invoke('screen_presets').catch(() => []);
      view = v;
      draft = structuredClone(v.settings);
      for (const id of ORDER) if (!draft.windows.find((w) => w.id === id)) draft.windows.push(fallback(id));
      draft.windows.sort((a, b) => ORDER.indexOf(a.id) - ORDER.indexOf(b.id));
      saved = JSON.stringify(draft);
      noteShapes();
    } catch (e) {
      problem = String(e);
    }
  }

  onMount(() => {
    load();
    // Done or Cancel pressed on a stand-in window (Enter or Esc there)
    const off = listen('arrange-finish', (e) => doneArranging(!!e.payload?.keep));
    return () => {
      off.then((f) => f());
      if (arranging) invoke('cancel_arranging');
    };
  });

  function fallback(id) {
    const s = mainScreen();
    return { id, output: id === 'playfield' ? 'window' : 'off', screen: s?.name || '', fullscreen: id === 'playfield', x: 0, y: 0, width: s?.width || 1280, height: s?.height || 720 };
  }

  const screens = $derived(view?.screens || []);
  const mainScreen = () => screens.find((s) => s.primary) || screens[0];
  const screenOf = (w) => screens.find((s) => s.name === w.screen) || mainScreen();
  const win = (id) => draft?.windows.find((w) => w.id === id);

  // the desk: every screen's place, in desktop pixels
  const desk = $derived.by(() => {
    if (!screens.length) return { x: 0, y: 0, w: 1, h: 1 };
    const x = Math.min(...screens.map((s) => s.x));
    const y = Math.min(...screens.map((s) => s.y));
    const r = Math.max(...screens.map((s) => s.x + s.width));
    const b = Math.max(...screens.map((s) => s.y + s.height));
    return { x, y, w: r - x, h: b - y };
  });

  const pct = (v, of) => `${(v / of) * 100}%`;

  /** A window's place on the desk, in desktop pixels. */
  function deskRect(w) {
    if (w.output === 'inside') {
      const pf = win('playfield');
      const s = screenOf(pf);
      return { x: s.x + pf.x + w.x, y: s.y + pf.y + w.y, w: w.width, h: w.height };
    }
    const s = screenOf(w);
    if (w.fullscreen) return { x: s.x, y: s.y, w: s.width, h: s.height };
    return { x: s.x + w.x, y: s.y + w.y, w: w.width, h: w.height };
  }

  function inches(s) {
    if (!s.sizeCm) return '';
    const [a, b] = s.sizeCm;
    return `${Math.round(Math.hypot(a, b) / 2.54)}″`;
  }

  /** What a folded card says of its window. */
  function summary(w) {
    if (w.output === 'off') return 'Not shown';
    if (w.output === 'inside') return `Inside the table’s window, ${w.width}×${w.height}`;
    const s = screenOf(w);
    const on = s ? `${s.number}: ${s.name}` : w.screen;
    return w.fullscreen ? `Fills ${on}` : `${w.width}×${w.height} on ${on}`;
  }

  function jobsOn(s) {
    return draft?.windows.filter((w) => w.output === 'window' && w.screen === s.name).map((w) => KINDS[w.id].label) || [];
  }

  // dragging and sizing on the map
  let drag = null;

  function down(e, id, mode) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    selected = id;
    const w = win(id);
    if (!w || !mapEl) return;
    drag = { id, mode, sx: e.clientX, sy: e.clientY, orig: { ...w }, rect: deskRect(w), per: desk.w / mapEl.clientWidth };
    e.currentTarget.setPointerCapture(e.pointerId);
  }

  function snapTo(v, edge, near) {
    return Math.abs(v - edge) <= near ? edge : v;
  }

  function moveTo(e) {
    if (!drag) return;
    const w = win(drag.id);
    const dx = Math.round((e.clientX - drag.sx) * drag.per);
    const dy = Math.round((e.clientY - drag.sy) * drag.per);
    const o = drag.orig;
    if (w.output === 'inside') {
      const pf = win('playfield');
      if (drag.mode === 'move') {
        w.x = clamp(o.x + dx, 0, pf.width - w.width);
        w.y = clamp(o.y + dy, 0, pf.height - w.height);
      } else if (keeps(w.id)) {
        Object.assign(w, sized(o, o.width + dx, o.height + dy, pf.width - w.x, pf.height - w.y, 32));
      } else {
        w.width = clamp(o.width + dx, 32, pf.width - w.x);
        w.height = clamp(o.height + dy, 32, pf.height - w.y);
      }
      return;
    }
    if (drag.mode === 'move') {
      // the screen under its middle takes it
      const cx = drag.rect.x + drag.rect.w / 2 + dx;
      const cy = drag.rect.y + drag.rect.h / 2 + dy;
      const s = screens.find((s) => cx >= s.x && cx < s.x + s.width && cy >= s.y && cy < s.y + s.height) || screenOf(w);
      w.screen = s.name;
      if (w.fullscreen) return;
      Object.assign(w, fitted(o, s.width, s.height, keeps(w.id)));
      const near = Math.round(Math.max(s.width, s.height) * 0.015);
      let x = drag.rect.x + dx - s.x;
      let y = drag.rect.y + dy - s.y;
      x = snapTo(snapTo(x, 0, near), s.width - w.width, near);
      y = snapTo(snapTo(y, 0, near), s.height - w.height, near);
      w.x = clamp(x, 0, s.width - w.width);
      w.y = clamp(y, 0, s.height - w.height);
    } else {
      const s = screenOf(w);
      if (w.fullscreen) {
        w.fullscreen = false;
        Object.assign(o, { x: 0, y: 0, width: s.width, height: s.height });
        w.x = 0;
        w.y = 0;
      }
      const near = Math.round(Math.max(s.width, s.height) * 0.015);
      if (keeps(w.id)) {
        Object.assign(w, sized(o, o.width + dx, o.height + dy, s.width - w.x, s.height - w.y, 64, near));
      } else {
        w.width = clamp(snapTo(o.width + dx, s.width - w.x, near), 64, s.width - w.x);
        w.height = clamp(snapTo(o.height + dy, s.height - w.y, near), 64, s.height - w.y);
      }
    }
  }

  /** The size a window sized from `from` to `width` by `height` takes keeping its shape: the side
   *  pulled further leads and the other follows, an edge within `near` takes it, and it stays
   *  within `maxW` by `maxH`, at least `min` each way where there's room. */
  function sized(from, width, height, maxW, maxH, min, near = 0) {
    const ratio = from.width / from.height;
    let w = width;
    let h = height;
    if (Math.abs(w / from.width - 1) >= Math.abs(h / from.height - 1)) h = w / ratio;
    else w = h * ratio;
    if (near && Math.abs(w - maxW) <= near) {
      w = maxW;
      h = w / ratio;
    } else if (near && Math.abs(h - maxH) <= near) {
      h = maxH;
      w = h * ratio;
    }
    const g = Math.max(1, min / w, min / h);
    const f = Math.min(1, maxW / (w * g), maxH / (h * g));
    return { width: Math.round(w * g * f), height: Math.round(h * g * f) };
  }

  /** A window's size within `maxW` by `maxH`: both sides made smaller alike if it keeps its shape. */
  function fitted(w, maxW, maxH, keep) {
    if (!keep) return { width: Math.min(w.width, maxW), height: Math.min(w.height, maxH) };
    const f = Math.min(1, maxW / w.width, maxH / w.height);
    return { width: Math.round(w.width * f), height: Math.round(w.height * f) };
  }

  /** The room a window has: its screen, or the table's window for one shown inside it. */
  function roomOf(w) {
    if (w.output !== 'inside') return screenOf(w);
    const pf = win('playfield');
    return { width: pf.width, height: pf.height };
  }

  /** The shape it had put back: as wide as it is, its height following, within its room. */
  function resetShape(w) {
    const ratio = shapes[w.id];
    if (!(ratio > 0)) return;
    const room = roomOf(w);
    const f = Math.min(1, (room.width - w.x) / w.width, (room.height - w.y) / (w.width / ratio));
    const width = Math.round(w.width * f);
    Object.assign(w, { width, height: Math.round(width / ratio) });
  }

  function up() {
    if (!drag) return;
    const w = win(drag.id);
    const s = screenOf(w);
    if (w.output !== 'inside' && !w.fullscreen && w.x === 0 && w.y === 0 && w.width === s.width && w.height === s.height) w.fullscreen = true;
    drag = null;
  }

  const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

  function setOutput(w, output) {
    w.output = output;
    if (output === 'inside') {
      const pf = win('playfield');
      Object.assign(w, fitted(w, Math.round(pf.width / 3), Math.round(pf.height / 3), keeps(w.id)));
      w.x = clamp(w.x, 0, pf.width - w.width);
      w.y = clamp(w.y, 0, pf.height - w.height);
    } else if (output === 'window') {
      const s = screenOf(w);
      Object.assign(w, fitted(w, s.width, s.height, keeps(w.id)));
      w.x = clamp(w.x, 0, s.width - w.width);
      w.y = clamp(w.y, 0, s.height - w.height);
    }
  }

  function setScreen(w, name) {
    w.screen = name;
    const s = screenOf(w);
    Object.assign(w, fitted(w, s.width, s.height, keeps(w.id)));
    w.x = clamp(w.x, 0, s.width - w.width);
    w.y = clamp(w.y, 0, s.height - w.height);
  }

  function setFill(w, on) {
    w.fullscreen = on;
    if (on) {
      const s = screenOf(w);
      Object.assign(w, { x: 0, y: 0, width: s.width, height: s.height });
    }
  }

  function setNumber(w, key, value) {
    const v = Math.round(Number(value));
    if (!Number.isFinite(v)) return;
    const box = roomOf(w);
    const keep = keeps(w.id) && (key === 'width' || key === 'height');
    if (keep) Object.assign(w, sized(w, key === 'width' ? v : w.width, key === 'height' ? v : w.height, box.width - w.x, box.height - w.y, 32));
    else if (key === 'width') w.width = clamp(v, 32, box.width - w.x);
    else if (key === 'height') w.height = clamp(v, 32, box.height - w.y);
    else if (key === 'x') w.x = clamp(v, 0, box.width - w.width);
    else w.y = clamp(v, 0, box.height - w.height);
    if (w.output !== 'inside') w.fullscreen = w.x === 0 && w.y === 0 && w.width === box.width && w.height === box.height;
  }

  // ready-made layouts for the screens there are
  const layouts = $derived.by(() => {
    if (!screens.length) return [];
    const main = mainScreen();
    const others = screens.filter((s) => s !== main).sort((a, b) => b.width * b.height - a.width * a.height);
    const out = [];
    const full = (id, s) => ({ id, output: 'window', screen: s.name, fullscreen: true, x: 0, y: 0, width: s.width, height: s.height });
    const off = (id) => ({ ...fallback(id), output: 'off' });
    out.push({
      name: 'One screen',
      text: `The table fills ${main.name}; a ROM table’s score display floats over it.`,
      apply: { windows: [full('playfield', main), off('backglass'), off('scoreView'), off('topper')], floatingScore: true },
    });
    if (others.length >= 1) {
      out.push({
        name: 'Table and backglass',
        text: `The table on ${main.name}, the backglass filling ${others[0].name}, the score display in its speaker panel.`,
        apply: { windows: [full('playfield', main), full('backglass', others[0]), off('scoreView'), off('topper')], backglassDmd: 'auto', showGrill: true },
      });
      const wide = [main, ...others].find((s) => s !== main && s.width / s.height >= 2.2);
      if (wide) {
        // the backglass at the screen's full height, 4:3; the topper and the score display beside it, 16:9
        const H = wide.height;
        const bw = Math.round((H * 4) / 3);
        const side = Math.floor((wide.width - bw) / 2);
        const sh = Math.round((side * 9) / 16);
        const sy = Math.round((H - sh) / 2);
        out.push({
          name: 'Ultrawide',
          text: `The table on ${main.name}; on ${wide.name}, side by side: the topper, the backglass and the score display.`,
          apply: {
            windows: [
              full('playfield', main),
              { id: 'backglass', output: 'window', screen: wide.name, fullscreen: false, x: side, y: 0, width: bw, height: H },
              { id: 'scoreView', output: 'window', screen: wide.name, fullscreen: false, x: side + bw, y: sy, width: side, height: sh },
              { id: 'topper', output: 'window', screen: wide.name, fullscreen: false, x: 0, y: sy, width: side, height: sh },
            ],
          },
        });
      }
    }
    if (others.length >= 2) {
      out.push({
        name: 'Three screens',
        text: `The table on ${main.name}, the backglass on ${others[0].name}, the score display on ${others[1].name}.`,
        apply: { windows: [full('playfield', main), full('backglass', others[0]), full('scoreView', others[1]), off('topper')] },
      });
    }
    if (others.length >= 3) {
      out.push({
        name: 'Four screens',
        text: `As three screens, with the topper on ${others[2].name}.`,
        apply: { windows: [full('playfield', main), full('backglass', others[0]), full('scoreView', others[1]), full('topper', others[2])] },
      });
    }
    return out;
  });

  /** Whether `windows` are as layout `l` puts them: where it shows them, on which screen, and, for
   *  those not filling their screen, where and how big, to a pixel or two. */
  function matches(l, windows = draft.windows) {
    return ORDER.every((id) => {
      const a = l.apply.windows.find((w) => w.id === id);
      const b = windows.find((w) => w.id === id);
      if (!a || !b || a.output !== b.output) return false;
      if (a.output === 'off') return true;
      if (a.screen !== b.screen || a.fullscreen !== b.fullscreen) return false;
      return a.fullscreen || ['x', 'y', 'width', 'height'].every((k) => Math.abs(a[k] - b[k]) <= 2);
    });
  }

  const chosen = $derived(draft ? layouts.find((l) => matches(l)) : null);

  // what the settings were before a layout was chosen, to go back to by choosing it again: this
  // visit's, else the one kept when it was saved
  let before = $state(null);
  const kept = (d) => ({ windows: d.windows, view: d.view, floatingScore: d.floatingScore, backglassDmd: d.backglassDmd, showGrill: d.showGrill });

  function useLayout(l) {
    if (chosen === l) {
      unchoose(l);
      return;
    }
    if (!chosen) before = $state.snapshot(kept(draft));
    draft.windows = l.apply.windows.map((w) => ({ ...w }));
    if (l.apply.floatingScore !== undefined) draft.floatingScore = l.apply.floatingScore;
    if (l.apply.backglassDmd) draft.backglassDmd = l.apply.backglassDmd;
    if (l.apply.showGrill !== undefined) draft.showGrill = l.apply.showGrill;
    // a portrait screen for the table: the cabinet's view, the playfield turned up it
    const pf = screenOf(win('playfield'));
    if (pf && pf.height > pf.width) draft.view = 'cabinet';
    noteShapes();
  }

  /** Back to what was there before layout `l`, or else the player's own: the table alone, filling
   *  the main screen. */
  function unchoose(l) {
    const back = before || app.prefs.screensBefore;
    if (back?.windows && !matches(l, back.windows)) {
      Object.assign(draft, structuredClone($state.snapshot(back)));
      for (const id of ORDER) if (!win(id)) draft.windows.push(fallback(id));
      draft.windows.sort((a, b) => ORDER.indexOf(a.id) - ORDER.indexOf(b.id));
    } else {
      const main = mainScreen();
      draft.windows = ORDER.map((id) => ({ ...fallback(id), screen: main?.name || '' }));
      if (l.name === 'One screen') notice('The table alone on the main screen is the player’s own way; there’s nothing before it to go back to.');
    }
    before = null;
    noteShapes();
  }

  async function save() {
    busy = true;
    try {
      await invoke('save_screens', { settings: $state.snapshot(draft), table: table?.id ?? null });
      saved = JSON.stringify(draft);
      // what was there before the layout saved, for choosing it again to go back to
      if (chosen && before && !table) {
        app.prefs.screensBefore = $state.snapshot(before);
        savePrefs();
      } else if (!chosen && app.prefs.screensBefore) {
        delete app.prefs.screensBefore;
        savePrefs();
      }
      notice(table ? `Saved. ${table.short} plays with these screens from now on.` : 'Saved. Tables started from now on use these screens.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  function undo() {
    draft = JSON.parse(saved);
    before = null;
    noteShapes();
  }

  /** The page kept as a preset, under the name given. */
  async function savePreset() {
    const name = presetName.trim();
    if (!name) return;
    try {
      await invoke('save_screen_preset', { name, settings: $state.snapshot(draft) });
      presets = await invoke('screen_presets');
      notice(`Kept as the preset “${name}”: a table can play with it, from its settings.`, { kind: 'done' });
      naming = false;
      presetName = '';
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  /** A preset shown on the page, to save as the usual, or change and keep again. */
  async function usePreset(name) {
    try {
      const s = await invoke('screen_preset', { name });
      for (const id of ORDER) if (!s.windows.find((w) => w.id === id)) s.windows.push(fallback(id));
      s.windows.sort((a, b) => ORDER.indexOf(a.id) - ORDER.indexOf(b.id));
      draft = s;
      noteShapes();
      notice(`“${name}” is on the page: Save makes it ${table ? 'this table’s' : 'your usual'}.`);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function changePreset(name, to) {
    try {
      await invoke('change_screen_preset', { name, to });
      presets = await invoke('screen_presets');
      renaming = null;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function showFile() {
    try {
      await invoke('show_settings_file');
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function identify() {
    const jobs = {};
    for (const s of screens) jobs[s.name] = jobsOn(s).join(', ') || 'Nothing on it';
    try {
      await invoke('identify_screens', { jobs });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function arrange() {
    try {
      // the stand-ins keep the shapes kept here
      const keep = {};
      for (const w of draft.windows) if (keeps(w.id)) keep[w.id] = shapeOf(w);
      await invoke('arrange_windows', { windows: $state.snapshot(draft.windows), keep });
      arranging = true;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function doneArranging(keep) {
    if (!arranging) return;
    arranging = false;
    try {
      if (keep) draft.windows = await invoke('arranged_windows', { windows: $state.snapshot(draft.windows) });
      else await invoke('cancel_arranging');
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  const overlayChoices = [
    { v: 'off', label: 'Off' },
    { v: 'auto', label: 'Found in its picture' },
    { v: 'placed', label: 'Where the settings file places it' },
  ];
  const pick = (choices, v) => choices.findIndex((c) => c.v === v);
</script>

<div class="page" role="presentation" data-loading={draft || problem ? undefined : ''} onpointermove={moveTo} onpointerup={up} onpointercancel={up}>
  <header>
    <div class="title">
      <h1 class="display">{table ? `Screens for ${table.short}` : 'Screens'}</h1>
      <p>{#if table}For this table alone, kept in its own settings file over your usual screens; what's the same as them isn't written.{:else}Where each of the player’s windows goes. Drag them on the map, or on your screens themselves; the player takes them from here whenever a table starts.{/if}</p>
    </div>
    <div class="actions">
      {#if naming}
        <form class="name-it" onsubmit={(e) => { e.preventDefault(); savePreset(); }}>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="select" bind:value={presetName} placeholder="The preset’s name: No Topper…" aria-label="The preset’s name" autofocus />
          <button type="submit" class="btn small">Keep it</button>
          <button type="button" class="btn small ghost" onclick={() => (naming = false)}>Cancel</button>
        </form>
      {:else}
        <button type="button" class="btn" onclick={() => (naming = true)} disabled={!draft}><Icon name="layers" />Save as preset…</button>
      {/if}
      <button type="button" class="btn" onclick={identify} disabled={!screens.length}><Icon name="eye" />Identify screens</button>
      <button type="button" class="btn" onclick={arrange} disabled={!screens.length || arranging}><Icon name="grid" />Arrange on your screens</button>
      <button type="button" class="btn ghost" onclick={undo} disabled={!changed || busy}>Undo changes</button>
      <button type="button" class="btn save" onclick={save} disabled={!changed || busy}><Icon name="check" />Save</button>
      {#if onclose}<button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>{/if}
    </div>
  </header>

  {#if problem}
    <div class="problem">{problem} <button type="button" class="btn small" onclick={load}><Icon name="refresh" size={15} />Try again</button></div>
  {:else if !draft}
    <p class="soft">Looking at your screens…</p>
  {:else}
    {#if arranging}
      <div class="arranging" role="status">
        <Icon name="grid" />
        <span>Each window on your screens stands for one of the player’s. Drag it by its middle, and its edges or corner to size it, then Done — here or on any of them (Enter); close to a screen’s edge, it’s taken to the edge.</span>
        <button type="button" class="btn small" onclick={() => doneArranging(false)}>Cancel</button>
        <button type="button" class="btn small save" onclick={() => doneArranging(true)}><Icon name="check" size={15} />Done</button>
      </div>
    {/if}

    {#each Object.entries(view.missing || {}) as [id, name] (id)}
      <div class="missing" role="status"><Icon name="alert" size={17} />The {KINDS[id].label.toLowerCase()} is set for {name}, which isn’t connected; until it is, the player puts it on the main screen, as shown here.</div>
    {/each}

    <section class="map-wrap" aria-label="Your screens">
      <div class="map" bind:this={mapEl} style="aspect-ratio: {desk.w} / {desk.h}; width: min(100%, {Math.round((380 * desk.w) / desk.h)}px)">
        {#each screens as s (s.name)}
          <div class="screen" class:main={s.primary} style="left: {pct(s.x - desk.x, desk.w)}; top: {pct(s.y - desk.y, desk.h)}; width: {pct(s.width, desk.w)}; height: {pct(s.height, desk.h)}">
            <span class="tag"><span class="label"><b>{s.number}</b> {s.name}{s.primary ? ' · main' : ''}</span>
            <span class="facts">{s.width}×{s.height} · {Math.round(s.scale)}% · {Math.round(s.refresh)} Hz{inches(s) ? ` · ${inches(s)}` : ''}{s.hdr ? ' · HDR' : ''}</span></span>
          </div>
        {/each}
        {#each draft.windows.filter((w) => w.output !== 'off') as w (w.id)}
          {@const r = deskRect(w)}
          <div
            class="win"
            class:sel={selected === w.id}
            class:inside={w.output === 'inside'}
            role="button"
            tabindex="0"
            aria-label="{KINDS[w.id].label}: drag to move"
            style="left: {pct(r.x - desk.x, desk.w)}; top: {pct(r.y - desk.y, desk.h)}; width: {pct(r.w, desk.w)}; height: {pct(r.h, desk.h)}; --c: {KINDS[w.id].colour}"
            onpointerdown={(e) => down(e, w.id, 'move')}
            ondblclick={() => w.output !== 'inside' && setFill(w, !w.fullscreen)}
            onkeydown={(e) => e.key === 'Enter' && (selected = w.id)}
          >
            <span class="wname">{KINDS[w.id].label}</span>
            <span class="wsize">{w.fullscreen ? 'fills the screen' : `${w.width}×${w.height} · ${shapeName(w.width / w.height)}`}</span>
            <span class="grip" role="presentation" onpointerdown={(e) => down(e, w.id, 'size')}></span>
          </div>
        {/each}
      </div>
      <p class="hint">Drag a window to move it — onto another screen too — and its corner to size it, its shape kept while Keep its shape is ticked; double-click to make it fill its screen.</p>
    </section>

    <section class="block">
      <h2 class="section-title">The table’s view</h2>
      <div class="seg wide" role="radiogroup" aria-label="The table’s view">
        <button type="button" role="radio" aria-checked={draft.view === 'desktop'} onclick={() => (draft.view = 'desktop')}>Desktop</button>
        <button type="button" role="radio" aria-checked={draft.view === 'cabinet'} onclick={() => (draft.view = 'cabinet')}>Cabinet</button>
        <button type="button" role="radio" aria-checked={draft.view === 'fss'} onclick={() => (draft.view = 'fss')}>Full single screen</button>
      </div>
      <p class="note">
        {#if draft.view === 'cabinet'}The view for a cabinet’s playfield screen, looking down on the table as a player standing at it; on a portrait screen the table runs up it.{:else if draft.view === 'fss'}The whole table and its backglass on one portrait screen, as some tables offer.{:else}The view for a desk: the table seen from in front, its score display on its backdrop.{/if}
      </p>
    </section>

    <section class="block">
      <h2 class="section-title">Ready-made layouts</h2>
      <div class="layouts" role="radiogroup" aria-label="Ready-made layouts">
        {#each layouts as l (l.name)}
          <button type="button" class="layout" role="radio" aria-checked={chosen === l} onclick={() => useLayout(l)} title={chosen === l ? 'Chosen — choose it again to go back to what was there before it' : undefined}>
            <b>{#if chosen === l}<Icon name="check" size={15} />{/if}{l.name}</b><span>{l.text}</span>
          </button>
        {/each}
      </div>
    </section>

    <section class="block">
      <h2 class="section-title">Presets</h2>
      {#if presets.length}
        <div class="presets">
          {#each presets as p (p)}
            <div class="preset">
              {#if renaming === p}
                <form class="name-it" onsubmit={(e) => { e.preventDefault(); changePreset(p, e.currentTarget.elements.to.value); }}>
                  <!-- svelte-ignore a11y_autofocus -->
                  <input class="select" name="to" value={p} aria-label="{p}’s new name" autofocus />
                  <button type="submit" class="btn small">Rename</button>
                  <button type="button" class="btn small ghost" onclick={() => (renaming = null)}>Cancel</button>
                </form>
              {:else}
                <b>{p}</b>
                <button type="button" class="btn small" onclick={() => usePreset(p)}>Show it here</button>
                <button type="button" class="btn small ghost" onclick={() => (renaming = p)}>Rename</button>
                <button type="button" class="btn small ghost" onclick={() => changePreset(p, null)} title="To the Recycle Bin">Delete</button>
              {/if}
            </div>
          {/each}
        </div>
      {:else}
        <p class="note">None yet. Set the windows as you want them for some tables — the topper off, say — and Save as preset: a table plays with it when its settings choose it.</p>
      {/if}
    </section>

    <section class="block">
      <h2 class="section-title">The windows</h2>
      <div class="windows">
        {#each draft.windows as w (w.id)}
          {@const s = screenOf(w)}
          <div class="card" class:sel={selected === w.id} class:open={open[w.id]} style="--c: {KINDS[w.id].colour}">
            <button type="button" class="card-head" aria-expanded={!!open[w.id]} onclick={() => { open[w.id] = !open[w.id]; selected = w.id; }}>
              <span class="swatch"></span><b>{KINDS[w.id].label}</b><span class="what">{summary(w)}</span>
              <span class="chev"><Icon name="chevD" size={16} /></span>
            </button>
            {#if open[w.id]}
            <p class="what">{KINDS[w.id].what}</p>
            {#if w.id !== 'playfield'}
              <div class="row">
                <span class="name">Shows</span>
                <div class="seg" role="group" aria-label="How the {KINDS[w.id].label.toLowerCase()} shows">
                  <button type="button" aria-pressed={w.output === 'window'} onclick={() => setOutput(w, 'window')}>In its own window</button>
                  <button type="button" aria-pressed={w.output === 'inside'} onclick={() => setOutput(w, 'inside')}>Inside the table’s window</button>
                  <button type="button" aria-pressed={w.output === 'off'} onclick={() => setOutput(w, 'off')}>Not at all</button>
                </div>
              </div>
            {/if}
            {#if w.output === 'window' || w.id === 'playfield'}
              <div class="row">
                <span class="name">Screen</span>
                <select class="select" value={w.screen} onchange={(e) => setScreen(w, e.currentTarget.value)}>
                  {#each screens as o (o.name)}<option value={o.name}>{o.number}: {o.name}</option>{/each}
                </select>
                <label class="check"><input type="checkbox" checked={w.fullscreen} onchange={(e) => setFill(w, e.currentTarget.checked)} />Fill the screen</label>
              </div>
            {/if}
            {#if (w.output === 'window' && !w.fullscreen) || w.output === 'inside'}
              <div class="row nums">
                <span class="name">{w.output === 'inside' ? 'In the table’s window, in pixels' : `On ${s.name}, in pixels`}</span>
                {#each [['x', 'Across'], ['y', 'Down'], ['width', 'Width'], ['height', 'Height']] as [k, label] (k)}
                  <label class="num">{label}<input type="number" value={w[k]} onchange={(e) => setNumber(w, k, e.currentTarget.value)} /></label>
                {/each}
              </div>
            {/if}
            {#if w.output !== 'off'}
              <div class="row">
                <span class="name">Shape</span>
                <span class="shape">{shapeName(shapeOf(w))}{w.fullscreen ? ', its screen’s' : ''}</span>
                <label class="check"><input type="checkbox" checked={keeps(w.id)} onchange={(e) => setKeep(w.id, e.currentTarget.checked)} />Keep its shape</label>
                {#if reshaped(w)}
                  <button type="button" class="btn small" onclick={() => resetShape(w)}><Icon name="refresh" size={15} />Reset its shape to {shapeName(shapes[w.id])}</button>
                {/if}
              </div>
            {/if}
            {#if w.id === 'backglass'}
              <div class="row">
                <span class="name">The score display in it</span>
                <select class="select" value={pick(overlayChoices, draft.backglassDmd)} onchange={(e) => (draft.backglassDmd = overlayChoices[e.currentTarget.value].v)}>
                  {#each overlayChoices as c, i (c.v)}<option value={i}>{c.label}</option>{/each}
                </select>
              </div>
              <label class="check"><input type="checkbox" bind:checked={draft.showGrill} />Show the backglass picture’s speaker panel</label>
            {:else if w.id === 'scoreView'}
              <div class="row">
                <span class="name">Over a score panel</span>
                <select class="select" value={pick(overlayChoices, draft.scorePanelDmd)} onchange={(e) => (draft.scorePanelDmd = overlayChoices[e.currentTarget.value].v)}>
                  {#each overlayChoices as c, i (c.v)}<option value={i}>{c.label}</option>{/each}
                </select>
              </div>
              <p class="note">A table’s backglass file may hold a score panel, a picture for this window; the score display shows in its dark window.</p>
              <label class="check"><input type="checkbox" bind:checked={draft.floatingScore} />With no window of its own, float it over the table (ROM tables)</label>
            {/if}
            {/if}
          </div>
        {/each}
      </div>
    </section>

    <p class="file">
      Kept in {view.file}{view.exists ? '' : ' (made when you first save)'}.
      {#if view.exists}<button type="button" class="btn small ghost" onclick={showFile}><Icon name="folder" size={15} />Show the file</button>{/if}
    </p>
  {/if}
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 22px; max-width: 1240px; }
  header { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-end; justify-content: space-between; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 640px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .save { background: var(--accent-bg); border-color: var(--accent); }
  .soft { color: var(--faint); }
  .problem { padding: 14px 16px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; display: flex; gap: 12px; align-items: center; }
  .arranging {
    display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: 12px;
    background: #10263a; border: 1px solid #1f4a6e;
  }
  .arranging span { flex: 1; }
  .missing {
    display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-radius: 10px;
    background: #2a2414; border: 1px solid #5a4a20; color: var(--warn);
  }

  .map-wrap { display: flex; flex-direction: column; gap: 8px; }
  .map { position: relative; margin: 0 auto; user-select: none; touch-action: none; }
  .screen {
    position: absolute; border-radius: 6px; background: #0a0e15; border: 2px solid #2c3546;
    display: flex; flex-direction: column; justify-content: flex-end; padding: 6px 8px; overflow: hidden;
  }
  .screen.main { border-color: #3a4a64; }
  .tag {
    position: absolute; left: 6px; bottom: 6px; z-index: 6; pointer-events: none; max-width: calc(100% - 12px);
    display: flex; flex-direction: column; padding: 3px 8px; border-radius: 6px; background: rgba(8, 11, 16, 0.82);
  }
  .label { font-size: 13px; color: var(--fg-soft); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .label b { color: #fff; }
  .facts { font-size: 11px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .win {
    position: absolute; z-index: 3; border-radius: 4px; cursor: grab;
    background: color-mix(in srgb, var(--c) 22%, transparent); border: 2px solid var(--c);
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; overflow: hidden;
  }
  .win.inside { z-index: 4; border-style: dashed; }
  .win.sel { background: color-mix(in srgb, var(--c) 36%, transparent); box-shadow: 0 0 0 2px #0d1016, 0 0 0 4px var(--c); }
  .win:active { cursor: grabbing; }
  .wname { font-weight: 700; font-size: 13px; color: #fff; text-shadow: 0 1px 2px #000; white-space: nowrap; }
  .wsize { font-size: 11px; color: #e6ecf5; text-shadow: 0 1px 2px #000; white-space: nowrap; }
  .grip {
    position: absolute; right: 0; bottom: 0; width: 14px; height: 14px; cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, var(--c) 50%);
  }
  .hint { margin: 0; font-size: 12px; color: var(--faint); text-align: center; }

  .block { display: flex; flex-direction: column; gap: 10px; }
  .layouts { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 10px; }
  .layout {
    text-align: left; padding: 12px 14px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line);
    display: flex; flex-direction: column; gap: 4px; color: var(--fg);
  }
  .layout:hover { border-color: var(--accent); }
  .layout[aria-checked='true'] { border-color: var(--accent); background: var(--accent-bg); }
  .layout b { display: inline-flex; align-items: center; gap: 6px; }
  .layout span { color: var(--muted); font-size: 13px; }
  .layout[aria-checked='true'] span { color: var(--fg-soft); }

  .windows { display: grid; grid-template-columns: repeat(auto-fit, minmax(460px, 1fr)); gap: 12px; align-items: start; }
  .card { padding: 0; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); display: flex; flex-direction: column; gap: 10px; }
  .card.open { padding-bottom: 14px; }
  .card > :not(.card-head) { margin: 0 14px; }
  .card.sel { border-color: var(--c); }
  .card-head { display: flex; align-items: center; gap: 10px; background: none; border: 0; padding: 12px 14px; text-align: left; border-radius: 12px; color: var(--fg); }
  .card-head:hover { background: var(--hover); }
  .card-head b { font-size: 15px; }
  .card-head .what { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { display: inline-flex; color: var(--faint); transition: transform 0.15s; }
  .card.open .chev { transform: rotate(180deg); }
  .swatch { width: 12px; height: 12px; border-radius: 3px; background: var(--c); flex: 0 0 auto; }
  .what { color: var(--muted); font-size: 13px; }
  p.what { margin-top: -4px; }
  .row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .name { min-width: 150px; color: var(--fg-soft); font-size: 13px; }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-pressed='true'], .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg.wide { align-self: flex-start; }
  .check { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .check.line { font-size: 14px; }
  .shape { min-width: 92px; font-weight: 600; color: var(--fg); font-size: 13px; }
  .nums .num { display: inline-flex; flex-direction: column; gap: 2px; font-size: 11px; color: var(--faint); }
  .nums input { width: 84px; height: 30px; padding: 0 8px; border-radius: 6px; background: var(--control); border: 1px solid var(--control-line); color: var(--fg); }
  .name-it { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .name-it input { width: 240px; height: 36px; }
  .presets { display: flex; flex-direction: column; gap: 6px; }
  .preset { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .preset b { flex: 1 1 200px; font-size: 14px; }
  .note { margin: 0; color: var(--faint); font-size: 13px; max-width: 720px; }
  .file { margin: 0; color: var(--faint); font-size: 12px; display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
</style>
