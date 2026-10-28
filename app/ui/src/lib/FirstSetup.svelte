<script>
  // First setup: shown when the settings name no library yet,
  // or again from Settings › General. Where the library lives — the VPX setup found on this PC, or
  // one whose folders the user chooses, any folder changed, shared as it is
  // with VPX, PinUP Popper and VPin Studio; or a new library of our own — then the screens and
  // controls in a word each, and done. It stays until it's done, though the library is read as soon
  // as its folders are kept. Each step's words are short; the pages that set things in full are named.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, notice } from './store.svelte.js';

  let { again = false, onclose = () => {} } = $props();

  let step = $state(0);
  let offer = $state(null);
  let choice = $state('vpx');
  let root = $state('');
  // how a new library keeps each table's files: shared folders unless the user chooses otherwise
  let perGame = $state(false);
  let busy = $state(false);
  let problem = $state('');
  let screens = $state([]);
  let openScreens = $state(false);
  // the VPX setup's folders: those found, as the user changes them
  let folders = $state({ tables: '', pinmame: '', scripts: '', pinup: '' });

  onMount(async () => {
    app.setupOpen = true;
    try {
      offer = await invoke('setup_offer');
      if (offer.vpx) folders = { ...offer.vpx };
      choice = offer.vpx ? 'vpx' : 'fresh';
      root = offer.fresh;
    } catch (e) {
      problem = String(e);
    }
    screens = await invoke('screens_now').catch(() => []);
    openScreens = screens.length > 1;
  });

  const STEPS = ['Welcome', 'Your tables', 'Your screens', 'Your controls', 'Done'];

  // the VPX setup's folders, each chosen in Windows' folder picker
  const ROWS = [
    { key: 'tables', label: 'Tables', title: 'Choose the folder your tables are in' },
    { key: 'pinmame', label: 'ROMs (VPinMAME)', title: 'Choose VPinMAME’s folder, the one with its roms folder in it' },
    { key: 'scripts', label: 'Scripts', title: 'Choose the folder of VPX’s scripts' },
    { key: 'pinup', label: 'PinUP packs', title: 'Choose PinUP’s PUPVideos folder' },
  ];

  async function picked(title, start) {
    try {
      return await invoke('pick_folder', { title, start: start || null });
    } catch (e) {
      problem = String(e);
      return null;
    }
  }

  // VPX's own folder, or its tables' folder: everything found from it
  async function chooseVpx() {
    choice = 'vpx';
    const folder = await picked('Choose the folder VPX is in, or the folder your tables are in', folders.tables);
    if (folder) folders = await invoke('setup_look', { folder });
  }

  // one folder; a tables' folder brings the others found beside it, where none is set
  async function choose(row) {
    choice = 'vpx';
    const folder = await picked(row.title, folders[row.key] || folders.tables);
    if (!folder) return;
    if (row.key === 'tables') {
      const found = await invoke('setup_look', { folder }).catch(() => null);
      folders.tables = found?.tables || folder;
      for (const k of ['pinmame', 'scripts', 'pinup']) if (!folders[k] && found?.[k]) folders[k] = found[k];
    } else {
      folders[row.key] = folder;
    }
  }

  async function chooseRoot() {
    choice = 'fresh';
    const folder = await picked('Choose where the new library goes', root);
    if (folder) root = folder;
  }

  async function keepLibrary() {
    busy = true;
    problem = '';
    try {
      await invoke('setup_keep', choice === 'vpx' ? { folders: $state.snapshot(folders), root: null, perGame: null } : { folders: null, root, perGame });
      step = 2;
    } catch (e) {
      problem = String(e);
    }
    busy = false;
  }

  function finish() {
    onclose();
    if (openScreens) {
      app.page = 'settings';
      app.settingsPage = 'screens';
    } else {
      app.page = 'library';
    }
    if (again) notice('Your library is set up.', { kind: 'done' });
  }

  const name = (path) => path || '—';
</script>

<div class="cover" role="dialog" aria-modal="true" aria-labelledby="fs-title">
  <div class="box">
    <ol class="steps" aria-label="Steps">
      {#each STEPS as s, i (s)}<li class:on={i === step} class:done={i < step}>{s}</li>{/each}
    </ol>

    {#if step === 0}
      <section>
        <h1 id="fs-title" class="display">Welcome to Visual Pinball Infinite</h1>
        <p class="lead">VPI plays your Visual Pinball X tables — ROM machines, backglasses, PinUP packs and all — and keeps your library in order. A minute to set it up.</p>
        <div class="actions">
          {#if again}<button type="button" class="btn ghost" onclick={onclose}>Cancel</button>{/if}
          <button type="button" class="btn save" onclick={() => (step = 1)}>Set it up<Icon name="chevR" size={16} /></button>
        </div>
      </section>
    {:else if step === 1}
      <section>
        <h1 id="fs-title" class="display">Where are your tables?</h1>
        {#if !offer}
          <p class="soft">Looking for VPX on this PC…</p>
        {:else}
          <div class="choices">
            <div class="choice" class:on={choice === 'vpx'}>
              <button type="button" class="pick" aria-pressed={choice === 'vpx'} onclick={() => (folders.tables ? (choice = 'vpx') : chooseVpx())}>
                <span class="radio" aria-hidden="true"></span>
                <span class="words">
                  <b>{offer.vpx ? 'Use your VPX setup' : 'Use a VPX setup'}</b>
                  <span>{offer.vpx ? 'Found on this PC.' : 'VPI didn’t find VPX on this PC. If it’s here, choose its folder — the one with Tables in it — or your tables’ folder, and VPI finds the rest.'} VPI shares its folders with VPX, PinUP Popper and VPin Studio: nothing is moved or copied, and your high scores come over from VPX the first time each table plays here.</span>
                </span>
              </button>
              {#if folders.tables}
                <dl>
                  {#each ROWS as row (row.key)}
                    <dt>{row.label}</dt>
                    <dd><span class="path">{name(folders[row.key])}</span><button type="button" class="link" onclick={() => choose(row)}>{folders[row.key] ? 'Change' : 'Choose…'}</button></dd>
                  {/each}
                </dl>
              {:else}
                <div class="field"><button type="button" class="btn" onclick={chooseVpx}><Icon name="folder" size={16} />Choose VPX’s folder…</button></div>
              {/if}
            </div>
            <div class="choice" class:on={choice === 'fresh'}>
              <button type="button" class="pick" aria-pressed={choice === 'fresh'} onclick={() => (choice = 'fresh')}>
                <span class="radio" aria-hidden="true"></span>
                <span class="words">
                  <b>Start a new library</b>
                  <span>VPI makes its folders — tables, ROMs, scripts, PinUP packs — in the folder you give. You add tables by dropping their downloads on the app.</span>
                </span>
              </button>
              {#if choice === 'fresh'}
                <div class="field">
                  <input class="select" bind:value={root} aria-label="The new library's folder" />
                  <button type="button" class="btn" onclick={chooseRoot}><Icon name="folder" size={16} />Choose…</button>
                </div>
                <div class="layout" role="radiogroup" aria-label="How the new library keeps each table's files">
                  <span class="layout-q">How should it keep each table’s files?</span>
                  <label class="opt" class:on={!perGame}>
                    <input type="radio" name="layout" checked={!perGame} onchange={() => (perGame = false)} />
                    <span><span class="opt-name"><b>Shared folders</b><span class="rec">Recommended</span></span><span class="opt-what">Tables together, every ROM in one ROMs folder, every PinUP pack in one PinUP folder: the layout VPX 10.8.0, PinUP Popper and VPin Studio all read. The safe choice if you’ll also play in VPX.</span></span>
                  </label>
                  <label class="opt" class:on={perGame}>
                    <input type="radio" name="layout" checked={perGame} onchange={() => (perGame = true)} />
                    <span><b>A folder for each game</b><span class="opt-what">Each game’s tables in a folder of its own with their ROM, packs, coloring and music: easy to copy, back up or delete one game. Only VPX 10.8.1 and later read it: VPX 10.8.0, and PinUP Popper starting tables through it, can’t start ROM tables kept this way.</span></span>
                  </label>
                  <span class="opt-what">Settings › Library can move a library of shared folders into a folder for each game later, and put it back.</span>
                </div>
              {/if}
            </div>
          </div>
          {#if problem}<p class="problem">{problem}</p>{/if}
          <div class="actions">
            <button type="button" class="btn ghost" onclick={() => (step = 0)}>Back</button>
            <button type="button" class="btn save" disabled={busy || (choice === 'fresh' ? !root.trim() : !folders.tables)} onclick={keepLibrary}>{busy ? 'Setting it up…' : 'Continue'}<Icon name="chevR" size={16} /></button>
          </div>
        {/if}
      </section>
    {:else if step === 2}
      <section>
        <h1 id="fs-title" class="display">Your screens</h1>
        {#if screens.length}
          <ul class="screens">
            {#each screens as s (s.number)}
              <li><Icon name="tv" size={18} /><b>{s.name || `Screen ${s.number}`}</b><span>{s.width} × {s.height}{s.primary ? ' · main' : ''}{s.hdr ? ' · HDR' : ''}</span></li>
            {/each}
          </ul>
        {/if}
        <p class="lead">{screens.length > 1 ? 'With more than one screen, the playfield, backglass, score display and topper each go where you put them.' : 'On one screen, the table plays in a window or full screen, its score display beside it.'} Settings › Screens has ready-made layouts and lets you drag each window into place.</p>
        <label class="check"><input type="checkbox" bind:checked={openScreens} />Open Settings › Screens when setup is done</label>
        <div class="actions">
          <button type="button" class="btn save" onclick={() => (step = 3)}>Continue<Icon name="chevR" size={16} /></button>
        </div>
      </section>
    {:else if step === 3}
      <section>
        <h1 id="fs-title" class="display">Your controls</h1>
        <p class="lead">VPX's keys work as they do in VPX, and so do gamepads and cabinet boards, through VPX's own layouts.</p>
        <ul class="keys">
          <li><kbd>Shift</kbd> flippers</li>
          <li><kbd>Enter</kbd> plunger</li>
          <li><kbd>5</kbd> coin · <kbd>1</kbd> start</li>
          <li><kbd>Z</kbd> <kbd>/</kbd> <kbd>Space</kbd> nudge</li>
          <li><kbd>P</kbd> pause menu: sounds, options, rules, controls</li>
          <li><kbd>Esc</kbd> quit, through the pause menu</li>
          <li><kbd>F12</kbd> screenshot</li>
        </ul>
        <p class="soft">Any action can go on any key or button: Settings › Controls, with a live test of every button, stick and plunger.</p>
        <div class="actions">
          <button type="button" class="btn save" onclick={() => (step = 4)}>Continue<Icon name="chevR" size={16} /></button>
        </div>
      </section>
    {:else}
      <section>
        <h1 id="fs-title" class="display">All set</h1>
        <p class="lead">{choice === 'vpx' ? 'Your tables are being read now: each matched to the Virtual Pinball Spreadsheet, with its pictures and facts.' : 'Your library is ready. Add a table by dropping its download — a file, a folder or an archive — anywhere on the app.'}</p>
        <p class="soft">Settings › General can run this setup again.</p>
        <div class="actions">
          <button type="button" class="btn save" onclick={finish}>{openScreens ? 'Set up the screens' : 'Open the library'}<Icon name="chevR" size={16} /></button>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .cover { position: fixed; inset: 0; z-index: 70; background: var(--bg); display: flex; align-items: center; justify-content: center; padding: 28px; overflow: auto; }
  .box { width: min(760px, 100%); display: flex; flex-direction: column; gap: 28px; }
  .steps { list-style: none; margin: 0; padding: 0; display: flex; gap: 8px; flex-wrap: wrap; }
  .steps li { padding: 6px 12px; border-radius: 999px; font-size: 13px; color: var(--faint); background: var(--control); }
  .steps li.on { color: #fff; background: var(--accent-bg); }
  .steps li.done { color: var(--good); }
  section { display: flex; flex-direction: column; gap: 18px; }
  h1 { margin: 0; font-size: 44px; line-height: 1.05; }
  .lead { margin: 0; color: var(--fg-soft); font-size: 17px; line-height: 1.5; }
  .soft { margin: 0; color: var(--faint); font-size: 14px; }
  .actions { display: flex; gap: 10px; justify-content: flex-end; margin-top: 8px; }
  .save { background: var(--accent-bg); border-color: var(--accent); display: inline-flex; gap: 6px; align-items: center; }
  .choices { display: flex; flex-direction: column; gap: 12px; }
  .choice { display: flex; flex-direction: column; gap: 12px; padding: 16px 18px; border-radius: 14px; border: 1px solid var(--card-line); background: var(--card); }
  .choice.on { border-color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent); }
  .pick { display: flex; gap: 14px; align-items: flex-start; text-align: left; padding: 0; border: 0; background: none; color: var(--fg); font: inherit; cursor: pointer; }
  .radio { flex: 0 0 auto; width: 18px; height: 18px; margin-top: 2px; border-radius: 50%; border: 2px solid var(--control-line); }
  .choice.on .radio { border-color: var(--accent); background: radial-gradient(circle, var(--accent) 45%, transparent 50%); }
  .words { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .words b { font-size: 16px; }
  .words > span { color: var(--fg-soft); font-size: 14px; line-height: 1.45; }
  /* the folders, under the words */
  dl { display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; margin: 0 0 0 32px; font-size: 13px; align-items: baseline; }
  dt { color: var(--faint); }
  dd { margin: 0; display: flex; gap: 12px; align-items: baseline; justify-content: space-between; min-width: 0; }
  .path { color: var(--fg-soft); font-family: ui-monospace, Consolas, monospace; font-size: 12px; overflow-wrap: anywhere; }
  .link { flex: 0 0 auto; padding: 0; border: 0; background: none; color: var(--accent); font: inherit; font-size: 13px; cursor: pointer; }
  .link:hover { text-decoration: underline; }
  .field { display: flex; gap: 8px; margin-left: 32px; }
  .field input { flex: 1; max-width: 420px; height: 36px; }
  .field .btn { display: inline-flex; gap: 6px; align-items: center; }
  /* a new library's layout, under its folder */
  .layout { display: flex; flex-direction: column; gap: 8px; margin-left: 32px; }
  .layout-q { color: var(--fg-soft); font-size: 14px; font-weight: 600; }
  .opt { display: flex; gap: 10px; align-items: flex-start; padding: 10px 12px; border-radius: 10px; border: 1px solid var(--card-line); background: var(--bg); cursor: pointer; }
  .opt.on { border-color: var(--accent); }
  .opt input { margin-top: 3px; accent-color: var(--accent); }
  .opt > span { display: flex; flex-direction: column; gap: 3px; font-size: 14px; }
  .opt-what { color: var(--faint); font-size: 13px; line-height: 1.45; }
  .opt-name { display: flex; align-items: center; gap: 8px; }
  .rec { padding: 1px 7px; border-radius: 999px; background: var(--accent-bg); color: #fff; font-size: 11px; font-weight: 600; }
  .problem { margin: 0; padding: 10px 14px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .screens { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .screens li { display: flex; gap: 10px; align-items: center; padding: 10px 14px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .screens li span { color: var(--muted); font-size: 13px; margin-left: auto; }
  .check { display: inline-flex; gap: 8px; align-items: center; color: var(--fg-soft); }
  .check input { accent-color: var(--accent); }
  .keys { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 10px; }
  .keys li { padding: 10px 14px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); color: var(--fg-soft); font-size: 14px; }
  kbd { display: inline-block; padding: 1px 7px; border-radius: 6px; border: 1px solid var(--control-line); background: var(--control); color: var(--fg); font: 12px ui-monospace, Consolas, monospace; }
</style>
