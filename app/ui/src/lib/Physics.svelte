<script>
  // A table's physics: each value as the table's author set it beside the one that plays, changed
  // and saved beside the table as its own physics file (<table>.vpp), which VPX reads too — the
  // table file stays as it is — or put back to the author's; presets to start from, saved from a
  // table's or brought in from a file.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, onclose } = $props();

  // each value in plain words, in the files' order: the table's eight, then its flippers' ten
  const LABELS = [
    ['Gravity', 'How hard the ball is pulled down the playfield'],
    ['Friction', 'Of the playfield and the table’s walls'],
    ['Bounciness', 'Of the playfield and the table’s walls'],
    ['Bounciness falloff', 'How much less it bounces the harder it hits'],
    ['Scatter', 'A little chance in the ball’s path off the playfield, in degrees'],
    ['Parts’ scatter', 'The same off parts that have none of their own'],
    ['Slope, easiest', 'The playfield’s slope in degrees at the easiest difficulty'],
    ['Slope, hardest', 'At the hardest; the table’s difficulty sets where between'],
    ['Mass', ''],
    ['Strength', ''],
    ['Bounciness', ''],
    ['Scatter', 'In degrees'],
    ['End-of-stroke hold', 'How firmly a flipper holds at the end of its stroke'],
    ['End-of-stroke angle', 'How many degrees before the end the hold begins'],
    ['Return', 'How fast a flipper falls back'],
    ['Bounciness falloff', ''],
    ['Friction', ''],
    ['Coil ramp-up', 'How fast the coil comes to full strength'],
  ];

  let view = $state(null);
  let problem = $state('');
  let values = $state([]);
  let busy = $state(false);
  let presetName = $state('');
  let picked = $state('');
  let naming = $state(false);

  // what plays now: its own file, else the folder's, else the author's
  const playing = $derived(view ? view.file ?? view.shared ?? null : null);
  const fill = (file) => view.own.map((v, i) => file?.values[i] ?? v);
  const changed = $derived(view?.own ? values.some((v, i) => Math.abs(v - fill(playing)[i]) > 1e-6) : false);

  async function load() {
    try {
      view = await invoke('physics_view', { id: t.id });
      if (view.own) values = fill(playing);
    } catch (e) {
      problem = String(e);
    }
  }
  onMount(load);

  const shown = (v) => (Number.isInteger(v) ? v.toFixed(1) : String(Math.round(v * 1e6) / 1e6));

  async function save() {
    busy = true;
    try {
      await invoke('physics_save', { id: t.id, values: $state.snapshot(values) });
      notice(`${t.short} plays these physics from its next game.`, { kind: 'done' });
      onclose(true);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  async function back() {
    busy = true;
    try {
      await invoke('physics_back', { id: t.id });
      notice(`${t.short} plays its own physics again.`, { kind: 'done' });
      onclose(true);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  // a preset's values put in to look at; saved only with Save
  function usePreset() {
    const p = view.presets.find((x) => x.path === picked);
    if (p) values = fill(p);
  }

  async function savePreset() {
    try {
      await invoke('physics_preset_save', { name: presetName, values: $state.snapshot(values) });
      notice(`Kept as the preset “${presetName.trim()}”.`, { kind: 'done' });
      naming = false;
      presetName = '';
      await load();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function importPreset() {
    try {
      if (await invoke('physics_preset_import')) await load();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  const presetLabel = (p) => p.name || p.path.split('\\').pop().replace(/\.vpp$/i, '');

  function key(e) {
    if (e.key === 'Escape' && !busy) onclose(false);
  }
</script>

<svelte:window onkeydown={key} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && !busy && onclose(false)}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ph-title" data-loading={view || problem ? undefined : ''}>
    <header>
      <h2 id="ph-title">Physics for {t.short}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={() => onclose(false)} disabled={busy}><Icon name="x" /></button>
    </header>

    {#if problem}
      <p class="problem">{problem}</p>
    {:else if !view}
      <p class="soft">Reading the table…</p>
    {:else if !view.own}
      <p class="soft">This table has no flippers, so it has no physics of its own to change here.</p>
    {:else}
      <p class="soft">
        {#if view.file}It plays its own physics file, {view.file.path.split('\\').pop()}, beside it.
        {:else if view.shared}It plays autovpp.vpp, the physics file for every table in its folder without one of its own.
        {:else}It plays the physics its author set.{/if}
        Changes are kept beside it as its own physics file, which VPX reads too; the table file stays as it is.
      </p>
      {#if view.set}
        <p class="warn"><Icon name="alert" size={16} />The table plays VPX’s physics set {view.set} from your settings, over its own and any file.</p>
      {/if}

      <div class="grid">
        {#each [['The table', 0, 8], ['Its flippers', 8, 18]] as [title, from, to] (title)}
          <h3 class="section-title">{title}</h3>
          <div class="head"><span></span><span>Its author’s</span><span>Playing</span></div>
          {#each values.slice(from, to) as _, k (k)}
            {@const i = from + k}
            <div class="row" class:diff={Math.abs(values[i] - view.own[i]) > 1e-6}>
              <span class="name" title={LABELS[i][1]}>{LABELS[i][0]}{#if LABELS[i][1]}<small>{LABELS[i][1]}</small>{/if}</span>
              <span class="own">{shown(view.own[i])}</span>
              <input class="select" type="number" step="any" bind:value={values[i]} aria-label={LABELS[i][0]} />
            </div>
          {/each}
        {/each}
      </div>

      <div class="presets">
        <span>Presets</span>
        <select class="select" bind:value={picked} aria-label="A preset">
          <option value="">{view.presets.length ? 'Choose one…' : 'None kept yet'}</option>
          {#each view.presets as p (p.path)}<option value={p.path}>{presetLabel(p)}</option>{/each}
        </select>
        <button type="button" class="btn small" disabled={!picked} onclick={usePreset}>Use</button>
        {#if naming}
          <input class="select" type="text" bind:value={presetName} placeholder="Its name" aria-label="The preset’s name" />
          <button type="button" class="btn small" disabled={!presetName.trim()} onclick={savePreset}>Keep</button>
        {:else}
          <button type="button" class="btn small ghost" onclick={() => (naming = true)}>Keep these as a preset…</button>
        {/if}
        <button type="button" class="btn small ghost" onclick={importPreset}>Bring one in…</button>
      </div>

      <footer>
        {#if view.file}
          <button type="button" class="btn ghost" disabled={busy} onclick={back}>Back to its author’s</button>
        {/if}
        <button type="button" class="btn ghost" disabled={busy} onclick={() => (values = [...view.own])}>Fill in its author’s</button>
        <button type="button" class="btn ghost" disabled={busy} onclick={() => onclose(false)}>Cancel</button>
        <button type="button" class="btn go" disabled={busy || !changed} onclick={save}><Icon name="check" size={16} />Save beside it</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(760px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  h3 { margin: 8px 0 0; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .warn { display: flex; align-items: center; gap: 8px; margin: 0; color: var(--warn); font-size: 13px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .grid { display: flex; flex-direction: column; gap: 4px; }
  .head, .row { display: grid; grid-template-columns: minmax(0, 1fr) 110px 140px; gap: 12px; align-items: center; }
  .head { color: var(--faint); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; }
  .row { padding: 4px 0; border-top: 1px solid var(--line-soft); font-size: 14px; }
  .row.diff .name { color: var(--accent); }
  .name { display: flex; flex-direction: column; min-width: 0; }
  .name small { color: var(--faint); font-size: 12px; }
  .own { color: var(--fg-soft); font-variant-numeric: tabular-nums; }
  .row input { height: 32px; font-variant-numeric: tabular-nums; }
  .presets { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; font-size: 13px; color: var(--fg-soft); }
  .presets select { height: 32px; max-width: 220px; }
  .presets input { height: 32px; width: 180px; }
  footer { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  footer .btn:first-child { margin-right: auto; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
