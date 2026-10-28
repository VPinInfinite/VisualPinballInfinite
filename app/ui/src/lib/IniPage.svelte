<script>
  // A settings page made of plain settings (Picture, General): its parts, each under a thin line
  // with its name (as Settings › Library), each setting a row — its name,
  // its control, what it does and VPX's own name for it — read from the settings file, a setting
  // left unset showing the player's own default; Save writes only what changed, as every settings
  // page does (`ini_page.rs`). A page lists its rows as data, so moving a setting is moving a line:
  //   choice  — options, each the settings it sets ({ 'Player/MSAASamples': '1' }; null: taken out;
  //             a list: any of them counts as it, the first written), or `read`/`write` of its own
  //   toggle  — `key`, on unless set when `def`, `invert` for VPX's Force…Off switches
  //   slider  — `key`, `min`, `max`, `step`, `def`; `unset` names the player's own (the table's)
  //   number, text — `key`; checks — `key`, its `options`, all of them when unset
  //   custom  — a snippet of the page's own, given `get` and `set`, its `keys` read with the rest
  // A row may show only `when` the others say so, and one still to build is `later`.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  // `kept`: where its settings are kept, said in place of the settings file's place
  let { title, lead, parts, custom = {}, after = null, kept = null } = $props();

  let view = $state(null);
  let draft = $state(null);
  let saved = $state('');
  let busy = $state(false);
  let problem = $state('');

  const changed = $derived(!!draft && JSON.stringify(draft) !== saved);

  // every setting the rows read, `Section/Name`
  const keys = $derived.by(() => {
    const all = new Set();
    for (const p of parts) {
      for (const r of p.rows) {
        if (r.key) all.add(r.key);
        for (const k of r.keys || []) all.add(k);
        for (const o of r.options || []) if (o.set) Object.keys(o.set).forEach((k) => all.add(k));
      }
    }
    return [...all];
  });

  async function load() {
    problem = '';
    try {
      const list = keys;
      const v = await invoke('ini_values', { keys: list.map((k) => ({ section: k.split('/')[0], name: k.split('/').slice(1).join('/') })) });
      view = v;
      draft = Object.fromEntries(list.map((k, i) => [k, v.values[i]]));
      saved = JSON.stringify(draft);
    } catch (e) {
      problem = String(e);
    }
  }
  onMount(load);

  async function save() {
    busy = true;
    try {
      const before = JSON.parse(saved);
      const sets = Object.keys(draft)
        .filter((k) => draft[k] !== before[k])
        .map((k) => ({ section: k.split('/')[0], name: k.split('/').slice(1).join('/'), value: draft[k] }));
      await invoke('save_ini_values', { sets });
      saved = JSON.stringify(draft);
      notice('Saved. Tables started from now on play this way.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  function undo() {
    draft = JSON.parse(saved);
  }

  const get = (k) => draft?.[k] ?? null;
  const set = (k, v) => (draft[k] = v === '' || v === undefined ? null : v);
  const on = (v) => v != null && ['1', 'true', 'yes', 'on'].includes(String(v).trim().toLowerCase());
  const num = (v) => (v == null || v === '' || Number.isNaN(Number(v)) ? null : Number(v));
  // a value as the file has it, matched against what an option says: numbers by number
  const same = (have, want) => (want === null ? have === null : have !== null && (String(have) === String(want) || (num(have) !== null && num(have) === num(want))));
  const matches = (o) => Object.entries(o.set).every(([k, want]) => (Array.isArray(want) ? want : [want]).some((w) => same(get(k), w)));

  function chosen(r) {
    if (r.read) return r.read(get);
    return r.options.findIndex(matches);
  }
  function choose(r, i) {
    if (r.write) return r.write(i, set);
    for (const [k, want] of Object.entries(r.options[i].set)) set(k, Array.isArray(want) ? want[0] : want);
  }
  // shown on or off: unset, as the player's own default (`def`); set, as the file says, a Force…Off
  // switch the other way round
  const toggled = (r) => {
    const v = get(r.key);
    return v === null ? !!r.def : r.invert ? !on(v) : on(v);
  };
  const toggle = (r, value) => set(r.key, (r.invert ? !value : value) ? '1' : '0');
  const checked = (r, id) => {
    const v = get(r.key);
    return v === null ? true : v.split(',').map((x) => x.trim().toLowerCase()).includes(id);
  };
  function check(r, id, value) {
    const now = r.options.map(([o]) => o).filter((o) => (o === id ? value : checked(r, o)));
    set(r.key, now.length === r.options.length ? null : now.join(','));
  }
  const fmt = (r, v) => (r.fmt ? r.fmt(v) : String(v));
</script>

<div class="page" data-loading={draft || problem ? undefined : ''}>
  <header>
    <div class="title">
      <h1 class="display">{title}</h1>
      <p>{lead}</p>
    </div>
    {#if draft}
      <div class="actions">
        <button type="button" class="btn ghost" onclick={undo} disabled={!changed || busy}>Undo changes</button>
        <button type="button" class="btn save" onclick={save} disabled={!changed || busy}><Icon name="check" />Save</button>
      </div>
    {/if}
  </header>

  {#if problem}
    <div class="problem">{problem} <button type="button" class="btn small" onclick={load}>Try again</button></div>
  {:else if !draft}
    <p class="soft">Reading the settings…</p>
  {:else}
    {#each parts as part (part.name)}
      <section class="part" aria-label={part.name}>
        <h2 class="partname">{part.name}</h2>
        {#if part.note}<p class="note">{part.note}</p>{/if}
        <div class="rows">
          {#each part.rows.filter((r) => !r.when || r.when(get)) as r (r.label)}
            <div class="row" class:later={r.later}>
              <span class="name">{r.label}{#if r.later}<span class="pill tobuild">To build</span>{/if}</span>
              <span class="ctl">
                {#if r.kind === 'choice'}
                  {@const at = chosen(r)}
                  {#if r.select}
                    <select class="select long" value={at} disabled={r.later} onchange={(e) => choose(r, Number(e.currentTarget.value))} aria-label={r.label}>
                      {#if at < 0}<option value={-1}>As the settings file has it</option>{/if}
                      {#each r.options as o, i (i)}<option value={i} disabled={o.later}>{o.label}{o.later ? ' (to build)' : ''}</option>{/each}
                    </select>
                  {:else}
                    <span class="seg" role="radiogroup" aria-label={r.label}>
                      {#each r.options as o, i (i)}<button type="button" role="radio" aria-checked={at === i} disabled={r.later || o.later} title={o.later ? 'To build' : undefined} onclick={() => choose(r, i)}>{o.label}</button>{/each}
                    </span>
                  {/if}
                {:else if r.kind === 'toggle'}
                  <label class="switch"><input type="checkbox" checked={toggled(r)} disabled={r.later} onchange={(e) => toggle(r, e.currentTarget.checked)} /><span>{toggled(r) ? (r.onLabel ?? 'On') : (r.offLabel ?? 'Off')}</span></label>
                {:else if r.kind === 'slider'}
                  {@const v = num(get(r.key))}
                  {#if r.unset}
                    <label class="own"><input type="checkbox" checked={v === null} onchange={(e) => set(r.key, e.currentTarget.checked ? null : String(r.def))} />{r.unset}</label>
                  {/if}
                  <input type="range" min={r.min} max={r.max} step={r.step} value={v ?? r.def} disabled={r.unset && v === null} oninput={(e) => set(r.key, e.currentTarget.value)} aria-label={r.label} />
                  <span class="val">{v === null ? (r.unset ? '' : fmt(r, r.def)) : fmt(r, v)}</span>
                {:else if r.kind === 'number'}
                  <input class="select num" type="number" min={r.min} max={r.max} step={r.step} placeholder={r.placeholder ?? ''} value={get(r.key) ?? ''} onchange={(e) => set(r.key, e.currentTarget.value)} aria-label={r.label} />
                  {#if r.unit}<span class="val">{r.unit}</span>{/if}
                {:else if r.kind === 'text'}
                  <input class="select path" type="text" placeholder={r.placeholder ?? ''} value={get(r.key) ?? ''} onchange={(e) => set(r.key, e.currentTarget.value.trim())} aria-label={r.label} />
                {:else if r.kind === 'checks'}
                  {#each r.options as [id, label] (id)}<label class="own"><input type="checkbox" checked={checked(r, id)} onchange={(e) => check(r, id, e.currentTarget.checked)} />{label}</label>{/each}
                {:else if r.kind === 'custom'}
                  {@render custom[r.name]?.(get, set)}
                {:else if r.kind === 'info'}
                  <span class="val">{r.text}</span>
                {/if}
              </span>
              {#if r.what || r.vpx}<span class="what">{r.what ?? ''}{#if r.vpx}<span class="vpx">{r.vpx}</span>{/if}</span>{/if}
            </div>
          {/each}
        </div>
      </section>
    {/each}
    {@render after?.()}
    <p class="file">{kept ?? `Kept in ${view.file}${view.exists ? '' : ' (made when you first save)'}.`} A table’s own settings always win over these.</p>
  {/if}
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 26px; max-width: 1240px; }
  header { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-end; justify-content: space-between; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .save { background: var(--accent-bg); border-color: var(--accent); }
  .soft { color: var(--faint); }
  .problem { padding: 14px 16px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; display: flex; gap: 12px; align-items: center; }
  /* each part under a thin line, with its name (as the side panel's divider) */
  .part { display: flex; flex-direction: column; gap: 10px; padding-top: 22px; border-top: 1px solid var(--line); }
  .partname { margin: 0 0 4px; font-size: 20px; font-weight: 700; color: var(--fg); }
  .note { margin: -4px 0 0; color: var(--faint); font-size: 13px; max-width: 760px; }
  .rows { display: flex; flex-direction: column; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; padding: 12px 0; border-top: 1px solid var(--line-soft); }
  .row:first-child { border-top: 0; }
  .row.later { opacity: 0.7; }
  .name { flex: 0 0 240px; font-size: 14px; color: var(--fg); display: inline-flex; align-items: center; gap: 8px; }
  .ctl { flex: 1 1 320px; min-width: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; }
  .what { flex: 1 1 100%; margin-left: 254px; color: var(--faint); font-size: 13px; }
  .vpx { margin-left: 8px; font: 11px ui-monospace, Consolas, monospace; color: #6f7889; }
  .seg { display: inline-flex; flex-wrap: wrap; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 34px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg button:disabled { color: var(--faint); }
  .long { min-width: 320px; }
  .num { width: 140px; }
  .path { flex: 1 1 360px; min-width: 0; font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
  input[type='range'] { width: 260px; accent-color: var(--accent); }
  .val { font-size: 13px; color: var(--fg-soft); min-width: 50px; }
  .own, .switch { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .switch input, .own input { accent-color: var(--accent); }
  .pill { padding: 2px 8px; border-radius: 999px; font-size: 11px; font-weight: 600; }
  .tobuild { background: #2a2414; color: #ffd27a; }
  .file { margin: 0; color: var(--faint); font-size: 13px; }
  @media (max-width: 900px) { .what { margin-left: 0; } }
</style>
