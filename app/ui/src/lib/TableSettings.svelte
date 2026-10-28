<script>
  // A table's own settings: the options its script offers, as VPX's in-game
  // options menu shows them; its own choices over the user's usual — its view, volumes, VPX's
  // general table settings (tone mapper, exposure, scene lighting, difficulty), sound and colour
  // packs — kept in the table's own .ini beside it, as VPX keeps them; which physics it plays with;
  // and, for a table that drives PinMAME, the machine's settings for its game, kept as VPinMAME
  // keeps them for each game.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import Physics from './Physics.svelte';
  import { notice, showFile, follow } from './store.svelte.js';

  let { t, onclose, onfindvps, onscreens } = $props();

  // a rename to VPS's name put back: the files, PinUP Popper's game, and what's ours
  let putting = $state(false);
  async function putBack() {
    putting = true;
    try {
      const [was, short] = [t.id, t.short];
      const id = await invoke('put_names_back', { id: was });
      follow(was, id);
      notice(`${short} has its own names back, for every program.`, { kind: 'done' });
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    putting = false;
  }

  // the screen presets kept, for the table to play with one
  let presets = $state([]);

  let view = $state(null);
  let options = $state({});
  let mine = $state(null);
  let machine = $state(null);
  // whether it may reach outside its own files: its own word, kept in your settings
  let outside = $state({ reach: null });
  let busy = $state(false);
  let problem = $state('');
  let physicsOpen = $state(false);

  // the physics window closed: what plays now, read again where it saved or put back
  async function physicsClosed(saved) {
    physicsOpen = false;
    if (!saved) return;
    const v = await invoke('table_settings', { id: t.id }).catch(() => null);
    if (v) view.physics = v.physics;
  }

  onMount(async () => {
    try {
      const v = await invoke('table_settings', { id: t.id });
      view = v;
      options = Object.fromEntries(v.options.map((o) => [o.id, o.value ?? null]));
      mine = structuredClone(v.forTable);
      machine = v.machine ? structuredClone(v.machine.own) : null;
      outside = { reach: v.reachOutside ?? null };
      presets = await invoke('screen_presets').catch(() => []);
    } catch (e) {
      problem = String(e);
    }
  });

  const isToggle = (o) => o.names?.length === 2;
  const current = (o) => options[o.id] ?? o.default;
  const shown = (o, v) => (o.names ? o.names[Math.round(v - o.min)] ?? v : o.unit === 1 ? `${Math.round(v * 100)}%` : Number.isInteger(o.step) ? String(Math.round(v)) : v.toFixed(2));

  const VIEWS = ['Desktop', 'Cabinet', 'Full single screen'];
  const MAPPERS = ['Reinhard', 'AgX', 'Filmic', 'Neutral', 'AgX Punchy'];
  const LIGHTING = ['The table’s own', 'A level of your own', 'By the time of day'];
  const onOff = (b) => (b ? 'on' : 'off');
  const db = (v) => (v > 0 ? `+${v} dB` : `${v} dB`);
  // VPX's general table settings start from the table's own, unless the user's usual sets them
  const usualOf = (k, text) => (view.usual.yours.includes(k) ? `your usual: ${text}` : `the table’s own: ${text}`);
  const usualLabel = (k, text) => (view.usual.yours.includes(k) ? `Your usual (${text})` : `The table’s own (${text})`);

  // what its script sets for the game each time it starts, over the machine's settings here
  const SAYS = {
    sound: (v) => `its sound ${onOff(v)}`,
    volume: (v) => `its volume to ${db(v)}`,
    samples: (v) => `recorded sounds ${onOff(v)}`,
    cheat: (v) => `cheat mode ${onOff(v)}`,
    showpindmd: (v) => `the real DMD ${onOff(v)}`,
    sound_mode: (v) => `its AltSound pack ${onOff(v)}`,
  };
  const scriptSays = $derived.by(() => {
    const s = view?.machine?.scriptSets ?? {};
    const out = Object.entries(s).filter(([k]) => SAYS[k]).map(([k, v]) => SAYS[k](v));
    if (['dmd_red', 'dmd_green', 'dmd_blue'].some((k) => k in s)) out.push('its DMD color');
    return out;
  });

  function setLighting(v) {
    mine.lighting = v;
    // a level of its own starts where the table's own is
    mine.lightLevel = v === 1 ? (mine.lightLevel ?? view.usual.lightLevel) : null;
  }

  async function save() {
    busy = true;
    try {
      await invoke('save_table_settings', {
        id: t.id,
        options: $state.snapshot(options),
        forTable: $state.snapshot(mine),
        machine: view.machine ? [view.machine.game, $state.snapshot(machine)] : null,
      });
      if (outside.reach !== (view.reachOutside ?? null)) await invoke('save_reach_outside', { id: t.id, value: outside.reach });
      notice(`Saved for ${t.short}.`, { kind: 'done' });
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  function key(e) {
    if (e.key === 'Escape' && !busy) onclose();
  }
</script>

<svelte:window onkeydown={(e) => !physicsOpen && key(e)} />

{#if physicsOpen}
  <Physics {t} onclose={physicsClosed} />
{/if}

{#snippet tri(target, k, usual, label)}
  <div class="seg" role="group" aria-label={label}>
    {#each [[null, `Your usual (${onOff(usual)})`], [true, 'On'], [false, 'Off']] as [v, l] (l)}
      <button type="button" aria-pressed={target[k] === v} onclick={() => (target[k] = v)}>{l}</button>
    {/each}
  </div>
{/snippet}

{#snippet own(target, k, start, label)}
  <!-- ticked: the default (the table's own, or your usual); unticked: a value of its own to set -->
  <label class="check" title="Untick to set its own"><input type="checkbox" checked={target[k] === null} onchange={(e) => (target[k] = e.currentTarget.checked ? null : start)} aria-label="{label}: the default" />Default</label>
{/snippet}

<div class="scrim">
  <div class="win" role="dialog" aria-modal="true" aria-labelledby="ts-title" data-loading={view || problem ? undefined : ''}>
    <header>
      <div>
        <h1 id="ts-title" class="display">Settings for {t.short}</h1>
        <p>For this table alone, over your usual settings. Kept beside it in {view ? view.file.split('\\').pop() : 'its own settings file'}, which VPX reads too.</p>
      </div>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose} disabled={busy}><Icon name="x" /></button>
    </header>

    <div class="body">
      {#if problem}
        <p class="problem">{problem}</p>
      {:else if !view}
        <p class="soft">Asking the table’s script what options it offers…</p>
      {:else}
        <section class="block">
          <h2 class="section-title">Its own options</h2>
          {#if view.optionsProblem}
            <p class="soft">{view.optionsProblem}</p>
          {:else if !view.options.length}
            <p class="soft">Its script offers no options of its own.</p>
          {:else}
            {#each view.options as o (o.id)}
              <div class="row">
                <span class="name">{o.name}</span>
                {#if isToggle(o)}
                  <div class="seg" role="group" aria-label={o.name}>
                    {#each o.names as n, k (k)}<button type="button" aria-pressed={current(o) === o.min + k} onclick={() => (options[o.id] = o.min + k)}>{n}</button>{/each}
                  </div>
                {:else if o.names}
                  <select class="select" value={current(o)} onchange={(e) => (options[o.id] = Number(e.currentTarget.value))}>
                    {#each o.names as n, k (k)}<option value={o.min + k}>{n}</option>{/each}
                  </select>
                {:else}
                  <input type="range" min={o.min} max={o.max} step={o.step} value={current(o)} oninput={(e) => (options[o.id] = Number(e.currentTarget.value))} aria-label={o.name} />
                  <span class="val">{shown(o, current(o))}</span>
                {/if}
                {#if options[o.id] === null}
                  <span class="def">its default</span>
                {:else}
                  <button type="button" class="btn small ghost" onclick={() => (options[o.id] = null)} title="Back to its default: {shown(o, o.default)}">Default</button>
                {/if}
              </div>
            {/each}
          {/if}
        </section>

        <section class="block">
          <h2 class="section-title">For this table</h2>
          <!-- its VPS match, changed here later too -->
          <div class="row">
            <span class="name">On VPS</span>
            <span class="vps">{t.guess ? `A guess to confirm: ${t.guess.name}` : t.onVps.game ? `${t.name}, ${t.onVps.found === 'as you chose' ? 'chosen by you, in VPI only' : `found ${t.onVps.found}`}` : 'Not on VPS'}</span>
            {#if onfindvps}<button type="button" class="btn small" onclick={onfindvps}>Change…</button>{/if}
          </div>
          {#if t.renamedFrom}
            <!-- renamed to VPS's name for every program: put back from here -->
            <div class="row">
              <span class="name">Renamed</span>
              <span class="vps">From {t.renamedFrom}, for every program</span>
              <button type="button" class="btn small" disabled={putting} onclick={putBack}>Put the names back</button>
            </div>
          {/if}
          <div class="row">
            <span class="name">View</span>
            <div class="seg" role="group" aria-label="View">
              <button type="button" aria-pressed={mine.view === null} onclick={() => (mine.view = null)}>Your usual ({VIEWS[view.usual.view]})</button>
              {#each VIEWS as label, v (v)}<button type="button" aria-pressed={mine.view === v} onclick={() => (mine.view = v)}>{label}</button>{/each}
            </div>
            <!-- its own screens, the Screens page for it alone, beside this window -->
            {#if onscreens}<button type="button" class="btn small" onclick={onscreens}>Custom…</button>{/if}
          </div>
          <div class="row">
            <span class="name">Its screens</span>
            <select class="select" value={mine.screenPreset ?? ''} onchange={(e) => (mine.screenPreset = e.currentTarget.value || null)} aria-label="Its screens">
              <option value="">Your usual screens</option>
              {#each presets as p (p)}<option value={p}>The preset “{p}”</option>{/each}
              {#if mine.screenPreset && !presets.includes(mine.screenPreset)}<option value={mine.screenPreset}>“{mine.screenPreset}”, a preset that isn’t there</option>{/if}
            </select>
            <span class="def">{presets.length ? 'Presets are saved from the Screens page.' : 'Save a preset from the Screens page to choose it here.'}</span>
          </div>
          {#each [['playfieldVolume', 'Playfield volume'], ['backglassVolume', 'Backglass volume']] as [k, label] (k)}
            <div class="row">
              <span class="name">{label}</span>
              {@render own(mine, k, view.usual[k], label)}
              {#if mine[k] !== null}
                <input type="range" min="0" max="100" step="1" bind:value={mine[k]} aria-label={label} />
                <span class="val">{mine[k]}%</span>
              {:else}
                <span class="def">your usual: {view.usual[k]}%</span>
              {/if}
            </div>
          {/each}

          <div class="row">
            <span class="name">Tone mapper</span>
            <select class="select" value={mine.toneMapper ?? ''} onchange={(e) => (mine.toneMapper = e.currentTarget.value === '' ? null : Number(e.currentTarget.value))} aria-label="Tone mapper">
              <option value="">{usualLabel('toneMapper', MAPPERS[view.usual.toneMapper])}</option>
              {#each MAPPERS as m, k (k)}<option value={k}>{m}</option>{/each}
            </select>
          </div>
          <div class="row">
            <span class="name">Exposure</span>
            {@render own(mine, 'exposure', view.usual.exposure, 'Exposure')}
            {#if mine.exposure !== null}
              <input type="range" min="0" max="200" step="1" bind:value={mine.exposure} aria-label="Exposure" />
              <span class="val">{mine.exposure}%</span>
            {:else}
              <span class="def">{usualOf('exposure', `${view.usual.exposure}%`)}</span>
            {/if}
          </div>
          <div class="row">
            <span class="name">Scene lighting</span>
            <select class="select" value={mine.lighting ?? ''} onchange={(e) => setLighting(e.currentTarget.value === '' ? null : Number(e.currentTarget.value))} aria-label="Scene lighting">
              <!-- the table's own once, unless the user's usual says otherwise or its file names it -->
              <option value="">{view.usual.yours.includes('lighting') ? `Your usual (${LIGHTING[view.usual.lighting].toLowerCase()})` : LIGHTING[0]}</option>
              {#each LIGHTING as l, k (k)}{#if k || view.usual.yours.includes('lighting') || view.forTable.lighting === 0}<option value={k}>{l}</option>{/if}{/each}
            </select>
            {#if mine.lighting === 1}
              <!-- a hand-made file may switch it on with no level: VPX's full light then -->
              <input type="range" min="0" max="100" step="1" value={mine.lightLevel ?? 100} oninput={(e) => (mine.lightLevel = Number(e.currentTarget.value))} aria-label="Light level" />
              <span class="val">{mine.lightLevel ?? 100}%</span>
            {:else if mine.lighting === 2}
              <span class="def">VPX’s sun at the hour you play, for Berlin until your settings say where you live</span>
            {/if}
          </div>
          <div class="row">
            <span class="name">Difficulty</span>
            {@render own(mine, 'difficulty', view.usual.difficulty, 'Difficulty')}
            {#if mine.difficulty !== null}
              <input type="range" min="0" max="100" step="1" bind:value={mine.difficulty} aria-label="Difficulty" />
              <span class="val">{mine.difficulty}%</span>
            {:else}
              <span class="def">{usualOf('difficulty', `${view.usual.difficulty}%`)}</span>
            {/if}
          </div>
          {#if view.machine}
            <div class="row">
              <span class="name">AltSound pack</span>
              {@render tri(mine, 'altsound', view.usual.altsound, 'AltSound pack')}
            </div>
            <div class="row">
              <span class="name">Color DMD</span>
              {@render tri(mine, 'colour', view.usual.colour, 'Color DMD')}
            </div>
          {/if}
          <div class="row">
            <span class="name">PinUP pack</span>
            {@render tri(mine, 'pup', view.usual.pup, 'PinUP pack')}
          </div>
          <!-- screenshots when a game here makes the high scores, and which windows -->
          <div class="row">
            <span class="name">Screenshot at a high score</span>
            {@render tri(mine, 'screenshot', view.usual.screenshot, 'Screenshot at a high score')}
          </div>
          <div class="row">
            <span class="name">The screens it takes</span>
            {#each [['playfield', 'Playfield'], ['backglass', 'Backglass'], ['dmd', 'Score display'], ['topper', 'Topper']] as [w, label] (w)}
              {@const list = mine.screenshotScreens ?? view.usual.screenshotScreens}
              <label class="check"><input type="checkbox" checked={list.includes(w)} onchange={(e) => (mine.screenshotScreens = e.currentTarget.checked ? [...list, w] : list.filter((x) => x !== w))} />{label}</label>
            {/each}
            {#if mine.screenshotScreens}
              <button type="button" class="btn small ghost" onclick={() => (mine.screenshotScreens = null)}>Your usual</button>
            {/if}
          </div>
          <div class="row">
            <span class="name">Physics</span>
            {#if view.physics}
              <span class="def strong">From {view.physics} beside it, read before it plays, as VPX reads it</span>
              <button type="button" class="btn small ghost" onclick={() => showFile(t, 'physics')}>Show</button>
            {:else}
              <span class="def">The table’s own</span>
            {/if}
            <button type="button" class="btn small" aria-label="Change its physics" onclick={() => (physicsOpen = true)}>Change…</button>
          </div>
          <div class="row">
            <span class="name">Reaching outside its files</span>
            {@render tri(outside, 'reach', view.reachOutsideUsual, 'Reaching outside its files')}
          </div>
          <p class="note">Reaching outside its files: on, its script may use the internet, start other programs and reach every file, as VPX lets it — only for a table you trust; off, only its own files and what tables need to play. Kept in your own settings, never beside the table.</p>
          <p class="note">Tone mapper, exposure, scene lighting and difficulty are VPX’s own settings for each table, as its in-game menu offers them. The difficulty is how steep the table is, how big the flippers and how much the ball scatters. A physics file (.vpp) dropped on the table goes beside it.</p>
        </section>

        {#if view.machine}
          {@const m = view.machine}
          <section class="block">
            <h2 class="section-title">The machine: {m.game}</h2>
            <p class="note">PinMAME’s settings for {m.game}, for every table that plays it, kept as VPinMAME keeps them for each game.</p>
            <div class="row">
              <span class="name">Its sound</span>
              {@render tri(machine, 'sound', m.usual.sound, 'Its sound')}
            </div>
            <div class="row">
              <span class="name">Its volume</span>
              {@render own(machine, 'volume', m.usual.volume, 'Its volume')}
              {#if machine.volume !== null}
                <input type="range" min="-32" max="32" step="1" bind:value={machine.volume} aria-label="Its volume" />
                <span class="val">{db(machine.volume)}</span>
              {:else}
                <span class="def">your usual: {db(m.usual.volume)}</span>
              {/if}
            </div>
            <div class="row">
              <span class="name">Recorded sounds</span>
              {@render tri(machine, 'samples', m.usual.samples, 'Recorded sounds')}
              <span class="def">older machines’ sounds played from recordings, when PinMAME has them</span>
            </div>
            <div class="row">
              <span class="name">Skip its start-up tests</span>
              {@render tri(machine, 'cheat', m.usual.cheat, 'Skip its start-up tests')}
              <span class="def">PinMAME’s cheat mode</span>
            </div>
            {#if m.dmdDevice}
              <div class="row">
                <span class="name">On the real DMD</span>
                {@render tri(machine, 'realDmd', m.usual.realDmd, 'On the real DMD')}
              </div>
              <div class="row">
                <span class="name">Its color there</span>
                {@render own(machine, 'dmdColour', m.usual.dmdColour, 'Its color on the real DMD')}
                {#if machine.dmdColour !== null}
                  <input type="color" bind:value={machine.dmdColour} aria-label="Its color on the real DMD" />
                {:else}
                  <span class="swatch" style="background: {m.usual.dmdColour}"></span><span class="def">your usual</span>
                {/if}
              </div>
            {/if}
            {#if scriptSays.length}
              <p class="note warn">Its script sets {scriptSays.join(', ')} each time it starts, over what’s here.</p>
            {/if}
          </section>
        {/if}
      {/if}
    </div>

    <footer>
      <button type="button" class="btn ghost" onclick={onclose} disabled={busy}>Cancel</button>
      <button type="button" class="btn save" onclick={save} disabled={busy || !view}><Icon name="check" />Save</button>
    </footer>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 40; background: rgba(5, 7, 11, 0.72); display: flex; align-items: center; justify-content: center; padding: 28px; }
  .win { width: min(960px, 100%); max-height: 100%; display: flex; flex-direction: column; background: var(--panel); border: 1px solid var(--panel-line); border-radius: 18px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6); }
  header { display: flex; justify-content: space-between; gap: 16px; padding: 22px 24px 14px; border-bottom: 1px solid var(--line-soft); }
  h1 { font-size: 30px; line-height: 1.05; }
  header p { margin: 6px 0 0; color: var(--muted); }
  .body { overflow: auto; padding: 16px 24px; display: flex; flex-direction: column; gap: 18px; min-height: 120px; }
  .soft { color: var(--faint); margin: 0; }
  .problem { margin: 0; padding: 10px 14px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .block { display: flex; flex-direction: column; gap: 10px; }
  .row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; min-height: 34px; }
  .name { flex: 0 0 200px; color: var(--fg-soft); font-size: 14px; }
  .vps { flex: 1 1 260px; min-width: 0; color: var(--fg); font-size: 14px; }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-pressed='true'] { background: var(--accent-bg); color: #fff; }
  input[type='range'] { width: 240px; accent-color: var(--accent); }
  input[type='color'] { width: 56px; height: 32px; padding: 2px; border: 1px solid var(--control-line); border-radius: 8px; background: var(--control); }
  .swatch { width: 22px; height: 22px; border-radius: 6px; border: 1px solid var(--control-line); }
  .val { min-width: 54px; font-variant-numeric: tabular-nums; }
  .def { color: var(--faint); font-size: 13px; }
  .def.strong { color: var(--fg-soft); }
  .check { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .note { margin: 0; color: var(--faint); font-size: 13px; }
  .note.warn { color: var(--warn); }
  footer { display: flex; gap: 12px; justify-content: flex-end; padding: 14px 24px 18px; border-top: 1px solid var(--line-soft); }
  .save { background: var(--accent-bg); border-color: var(--accent); }
</style>
