<script>
  // Settings › Advanced: every setting VPX 5436 declares, VPinMAME's for every machine and ours, from the
  // settings catalog (`settings-catalog.json`, made by the settings map's tool from VPX's own
  // declarations), by category or searched, each with what it does, VPX's default and what ours does
  // with it; changed, or taken out for its default again. What the file holds that the catalog
  // hasn't, under "Others in the file", any added there by its section and name. A PIN2DMD key shows
  // only as set. Locked until unlocked, on a word; locked again when the page is left.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, notice } from './store.svelte.js';

  let locked = $state(true);
  let asking = $state(false);
  let catalog = $state(null);
  /** the file's settings as read: `section/name` in lower case → [section, name, value] */
  let file = $state(null);
  /** what's changed and not saved: `section/name` in lower case → { section, name, value } (null: taken out) */
  let edits = $state({});
  let cat = $state('windows');
  let query = $state('');
  let onlySet = $state(false);
  let busy = $state(false);
  let problem = $state('');
  let adding = $state({ section: '', name: '', value: '' });

  const id = (section, name) => `${section}/${name}`.toLowerCase();
  const secret = (section, name) => id(section, name) === 'plugin.vni/packey';
  const OTHERS = 'file';

  async function unlock() {
    asking = false;
    locked = false;
    await load();
  }

  // the app's test mode: settings/advanced/unlocked shows them unlocked, for its picture
  onMount(() => {
    // the catalog read at once, for the count the lock shows; the file only once unlocked
    import('./settings-catalog.json').then((m) => (catalog ??= m.default)).catch(() => {});
    if (app.testing && app.settingsTab === 'unlocked') unlock();
  });

  async function load() {
    problem = '';
    try {
      const [c, all] = await Promise.all([catalog ? Promise.resolve(catalog) : import('./settings-catalog.json').then((m) => m.default), invoke('all_settings')]);
      catalog = c;
      file = Object.fromEntries(all.map(([s, n, v]) => [id(s, n), [s, n, v]]));
      edits = {};
    } catch (e) {
      problem = String(e);
    }
  }

  const known = $derived(catalog ? new Set(catalog.settings.map((e) => id(e.s, e.k))) : new Set());
  // what the file holds that the catalog hasn't: the input mappings, a table option, older names
  const others = $derived(
    file
      ? [
          ...Object.entries(file)
            .filter(([k]) => !known.has(k))
            .map(([, [s, n]]) => ({ s, k: n, l: n, d: '', t: 'raw', c: OTHERS })),
          ...Object.values(edits)
            .filter((e) => e.added && !file[id(e.section, e.name)])
            .map((e) => ({ s: e.section, k: e.name, l: e.name, d: '', t: 'raw', c: OTHERS })),
        ]
      : [],
  );
  const everything = $derived(catalog ? [...catalog.settings, ...others] : []);

  /** A setting's value now: changed, else the file's, else none (its default). */
  function now(e) {
    const k = id(e.s, e.k);
    if (k in edits) return edits[k].value;
    return file?.[k]?.[2] ?? null;
  }
  const isSet = (e) => now(e) !== null;
  const changed = (e) => id(e.s, e.k) in edits;

  function set(e, value) {
    const k = id(e.s, e.k);
    const was = file?.[k]?.[2] ?? null;
    if (value === was) {
      const { [k]: _, ...rest } = edits;
      edits = rest;
    } else {
      edits = { ...edits, [k]: { section: e.s, name: e.k, value, added: edits[k]?.added } };
    }
  }

  const changes = $derived(Object.values(edits).filter((x) => !secret(x.section, x.name)));

  // the categories, with how many each holds and how many of those are set
  const cats = $derived.by(() => {
    if (!catalog) return [];
    const out = catalog.categories.map((c) => ({ ...c, n: 0, set: 0 }));
    out.push({ id: OTHERS, label: 'Others in the file', note: 'In the settings file but not among VPX 5436’s or ours: the controls’ mappings, older names, ones added by hand. Kept as they are.', n: 0, set: 0 });
    for (const e of everything) {
      const c = out.find((x) => x.id === e.c);
      if (!c) continue;
      c.n += 1;
      if (isSet(e)) c.set += 1;
    }
    return out;
  });
  const current = $derived(cats.find((c) => c.id === cat) || cats[0]);

  // what shows: the category's, or every category's when searching
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    let list = q ? everything.filter((e) => `${e.l} ${e.s} ${e.k} ${e.d} ${e.why || ''} ${e.g || ''}`.toLowerCase().includes(q)) : everything.filter((e) => e.c === cat);
    if (onlySet) list = list.filter(isSet);
    return list.slice(0, 600);
  });
  // in groups, as VPX heads them (a plugin's under its name)
  const groups = $derived.by(() => {
    const out = [];
    for (const e of shown) {
      const g = query.trim() ? cats.find((c) => c.id === e.c)?.label : e.g || '';
      if (!out.length || out[out.length - 1].name !== g) out.push({ name: g, rows: [] });
      out[out.length - 1].rows.push(e);
    }
    return out;
  });

  const STATUS = {
    works: ['Works in VPI', 'good'],
    later: ['Not in VPI yet', 'later'],
    'not-ours': ['Not VPI’s', 'not'],
    'own-file': ['Its own file', 'not'],
  };

  const shownNumber = (v) => (typeof v === 'number' ? String(Math.round(v * 1e4) / 1e4) : v);

  /** A setting's default, in words. */
  function defaultText(e) {
    if (e.def === undefined) return e.t === 'raw' ? '' : 'its own';
    if (e.t === 'bool') return e.def ? 'on' : 'off';
    if (e.t === 'enum') return e.v?.[e.def - (e.v0 || 0)] ?? String(e.def);
    if (e.t === 'choice') return e.opts?.find(([v]) => v === String(e.def))?.[1] ?? String(e.def);
    if (e.t === 'string') return e.def === '' ? 'empty' : `“${e.def}”`;
    return shownNumber(e.def);
  }

  const on = (v) => ['1', 'true', 'yes', 'on'].includes(String(v).trim().toLowerCase());
  const boolNow = (e) => (isSet(e) ? on(now(e)) : !!e.def);

  function add() {
    const { section, name, value } = adding;
    if (!section.trim() || !name.trim()) return;
    const e = { s: section.trim(), k: name.trim() };
    const k = id(e.s, e.k);
    edits = { ...edits, [k]: { section: e.s, name: e.k, value, added: !file[k] } };
    adding = { section: '', name: '', value: '' };
    cat = OTHERS;
  }

  async function save() {
    busy = true;
    try {
      await invoke('save_any_settings', { sets: changes.map(({ section, name, value }) => ({ section, name, value })) });
      notice('Saved. Tables started from now on play this way.', { kind: 'done' });
      await load();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }
</script>

<div class="page" data-loading={locked || file || problem ? undefined : ''}>
  <header>
    <div class="title">
      <h1 class="display">Advanced</h1>
      <p>Every setting VPX 5436 has, VPinMAME’s for every machine and ours, by category: what each does, its default, and what VPI does with it. The other pages change the same settings.</p>
    </div>
    {#if !locked && file}
      <div class="actions">
        <button type="button" class="btn ghost" disabled={!changes.length || busy} onclick={() => (edits = {})}>Undo changes</button>
        <button type="button" class="btn save" disabled={!changes.length || busy} onclick={save}><Icon name="check" />{changes.length ? `Save ${changes.length === 1 ? '1 change' : `${changes.length} changes`}` : 'Save'}</button>
        <button type="button" class="btn ghost" disabled={busy} onclick={() => { if (!changes.length) locked = true; }} title={changes.length ? 'Save or undo the changes first' : 'Lock them again'}><Icon name="lock" size={16} />Lock</button>
      </div>
    {/if}
  </header>

  {#if locked}
    <div class="lock">
      <span class="padlock"><Icon name="lock" size={22} /></span>
      <div class="words">
        <b>The advanced settings are locked</b>
        <p>Here are all {catalog ? `${catalog.settings.length.toLocaleString('en-US')} ` : ''}settings, VPX’s names and all. A wrong value can make tables look, sound or play wrongly, so they’re locked against a slip: unlock them to change one. The other pages hold the ones most people need.</p>
        {#if asking}
          <div class="ask" role="alertdialog" aria-labelledby="unlock-q">
            <p id="unlock-q">Unlock the advanced settings? A change goes into the settings file every table plays with once you save it. Each can be taken out again for its default, and the file before each save is kept as Settings.previous.ini.</p>
            <div class="buttons">
              <button type="button" class="btn save" onclick={unlock}><Icon name="unlock" size={16} />Unlock them</button>
              <button type="button" class="btn ghost" onclick={() => (asking = false)}>Cancel</button>
            </div>
          </div>
        {:else}
          <button type="button" class="btn" onclick={() => (asking = true)}><Icon name="unlock" size={16} />Unlock…</button>
        {/if}
      </div>
    </div>
  {:else if problem}
    <div class="problem">{problem}</div>
  {:else if !file || !catalog}
    <p class="soft">Reading the settings…</p>
  {:else}
    <div class="body">
      <nav class="cats" aria-label="Categories">
        {#each cats as c (c.id)}
          {#if c.n}
            <button type="button" class="cat" aria-pressed={!query.trim() && cat === c.id} onclick={() => { cat = c.id; query = ''; }}>
              <span class="label">{c.label}</span>
              <span class="n">{c.set ? `${c.set} set · ` : ''}{c.n}</span>
            </button>
          {/if}
        {/each}
      </nav>
      <div class="list">
        <div class="tools">
          <label class="search"><Icon name="search" /><input type="search" bind:value={query} placeholder="Search every category: a name, a word of what it does" aria-label="Search the settings" /></label>
          <label class="own"><input type="checkbox" bind:checked={onlySet} />Only those set in the file</label>
        </div>
        {#if !query.trim() && current}
          <p class="catnote">{current.note}</p>
        {/if}
        {#each groups as g, gi (gi)}
          <section class="group">
            {#if g.name}<h2>{g.name}</h2>{/if}
            {#each g.rows as e (e.s + '/' + e.k)}
              {@const v = now(e)}
              {@const st = STATUS[e.st]}
              <div class="row" class:set={v !== null} class:changed={changed(e)}>
                <div class="who">
                  <span class="label">{e.l}</span>
                  {#if st}<span class="pill {st[1]}" title={e.why || ''}>{st[0]}</span>{/if}
                  <span class="key">[{e.s === 'Machine.default' ? 'Machine settings.ini › default' : e.s}] {e.k}</span>
                </div>
                <div class="ctl">
                  {#if secret(e.s, e.k)}
                    <span class="soft">{v ? 'Set, never shown' : 'Not set'}</span>
                  {:else if e.t === 'bool'}
                    <label class="switch"><input type="checkbox" checked={boolNow(e)} onchange={(x) => set(e, x.currentTarget.checked ? '1' : '0')} /><span>{boolNow(e) ? 'On' : 'Off'}</span></label>
                  {:else if e.t === 'enum'}
                    <select class="select" value={v ?? ''} onchange={(x) => set(e, x.currentTarget.value === '' ? null : x.currentTarget.value)}>
                      <option value="">Its default ({defaultText(e)})</option>
                      {#each e.v as label, i (i)}<option value={String(i + (e.v0 || 0))}>{label}</option>{/each}
                    </select>
                  {:else if e.t === 'choice'}
                    <select class="select" value={v ?? ''} onchange={(x) => set(e, x.currentTarget.value === '' ? null : x.currentTarget.value)}>
                      {#each e.opts as [value, label] (value)}<option {value}>{label}</option>{/each}
                    </select>
                  {:else if e.t === 'int' || e.t === 'float'}
                    <input class="select num" type="number" min={e.min} max={e.max} step={e.step || (e.t === 'int' ? 1 : 'any')} value={v ?? ''} placeholder={e.def !== undefined ? shownNumber(e.def) : ''} onchange={(x) => set(e, x.currentTarget.value === '' ? null : x.currentTarget.value)} />
                    {#if e.min !== undefined && e.max !== undefined}<span class="soft">{shownNumber(e.min)} to {shownNumber(e.max)}</span>{/if}
                  {:else}
                    <input class="select text" value={v ?? ''} placeholder={e.def ? e.def : ''} onchange={(x) => set(e, x.currentTarget.value)} />
                  {/if}
                  {#if e.t !== 'raw'}<span class="def">Default: {defaultText(e)}</span>{/if}
                  {#if v !== null && !secret(e.s, e.k)}
                    <button type="button" class="btn small ghost" onclick={() => set(e, null)} title="Out of the file: its default again">Take it out</button>
                  {/if}
                </div>
                {#if e.d || e.why}
                  <p class="what">{e.d}{#if e.why}<span class="ours">{e.d ? ' · ' : ''}In VPI: {e.why}</span>{/if}</p>
                {/if}
              </div>
            {/each}
          </section>
        {:else}
          <p class="soft">{query.trim() ? 'None matches.' : onlySet ? 'None of these is set in the file: each plays with its default.' : 'None here.'}</p>
        {/each}
        {#if shown.length === 600}<p class="soft">The first 600 shown: search for fewer.</p>{/if}
        {#if !query.trim() && cat === OTHERS}
          <form class="add" onsubmit={(x) => { x.preventDefault(); add(); }}>
            <span class="soft">Add one by name:</span>
            <input class="select" bind:value={adding.section} placeholder="Section (Player)" aria-label="Its section" />
            <input class="select" bind:value={adding.name} placeholder="Name (MaxFramerate)" aria-label="Its name" />
            <input class="select" bind:value={adding.value} placeholder="Value" aria-label="Its value" />
            <button type="submit" class="btn small"><Icon name="plus" size={14} />Add</button>
          </form>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 18px; max-width: 1400px; }
  header { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-end; justify-content: space-between; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 720px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .save { background: var(--accent-bg); border-color: var(--accent); }
  .soft { margin: 0; color: var(--faint); font-size: 13px; }
  .problem { padding: 14px 16px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; }
  .lock { display: flex; gap: 18px; padding: 22px 24px; border-radius: 14px; background: var(--card); border: 1px solid var(--card-line); max-width: 860px; }
  .padlock { flex: 0 0 auto; width: 46px; height: 46px; border-radius: 50%; display: flex; align-items: center; justify-content: center; background: #2a2414; color: #ffd27a; }
  .words { display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .words b { font-size: 17px; }
  .words p { margin: 0; color: var(--fg-soft); font-size: 14px; line-height: 1.5; }
  .ask { display: flex; flex-direction: column; gap: 12px; padding: 14px 16px; border-radius: 12px; background: #2a2414; border: 1px solid #5a4a20; }
  .ask p { color: #ffe2b0; }
  .buttons { display: flex; gap: 8px; }
  .body { display: flex; gap: 24px; align-items: flex-start; }
  .cats { flex: 0 0 250px; position: sticky; top: 76px; display: flex; flex-direction: column; gap: 2px; max-height: calc(100vh - 100px); overflow-y: auto; }
  .cat { display: flex; align-items: baseline; gap: 8px; width: 100%; padding: 8px 10px; border: 0; border-radius: 8px; background: transparent; color: var(--fg-soft); font-size: 14px; text-align: left; }
  .cat:hover { background: var(--hover); }
  .cat[aria-pressed='true'] { background: var(--selected); color: #fff; font-weight: 600; }
  .cat .label { flex: 1; min-width: 0; }
  .cat .n { font-size: 12px; color: var(--faint); font-weight: 400; white-space: nowrap; }
  .list { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 14px; }
  .tools { display: flex; flex-wrap: wrap; gap: 12px 18px; align-items: center; }
  .search { flex: 1 1 380px; display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 40px; max-width: 620px; border-radius: 10px; background: var(--bg); border: 1px solid #2a3242; color: var(--faint); }
  .search input { flex: 1; min-width: 0; background: transparent; border: 0; outline: none; color: var(--fg); font-size: 14px; }
  .own, .switch { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .own input, .switch input { accent-color: var(--accent); }
  .catnote { margin: 0; color: var(--muted); font-size: 14px; max-width: 820px; }
  .group { display: flex; flex-direction: column; padding-top: 14px; border-top: 1px solid var(--line); }
  .group h2 { margin: 0 0 6px; font-size: 16px; font-weight: 700; color: var(--fg); }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; padding: 10px 10px; border-top: 1px solid var(--line-soft); border-radius: 6px; }
  .group h2 + .row { border-top: 0; }
  .row.set { background: rgba(79, 177, 255, 0.04); }
  .row.changed { background: rgba(79, 177, 255, 0.1); }
  .who { flex: 0 0 330px; min-width: 0; display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 8px; }
  .who .label { font-size: 14px; color: var(--fg); }
  .key { flex: 1 1 100%; font: 11px ui-monospace, Consolas, monospace; color: #6f7889; overflow-wrap: anywhere; }
  .ctl { flex: 1 1 360px; min-width: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; }
  .ctl .select { height: 32px; }
  .num { width: 130px; }
  .text { flex: 1 1 260px; min-width: 0; }
  .def { font-size: 12px; color: var(--faint); }
  .what { flex: 1 1 100%; margin: 0 0 0 346px; color: var(--faint); font-size: 12.5px; line-height: 1.45; }
  .ours { color: #8d97a8; }
  .pill { padding: 1px 8px; border-radius: 999px; font-size: 11px; font-weight: 600; white-space: nowrap; }
  .pill.good { background: #143024; color: var(--good); }
  .pill.later { background: #2a2414; color: #ffd27a; }
  .pill.not { background: #1d2230; color: var(--faint); }
  .add { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding-top: 10px; border-top: 1px solid var(--line); }
  .add input { height: 32px; width: 200px; }
  @media (max-width: 1100px) { .what { margin-left: 0; } .who { flex-basis: 100%; } }
</style>
