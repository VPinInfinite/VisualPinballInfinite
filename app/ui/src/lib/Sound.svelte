<script>
  // Settings: Sound. Which speakers the playfield's sounds and the
  // backglass's play through, the playfield's speaker layout, and the volumes — the player's own
  // settings under VPX's names, saved into the settings file as the Screens page saves: only what
  // changed.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import SpeakerPreview from './SpeakerPreview.svelte';
  import { notice } from './store.svelte.js';

  let view = $state(null);
  let draft = $state(null);
  let saved = $state('');
  let busy = $state(false);
  let problem = $state('');

  const changed = $derived(!!draft && JSON.stringify(draft) !== saved);

  async function load() {
    problem = '';
    try {
      const v = await invoke('sound_view');
      view = v;
      draft = structuredClone(v.settings);
      saved = JSON.stringify(draft);
    } catch (e) {
      problem = String(e);
    }
  }

  onMount(load);

  async function save() {
    busy = true;
    try {
      await invoke('save_sound', { settings: $state.snapshot(draft) });
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

  async function showFile() {
    try {
      await invoke('show_settings_file');
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  const channels = (n) => (n === 2 ? 'stereo' : n === 6 ? '5.1' : n === 8 ? '7.1' : n ? `${n} channels` : '');

  // an output the file names that isn't connected now still shows, so it isn't lost by saving
  function choices(current) {
    const list = (view?.outputs || []).map((o) => ({ value: o.name, label: `${o.name}${o.channels ? ` — ${channels(o.channels)}` : ''}` }));
    if (current && !list.some((o) => o.value === current)) list.push({ value: current, label: `${current} — not connected now` });
    return [{ value: '', label: `Windows’ default${view?.defaultChannels ? ` — ${channels(view.defaultChannels)}` : ''}` }, ...list];
  }

  // VPX 5436's six layouts for the playfield's sound (`Sound3D`), in plain words, with a cabinet
  // showing where the sound comes from beside them
  const layouts = [
    { v: 0, name: 'Stereo', text: 'Two speakers — a desk’s, a TV’s, or a cabinet’s own pair on their own sound card. The table’s sounds move left and right.' },
    { v: 1, name: 'Stereo on the rear outputs', text: 'For a cabinet with one sound card: the table’s sounds on its rear outputs, to speakers in the cabinet’s body; the front outputs left for the backglass.' },
    { v: 2, name: 'Surround, the rear speakers by the lockbar', text: 'A 5.1 or 7.1 set: the front pair at the table’s far end, the rear pair by you — as in a home cinema, the ball rolling toward you comes from behind.' },
    { v: 3, name: 'Surround, the front speakers by the lockbar', text: 'The same, plugged in the other way round: the front pair by you, the rear pair at the table’s far end.' },
    { v: 4, name: 'Cabinet surround (SSF), VPX’s older mixing', text: 'Speakers or shakers under the playfield: the side outputs at its far end, the rear outputs by the lockbar; the front outputs left for the backglass. Mixed as VPX’s older builds did.' },
    { v: 5, name: 'Cabinet surround (SSF), VPX’s newer mixing', text: 'The same, mixed as VPX 10.8.1 mixes it — what most surround-sound cabinets are set up for.' },
  ];
  let shown = $state(null);

  const volumes = [
    ['playfieldVolume', 'Playfield', 'The machine’s mechanical sounds: flippers, the ball, bumpers.'],
    ['backglassVolume', 'Backglass', 'Music, the machine’s own sound from its ROM, PinUP packs and the like.'],
    ['pupVolume', 'PinUP packs', 'A PinUP pack’s videos and sounds, on top of the backglass volume.'],
  ];
</script>

<div class="page" data-loading={draft || problem ? undefined : ''}>
  <header>
    <div class="title">
      <h1 class="display">Sound</h1>
      <p>Which speakers each part of a table plays through, and how loud. The player reads these whenever a table starts; <kbd>-</kbd> and <kbd>=</kbd> change both volumes while you play.</p>
    </div>
    <div class="actions">
      <button type="button" class="btn ghost" onclick={undo} disabled={!changed || busy}>Undo changes</button>
      <button type="button" class="btn save" onclick={save} disabled={!changed || busy}><Icon name="check" />Save</button>
    </div>
  </header>

  {#if problem}
    <div class="problem">{problem} <button type="button" class="btn small" onclick={load}><Icon name="refresh" size={15} />Try again</button></div>
  {:else if !draft}
    <p class="soft">Looking at your sound outputs…</p>
  {:else}
    <section class="block">
      <h2 class="section-title">Speakers</h2>
      <div class="row">
        <span class="name">The playfield’s sounds</span>
        <select class="select long" value={draft.playfieldOutput || ''} onchange={(e) => (draft.playfieldOutput = e.currentTarget.value || null)}>
          {#each choices(draft.playfieldOutput) as c (c.value)}<option value={c.value}>{c.label}</option>{/each}
        </select>
      </div>
      <div class="row">
        <span class="name">The backglass’s sounds</span>
        <select class="select long" value={draft.backglassOutput || ''} onchange={(e) => (draft.backglassOutput = e.currentTarget.value || null)}>
          {#each choices(draft.backglassOutput) as c (c.value)}<option value={c.value}>{c.label}</option>{/each}
        </select>
      </div>
      <p class="note">A cabinet often sends the playfield’s sounds to the speakers or shakers in its body and the backglass’s to the speakers in its backbox. One set of speakers can play both.</p>
    </section>

    <section class="block">
      <h2 class="section-title">The playfield’s speaker layout</h2>
      <div class="speakers">
        <div class="layouts" role="radiogroup" aria-label="The playfield’s speaker layout">
          {#each layouts as l (l.v)}
            <button
              type="button"
              class="layout"
              role="radio"
              aria-checked={draft.layout === l.v}
              onclick={() => (draft.layout = l.v)}
              onpointerenter={() => (shown = l.v)}
              onpointerleave={() => (shown = null)}
              onfocus={() => (shown = l.v)}
              onblur={() => (shown = null)}
            >
              <b>{l.name}</b><span>{l.text}</span>
            </button>
          {/each}
        </div>
        <figure class="preview">
          <SpeakerPreview layout={shown ?? draft.layout} />
          <figcaption>{shown !== null && shown !== draft.layout ? 'Pointed at: ' : ''}{layouts.find((l) => l.v === (shown ?? draft.layout))?.name}</figcaption>
        </figure>
      </div>
    </section>

    <section class="block">
      <h2 class="section-title">Volumes</h2>
      {#each volumes as [key, label, what] (key)}
        <div class="row vol">
          <span class="name">{label}</span>
          <input type="range" min="0" max="100" step="1" bind:value={draft[key]} aria-label="{label} volume" />
          <span class="pct">{draft[key]}%</span>
          <span class="what">{what}</span>
        </div>
      {/each}
    </section>

    <section class="block">
      <h2 class="section-title">Achievements</h2>
      <label class="check line"><input type="checkbox" bind:checked={draft.achievementSound} />Play a sound as each achievement is announced</label>
      <div class="row vol" class:off={!draft.achievementSound}>
        <span class="name">Its volume</span>
        <input type="range" min="0" max="200" step="5" bind:value={draft.achievementVolume} disabled={!draft.achievementSound} aria-label="The achievements’ sound’s volume" />
        <span class="pct">{draft.achievementVolume}%</span>
        <span class="what">The pack’s own sound, or the player’s chime, on top of the backglass volume: 100% as it comes. The pause menu changes it too while you play.</span>
      </div>
    </section>

    <section class="block">
      <h2 class="section-title">Sound packs</h2>
      <label class="check line"><input type="checkbox" bind:checked={draft.altsound} />Play a game’s AltSound pack when it has one</label>
      <p class="note">AltSound packs replace a machine’s own music and sounds with remastered ones. Turned off, the machine’s own sound plays.</p>
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
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  kbd { padding: 0 6px; border-radius: 5px; background: var(--control); border: 1px solid var(--control-line); font-family: inherit; font-size: 13px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .save { background: var(--accent-bg); border-color: var(--accent); }
  .soft { color: var(--faint); }
  .problem { padding: 14px 16px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; display: flex; gap: 12px; align-items: center; }
  .block { display: flex; flex-direction: column; gap: 10px; }
  .row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .name { min-width: 190px; color: var(--fg-soft); font-size: 14px; }
  .select.long { min-width: 460px; }
  .speakers { display: grid; grid-template-columns: minmax(0, 1fr) 260px; gap: 20px; align-items: start; }
  @media (max-width: 900px) { .speakers { grid-template-columns: 1fr; } .preview { order: -1; } }
  .layouts { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 10px; }
  .preview { margin: 0; padding: 14px 12px 12px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); position: sticky; top: 12px; }
  .preview figcaption { margin-top: 6px; text-align: center; font-size: 13px; color: var(--fg-soft); }
  .layout {
    text-align: left; padding: 12px 14px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line);
    display: flex; flex-direction: column; gap: 4px; color: var(--fg);
  }
  .layout:hover { border-color: var(--accent); }
  .layout[aria-checked='true'] { border-color: var(--accent); background: var(--accent-bg); }
  .layout span { color: var(--muted); font-size: 13px; }
  .vol input[type='range'] { width: 280px; accent-color: var(--accent); }
  .pct { width: 48px; text-align: right; font-variant-numeric: tabular-nums; }
  .what { color: var(--faint); font-size: 13px; }
  .vol.off { opacity: 0.5; }
  .check { display: inline-flex; align-items: center; gap: 8px; color: var(--fg-soft); }
  .check.line { font-size: 14px; }
  .note { margin: 0; color: var(--faint); font-size: 13px; max-width: 760px; }
  .file { margin: 0; color: var(--faint); font-size: 12px; display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
</style>
