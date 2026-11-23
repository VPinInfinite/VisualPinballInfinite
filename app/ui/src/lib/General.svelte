<script>
  // Settings › General: where you are, for the light of day
  // (a location under General, for a table's lighting following the sun); screenshots for
  // every table (off unless on, a table's own over it); the player's readout. The player
  // reads each today, under VPX 5436's names; what's still to build says so.
  import { invoke } from '@tauri-apps/api/core';
  import { getVersion } from '@tauri-apps/api/app';
  import Icon from './Icon.svelte';
  import IniPage from './IniPage.svelte';
  import { app, notice, lookForUpdate, savePrefs, setFullscreen } from './store.svelte.js';

  const parts = [
    {
      name: 'Start-up',
      rows: [
        { label: 'First setup', kind: 'custom', name: 'again' },
        { label: 'The app’s window', kind: 'custom', name: 'appWindow' },
        { label: 'The app opens on', kind: 'custom', name: 'opensOn' },
      ],
    },
    {
      name: 'Where you are',
      note: 'For the light of day: a table’s lighting can follow the sun where you are (Picture › Light level, From the sun). Only the place’s latitude and longitude are kept.',
      rows: [
        { label: 'Find your place', kind: 'custom', name: 'locate', keys: ['Player/Latitude', 'Player/Longitude'] },
        { label: 'Latitude', kind: 'number', key: 'Player/Latitude', min: -90, max: 90, step: 0.01, placeholder: '52.52', unit: '° north (south below 0)', what: 'Berlin’s, VPX’s own, until you set yours.', vpx: '[Player] Latitude' },
        { label: 'Longitude', kind: 'number', key: 'Player/Longitude', min: -180, max: 180, step: 0.01, placeholder: '13.37', unit: '° east (west below 0)', vpx: '[Player] Longitude' },
      ],
    },
    {
      name: 'VPX’s readout',
      rows: [
        {
          label: 'Frame rate and timings',
          kind: 'choice',
          options: [
            { label: 'Hidden', set: { 'Player/ShowFPS': [null, '0'] } },
            { label: 'Shown', set: { 'Player/ShowFPS': ['1', '2'] } },
          ],
          what: 'The small readout in a corner while playing.',
          vpx: '[Player] ShowFPS',
        },
      ],
    },
  ];


  // the app's version, as bug reports ask for it
  let version = $state('');
  getVersion().then((v) => (version = v)).catch(() => {});

  // this PC's place from Windows' location service, rounded to about a kilometre, to be saved
  let locating = $state(false);
  async function findPlace(set) {
    locating = true;
    try {
      const [lat, lon] = await invoke('this_pc_location');
      set('Player/Latitude', lat.toFixed(2));
      set('Player/Longitude', lon.toFixed(2));
      notice('This PC’s place is filled in: Save keeps it.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    locating = false;
  }
</script>

{#snippet again()}
  <span class="kept">Where your library lives — your VPX setup's folders, or a new library — and the screens and controls in a word</span>
  <button type="button" class="btn small ghost" onclick={() => (app.setupAgain = true)}>Run it again</button>
{/snippet}

{#snippet opensOn()}
  <span class="seg" role="radiogroup" aria-label="The app opens on">
    {#each [['library', 'The library'], ['lastTable', 'The last table’s page']] as [v, label] (v)}
      <button type="button" role="radio" aria-checked={(app.prefs.opensOn ?? 'library') === v} onclick={() => { app.prefs.opensOn = v; savePrefs(); }}>{label}</button>
    {/each}
  </span>
{/snippet}

{#snippet appWindow()}
  <span class="kept">{app.fullscreen ? 'Full screen, and so the next time it opens' : 'In a window'} · F11 switches</span>
  <button type="button" class="btn small ghost" onclick={() => setFullscreen(!app.fullscreen)}><Icon name={app.fullscreen ? 'shrink' : 'expand'} size={14} />{app.fullscreen ? 'In a window' : 'Full screen'}</button>
{/snippet}

{#snippet locate(get, set)}
  <span class="kept">This PC’s own location, from Windows, filled in below to save</span>
  <button type="button" class="btn small ghost" disabled={locating} onclick={() => findPlace(set)}><Icon name="pin" size={14} />{locating ? 'Asking Windows…' : 'Use this PC’s location'}</button>
{/snippet}

{#snippet about()}
  {#if version}
    <p class="about">
      Visual Pinball Infinite {version}
      <button type="button" class="btn small ghost" onclick={() => invoke('open_link', { url: `https://github.com/VPinInfinite/VisualPinballInfinite/releases/tag/v${version}` }).catch((e) => notice(String(e), { kind: 'problem' }))}>Its release notes</button>
      <button type="button" class="btn small ghost" onclick={() => lookForUpdate(true)}>Look for a new version</button>
    </p>
  {/if}
{/snippet}

<IniPage title="General" lead="The app’s own choices: its window, where you are for the light of day, and VPX’s readout while playing." {parts} custom={{ again, locate, appWindow, opensOn }} after={about} />

<style>
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .kept { font-size: 13px; color: var(--fg-soft); }
  .about { margin: 28px 0 0; font-size: 13px; color: var(--faint); display: flex; gap: 12px; align-items: center; }
</style>
