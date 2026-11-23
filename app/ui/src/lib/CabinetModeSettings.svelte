<script>
  // Settings › Cabinet: cabinet mode's own choices — what the playfield screen shows (a wheel or a
  // row; videos with their sound, or pictures), the volume of what it plays, its picture
  // turned for a screen lying another way than Windows has it, moving through the tables by itself
  // when nobody plays, opening in it, and the app starting with Windows.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { app, savePrefs, notice, cabinetVolume, cabinetSoundOrder } from './store.svelte.js';

  let withWindows = $state(false);
  let presets = $state([]);
  onMount(() => {
    invoke('starts_with_windows').then((v) => (withWindows = v)).catch(() => {});
    invoke('screen_presets').then((p) => (presets = p)).catch(() => {});
  });

  function set(name, value) {
    app.prefs[name] = value;
    savePrefs();
  }

  async function setWindows(on) {
    try {
      await invoke('set_start_with_windows', { on });
      withWindows = on;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // the screens whose video plays its sound, one moved up or down the order
  const SOUND_NAMES = { backglass: 'The backglass’s video', topper: 'The topper’s video', playfield: 'The playfield’s video' };
  function moveSound(i, by) {
    const order = cabinetSoundOrder(app.prefs);
    const j = i + by;
    if (j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    set('cabinetSoundOrder', order);
  }

  const LOOKS = [['wheel', 'A wheel of their logos'], ['row', 'A row of their playfields']];
  const TURNS = [[0, 'As Windows has it'], [90, 'A quarter turn right'], [270, 'A quarter turn left'], [180, 'Upside down']];
  const ATTRACT = [[1, 'After a minute'], [3, 'After 3 minutes'], [5, 'After 5 minutes'], [10, 'After 10 minutes'], [0, 'Never']];
</script>

<section class="cm" aria-labelledby="cab-mode">
  <h2 id="cab-mode" class="partname">Cabinet mode</h2>
  <p class="what">The app full screen on your cabinet’s screens, with the tables chosen by its buttons: the flippers move a table, the magna-saves a letter, Start plays, and Exit opens its menu.</p>
  <div class="row">
    <span class="name">The playfield screen shows</span>
    <div class="seg" role="radiogroup" aria-label="The playfield screen shows">
      {#each LOOKS as [v, l] (v)}<button type="button" role="radio" aria-checked={(app.prefs.cabinetLook || 'wheel') === v} onclick={() => set('cabinetLook', v)}>{l}</button>{/each}
    </div>
  </div>
  <div class="row">
    <span class="name">Its playfield</span>
    <div class="seg" role="radiogroup" aria-label="Its playfield">
      {#each [[true, 'Videos, with their sound'], [false, 'Pictures']] as [v, l] (l)}<button type="button" role="radio" aria-checked={(app.prefs.cabinetVideos ?? true) === v} onclick={() => set('cabinetVideos', v)}>{l}</button>{/each}
    </div>
    <span class="what">A table without a video shows its picture; without sound, its music where it has some.</span>
  </div>
  <div class="row">
    <span class="name">Its volume</span>
    <input type="range" class="range" min="0" max="100" step="5" value={cabinetVolume(app.prefs)} aria-label="Its volume" oninput={(e) => set('cabinetVolume', Number(e.currentTarget.value))} />
    <span class="pct">{cabinetVolume(app.prefs) ? `${cabinetVolume(app.prefs)}%` : 'Off'}</span>
    <span class="what">One volume for every table while you browse, whatever a table’s own settings or its script’s: the videos the app makes are recorded equally loud. It’s also in cabinet mode’s menu.</span>
  </div>
  <div class="row top">
    <span class="name">Its sound</span>
    <ol class="order" aria-label="Its sound, in order">
      {#each cabinetSoundOrder(app.prefs) as s, i (s)}
        <li>
          <span class="n">{i + 1}</span>{SOUND_NAMES[s]}
          <button type="button" class="btn small ghost" aria-label="Move up" disabled={i === 0} onclick={() => moveSound(i, -1)}>↑</button>
          <button type="button" class="btn small ghost" aria-label="Move down" disabled={i === 2} onclick={() => moveSound(i, 1)}>↓</button>
        </li>
      {/each}
    </ol>
    <span class="what">One sound plays at a time: the first of these videos that has sound, on a screen your cabinet has. When none has sound, the table’s music plays. The score display’s videos play silent.</span>
  </div>
  <div class="row">
    <span class="name">Its screens</span>
    <select class="select" value={app.prefs.cabinetPreset || ''} onchange={(e) => set('cabinetPreset', e.currentTarget.value)}>
      <option value="">Your usual screens</option>
      {#each presets as p (p)}<option value={p}>The preset {p}</option>{/each}
    </select>
    <span class="what">Cabinet mode and the tables it starts use these; the desktop app keeps your usual screens. Presets are made on the Screens page.</span>
  </div>
  <div class="row">
    <span class="name">Its picture</span>
    <select class="select" value={app.prefs.cabinetTurn || 0} onchange={(e) => set('cabinetTurn', Number(e.currentTarget.value))}>
      {#each TURNS as [v, l] (v)}<option value={v}>{l}</option>{/each}
    </select>
  </div>
  <div class="row">
    <span class="name">Through the tables by itself</span>
    <select class="select" value={app.prefs.cabinetAttract ?? 3} onchange={(e) => set('cabinetAttract', Number(e.currentTarget.value))}>
      {#each ATTRACT as [v, l] (v)}<option value={v}>{l}</option>{/each}
    </select>
  </div>
  <label class="check"><input type="checkbox" checked={!!app.prefs.startInCabinet} onchange={(e) => set('startInCabinet', e.currentTarget.checked)} />The app opens in cabinet mode</label>
  <label class="check"><input type="checkbox" checked={withWindows} onchange={(e) => setWindows(e.currentTarget.checked)} />The app starts with Windows</label>
  <button type="button" class="btn open" onclick={() => (app.cabinet = true)}>Open cabinet mode</button>
</section>

<style>
  .cm { display: flex; flex-direction: column; gap: 10px; padding: 28px 32px 8px; max-width: 1240px; }
  .partname { margin: 0; font-size: 22px; }
  .what { margin: 0 0 4px; color: var(--muted); max-width: 720px; }
  .row { display: flex; align-items: center; gap: 14px; flex-wrap: wrap; }
  .name { min-width: 220px; color: var(--fg-soft); }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 10px; overflow: hidden; }
  .seg button { padding: 7px 14px; border: 0; background: transparent; color: var(--fg-soft); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: var(--fg); }
  .select { height: 36px; }
  .range { width: 260px; accent-color: var(--accent); }
  .pct { min-width: 44px; color: var(--fg-soft); font-variant-numeric: tabular-nums; }
  .row.top { align-items: flex-start; }
  .order { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; min-width: 260px; }
  .order li { display: flex; align-items: center; gap: 8px; padding: 4px 8px; border: 1px solid var(--control-line); border-radius: 8px; color: var(--fg-soft); }
  .order .n { width: 18px; color: var(--accent); font-variant-numeric: tabular-nums; }
  .order li button:first-of-type { margin-left: auto; }
  .check { display: flex; align-items: center; gap: 8px; color: var(--fg-soft); }
  .open { align-self: flex-start; margin-top: 4px; }
</style>
