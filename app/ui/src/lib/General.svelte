<script>
  // Settings › General: where you are, for the light of day
  // (a location under General, for a table's lighting following the sun); screenshots for
  // every table (off unless on, a table's own over it); the player's readout. The player
  // reads each today, under VPX 5436's names; what's still to build says so.
  import { invoke } from '@tauri-apps/api/core';
  import { getVersion } from '@tauri-apps/api/app';
  import Icon from './Icon.svelte';
  import IniPage from './IniPage.svelte';
  import VpidbLink from './VpidbLink.svelte';
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
      name: 'Playing',
      rows: [
        {
          label: 'Esc while playing',
          kind: 'toggle',
          key: 'VPI/PauseMenuBeforeQuit',
          def: true,
          onLabel: 'Opens the pause menu',
          offLabel: 'Quits the table',
          what: 'The pause menu opens with Quit the table picked, so Esc then Enter quits, and Esc again plays on. When a front end such as PinUP Popper starts a table with VPX’s -minimized switch, Esc quits at once, as VPX does, so a cabinet’s Exit button goes straight back.',
          vpx: '[VPI] PauseMenuBeforeQuit',
        },
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
      name: 'Screenshots',
      note: 'The Screenshot key takes them while you play: F12 unless the Controls page gives it another.',
      rows: [
        { label: 'When a game makes the high scores', kind: 'toggle', key: 'VPI/ScreenshotAtHighScore', def: false, onLabel: 'Take a screenshot', offLabel: 'Off', what: 'A ROM machine’s game when it makes the high scores in its memory, or a table whose script keeps its own scores when it saves a new one; a table can turn it on for itself alone, in its own settings.', vpx: '[VPI] ScreenshotAtHighScore' },
        { label: 'The screens taken', kind: 'checks', key: 'VPI/ScreenshotScreens', options: [['playfield', 'Playfield'], ['backglass', 'Backglass'], ['dmd', 'Score display'], ['topper', 'Topper']], what: 'Each that’s showing; a table can choose its own.', vpx: '[VPI] ScreenshotScreens' },
        { label: 'Kept in', kind: 'custom', name: 'folder' },
      ],
    },
    {
      name: 'Table scripts',
      note: 'A table’s script is a small program. Out of the box it uses only its own files and what tables need to play. You choose, for every table here or for one in its own settings.',
      rows: [
        { label: 'Tables reaching outside their files', kind: 'toggle', key: 'VPI/ScriptsReachOutside', def: false, onLabel: 'Let them', offLabel: 'Off', what: 'Let them: tables may use the internet, start other programs and reach every file, as VPX lets them — an online leaderboard needs it. Only for tables you trust.', vpx: '[VPI] ScriptsReachOutside' },
      ],
    },
    {
      name: 'The player’s readout',
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

  const open = () => invoke('show_folder', { key: 'screenshots' }).catch((e) => notice(String(e), { kind: 'problem' }));

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
  <section class="vpidb" id="vpidb-settings" aria-label="VPIDB">
    <h2>VPIDB</h2>
    <p class="vnote">Your link to VPIDB, and what you share there: the achievements you earn, and your ratings and which tables you have. Switch either off at any time.</p>
    <VpidbLink heading={false} />
  </section>
  {#if version}
    <p class="about">Visual Pinball Infinite {version} <button type="button" class="btn small ghost" onclick={() => lookForUpdate(true)}>Look for a new version</button></p>
  {/if}
{/snippet}

{#snippet folder()}
  <span class="kept">Visual Pinball Infinite\Screenshots, a folder for each table; on a table’s page its Screenshots tile shows them</span>
  <button type="button" class="btn small ghost" onclick={open}><Icon name="folder" size={14} />Open</button>
{/snippet}

<IniPage title="General" lead="The app’s own choices: its window, Esc while playing, where you are for the light of day, screenshots, and the player’s readout." {parts} custom={{ folder, again, locate, appWindow, opensOn }} after={about} />

<style>
  .vpidb { display: flex; flex-direction: column; gap: 10px; padding-top: 22px; margin-top: 8px; border-top: 1px solid var(--line); }
  .vpidb h2 { margin: 0; font-size: 20px; font-weight: 700; color: var(--fg); }
  .vnote { margin: 0; color: var(--muted); font-size: 14px; max-width: 680px; }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .kept { font-size: 13px; color: var(--fg-soft); }
  .about { margin: 28px 0 0; font-size: 13px; color: var(--faint); display: flex; gap: 12px; align-items: center; }
</style>
