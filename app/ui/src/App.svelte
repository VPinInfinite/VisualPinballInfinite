<script>
  // The app's window: the top bar, the screen shown, and the notices at its foot.
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { app, vps, drop, dropFiles, addDrop, start, notice, dismiss, dropToast, openTable, testTable, makePictures, remember, goBack, goForward, lookForUpdate, toggleFullscreen, guard, leaving } from './lib/store.svelte.js';
  import { listen } from '@tauri-apps/api/event';

  // the app closed with an achievement pack's changes not saved: asked first
  $effect(() => {
    const off = listen('close-asked', () => leaving(() => invoke('quit'), { closing: true }));
    return () => off.then((f) => f());
  });
  import Drop from './lib/Drop.svelte';
  import TopBar from './lib/TopBar.svelte';
  import Library from './lib/Library.svelte';
  import TablePage from './lib/TablePage.svelte';
  import Settings from './lib/Settings.svelte';
  import VpsBrowser from './lib/VpsBrowser.svelte';
  import Tools from './lib/Tools.svelte';
  import Editor from './lib/Editor.svelte';
  import Hub from './lib/Hub.svelte';
  import ReportProblem from './lib/ReportProblem.svelte';
  import RemoveTable from './lib/RemoveTable.svelte';
  import RatingsAsk from './lib/RatingsAsk.svelte';
  import UnsavedAsk from './lib/UnsavedAsk.svelte';
  import CabinetMode from './lib/CabinetMode.svelte';
  import FirstSetup from './lib/FirstSetup.svelte';
  import Icon from './lib/Icon.svelte';

  onMount(() => {
    start()
      // opened in cabinet mode, as Settings › Cabinet says, but in the test mode
      .then(async () => {
        if (app.prefs.startInCabinet && !(await invoke('test_mode'))) app.cabinet = true;
      })
      .then(testMode)
      .catch((e) => notice(`The library couldn’t be read: ${e}`, { kind: 'problem' }));
    // files dragged onto the window: onto a table's page, they're for that table, and onto its tile
    // too unless the library's settings say otherwise
    const stop = getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      const under = () => {
        if (drop.open) return null;
        if (app.page === 'table') return app.tableId;
        const r = window.devicePixelRatio || 1;
        const el = document.elementFromPoint(p.position.x / r, p.position.y / r);
        // a row naming its table outright (a PinUP pack's, in Settings), whatever the tiles' setting
        const row = el?.closest('[data-drop-table]')?.getAttribute('data-drop-table');
        if (row) return row;
        if (app.prefs.dropOnTiles === false) return null;
        return el?.closest('[data-table]')?.getAttribute('data-table') || null;
      };
      if (p.type === 'enter' || p.type === 'over') {
        drop.dragging = true;
        drop.over = under();
      } else if (p.type === 'leave') {
        drop.dragging = false;
        drop.over = null;
      } else if (p.type === 'drop') {
        const tableId = under();
        drop.dragging = false;
        drop.over = null;
        if (p.paths?.length) dropFiles(p.paths, tableId);
      }
    });
    return () => stop.then((f) => f());
  });

  // each screen shown remembered, for the mouse's back and forward buttons
  $effect(() => remember());

  // the mouse's back and forward buttons (buttons 3 and 4), which the web view would otherwise take
  // for its own history, which the app has none of
  function mouseButton(e) {
    if (e.button !== 3 && e.button !== 4) return;
    e.preventDefault();
    if (e.type === 'mouseup') (e.button === 3 ? goBack : goForward)();
  }

  // F11: the window full screen, or back
  function key(e) {
    if (e.key !== 'F11' || e.repeat) return;
    e.preventDefault();
    toggleFullscreen();
  }

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  async function until(test, ms) {
    for (const end = Date.now() + ms; Date.now() < end && !test(); ) await sleep(200);
  }

  // the app's test mode (vpi-app --shot): the page asked for shown, its picture taken once
  // everything on it has loaded
  async function testMode() {
    const shot = await invoke('test_mode');
    if (!shot) {
      // a new version looked for, quietly, once the library is read (never in the test mode)
      lookForUpdate();
      return;
    }
    await until(() => app.loaded, 180000);
    app.testing = true;
    if (shot.picture) app.prefs.picture = shot.picture;
    if (shot.view) app.prefs.view = shot.view;
    if (shot.pick) app.pick = shot.pick;
    if (shot.page === 'browser' && shot.table) {
      // the browser searched for it, as its search box would
      app.page = 'browser';
      vps.query = shot.table;
    } else if (shot.page === 'table') {
      const want = (shot.table || '').toLowerCase();
      const t = app.tables.find((x) => x.name.toLowerCase().includes(want) || x.file.toLowerCase().includes(want)) || app.tables[0];
      if (t) openTable(t.id, app.tables.map((x) => x.id));
    } else if (shot.page === 'setup') {
      // first setup, as when asked for again
      app.setupAgain = true;
    } else if (shot.page.startsWith('tools/')) {
      // one of the managers: tools/roms and the like
      app.page = 'tools';
      app.toolsPage = shot.page.slice('tools/'.length);
    } else if (shot.page === 'editor/inspect' || shot.page === 'editor/achievements') {
      // the table inspector, or its achievements editor, on the table asked for
      const want = (shot.table || '').toLowerCase();
      const t = app.tables.find((x) => x.name.toLowerCase().includes(want) || x.file.toLowerCase().includes(want)) || app.tables[0];
      app.page = 'editor';
      app.inspectMode = shot.page === 'editor/achievements' ? 'achievements' : 'parts';
      if (t) app.inspectId = t.id;
    } else if (shot.page.startsWith('hub/')) {
      app.page = 'hub';
      app.hubPage = shot.page.slice('hub/'.length);
    } else if (shot.page.startsWith('settings/')) {
      // one settings page: settings/sound and the like
      app.page = 'settings';
      // and a tab of it: settings/library/playlists
      [app.settingsPage, app.settingsTab] = shot.page.slice('settings/'.length).split('/');
    } else {
      app.page = shot.page;
    }
    if (shot.checks) {
      // the background checks begun, then done
      await sleep(1500);
      await until(() => !app.tasks.length, 900000);
    }
    if (shot.test && app.page === 'table') await testTable(app.tableId);
    if (shot.makePictures && app.page === 'table') {
      await makePictures(app.tableId);
      // the library read again with them, then a moment for the page to show them
      await new Promise((r) => setTimeout(r, 1500));
    }
    if (shot.findVps && app.page === 'table') {
      await tick();
      document.querySelector('.link.change, .choose .btn:last-child')?.click();
      await until(() => document.querySelector('.dialog .pick'), 30000);
      document.querySelector('.dialog .pick')?.click();
      await until(() => document.querySelector('.dialog .pick .tags, .dialog .pick.plain'), 30000);
      // its first release picked: the two ways of keeping the match, and the renames the second would do
      document.querySelector('.dialog .list .pick')?.click();
      await until(() => document.querySelector('.dialog .hows .renames, .dialog .hows .what'), 30000);
      await sleep(800);
    }
    if (shot.openMedia && app.page === 'table') {
      await tick();
      [...document.querySelectorAll('.strip .thumb')].find((b) => b.querySelector('.cap')?.textContent.toLowerCase().startsWith(shot.openMedia.toLowerCase()))?.click();
      await sleep(1000);
    }
    if (shot.openStack && app.page === 'table') {
      await tick();
      document.querySelector('.strip .frame.stack')?.closest('button')?.click();
      await sleep(1000);
    }
    if (shot.changeRom && app.page === 'table') {
      await tick();
      [...document.querySelectorAll('.pactions .btn')].find((b) => b.textContent === 'Change')?.click();
      await until(() => !document.querySelector('.dialog[data-loading]'), 60000);
      await sleep(500);
    }
    if (shot.customScreens && app.page === 'table') {
      await tick();
      document.querySelector('[aria-label="Table settings"]')?.click();
      await until(() => [...document.querySelectorAll('.btn')].find((b) => b.textContent === 'Custom…'), 60000);
      [...document.querySelectorAll('.btn')].find((b) => b.textContent === 'Custom…')?.click();
      await until(() => document.querySelector('.side-panel .map'), 60000);
      await sleep(500);
    }
    if (shot.tableSettings && app.page === 'table') {
      await tick();
      document.querySelector('[aria-label="Table settings"]')?.click();
      await tick();
    }
    // cabinet mode first, so a click can be one of its own buttons
    if (shot.cabinet) {
      app.cabinet = true;
      await sleep(1500);
    }
    for (const words of shot.click || []) {
      await tick();
      await sleep(500);
      [...document.querySelectorAll('button')].find((b) => b.textContent.trim().startsWith(words) || b.getAttribute('aria-label')?.startsWith(words))?.click();
      await sleep(1500);
    }
    if (shot.drop?.length) {
      await tick();
      await dropFiles(shot.drop, app.page === 'table' ? app.tableId : null);
      // more dropped onto the window, as the window's own drop would (no table under them)
      if (shot.thenDrop?.length) await dropFiles(shot.thenDrop, null);
      if (shot.add) {
        await addDrop();
        // the library read again with them, scrolled to the one added
        await sleep(2500);
      }
      if (shot.see) {
        await tick();
        document.querySelector('.dl-head .see')?.click();
        await sleep(1500);
      }
    }
    await tick();
    await sleep(300);
    const inView = (el) => {
      const r = el.getBoundingClientRect();
      return shot.full || (r.bottom > 0 && r.top < window.innerHeight);
    };
    // a picture of the whole page: lists that scroll on their own shown whole too
    if (shot.full) document.documentElement.dataset.whole = '';
    if (shot.full) for (const img of document.images) img.loading = 'eager';
    await until(() => [...document.images].filter(inView).every((i) => i.complete), 60000);
    // a page still asking the engine side for what it shows (the Screens page asks the player)
    await until(() => !document.querySelector('[data-loading]'), 30000);
    await sleep(600);
    await invoke('settled');
  }
</script>

<svelte:window onmousedown={mouseButton} onmouseup={mouseButton} onkeydown={key} />

<div class="window">
  <TopBar />
  <div class="screen">
    {#if app.page === 'table'}
      <TablePage />
    {:else if app.page === 'browser'}
      <VpsBrowser />
    {:else if app.page === 'settings'}
      <Settings />
    {:else if app.page === 'tools'}
      <Tools />
    {:else if app.page === 'editor'}
      <Editor />
    {:else if app.page === 'hub' || app.page === 'stats'}
      <Hub />
    {:else}
      <Library />
    {/if}
  </div>
  {#if drop.open}<Drop />{/if}
  {#if app.report}<ReportProblem table={app.report.table} onclose={() => (app.report = null)} />{/if}
  {#if app.removing}<RemoveTable t={app.removing} onclose={() => (app.removing = null)} />{/if}
  {#if app.askRatings}<RatingsAsk />{/if}
  {#if guard.ask}<UnsavedAsk />{/if}
  {#if app.cabinet}<CabinetMode />{/if}
  <!-- first setup: the settings name no library yet, or asked for again; it stays until it's done -->
  {#if app.status.phase === 'setup' || app.setupAgain || app.setupOpen}<FirstSetup again={app.setupAgain} onclose={() => ((app.setupAgain = false), (app.setupOpen = false))} />{/if}
  {#if drop.dragging && !drop.open}
    <div class="dragging" aria-hidden="true">
      <span>{drop.over ? `Drop to add them to ${app.tables.find((t) => t.id === drop.over)?.name || 'this table'}` : app.page === 'table' ? 'Drop to add them to this table' : 'Drop to add them'}</span>
    </div>
  {/if}
  {#if app.toasts.length}
    <!-- what was added, at the top right, fading by itself; held while the pointer is on it -->
    <div class="toasts" role="status" aria-live="polite">
      {#each app.toasts as t (t.id)}
        <button type="button" class="toast" onanimationend={() => dropToast(t.id)} onclick={() => dropToast(t.id)} title="Close">
          <span class="ticked"><Icon name="check" size={16} width={2.6} /></span>
          <span class="words">
            <b>{t.title}</b>
            {#each t.lines as l (l)}<span>{l}</span>{/each}
          </span>
        </button>
      {/each}
    </div>
  {/if}
  {#if app.notices.length}
    <div class="notices" role="status" aria-live="polite">
      {#each app.notices as n (n.id)}
        <div class="notice {n.kind}">
          <span class="text">{n.text}</span>
          {#if n.action}
            <button type="button" class="btn small" onclick={() => n.action.run()}>{n.action.label}</button>
          {/if}
          <button type="button" class="btn small ghost close" aria-label="Close" onclick={() => dismiss(n.id)}><Icon name="x" size={16} /></button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .window { min-height: 100%; display: flex; flex-direction: column; }
  .screen { flex: 1; display: flex; flex-direction: column; min-height: 0; }
  .notices {
    position: fixed; right: 20px; bottom: 20px; z-index: 50;
    display: flex; flex-direction: column; gap: 10px; max-width: min(520px, calc(100vw - 40px));
  }
  .notice {
    display: flex; align-items: center; gap: 12px; padding: 12px 12px 12px 16px;
    border-radius: 12px; background: #182031; border: 1px solid #2a3550; box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5);
  }
  .toasts {
    position: fixed; top: 74px; right: 20px; z-index: 60;
    display: flex; flex-direction: column; gap: 10px; width: min(420px, calc(100vw - 40px));
  }
  .toast {
    display: flex; gap: 12px; align-items: flex-start; padding: 12px 16px; text-align: left; color: var(--fg); font: inherit;
    border-radius: 12px; background: #142a20; border: 1px solid #24533b; box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5);
    animation: toast 6s ease forwards; cursor: pointer;
  }
  .toast:hover { animation-play-state: paused; }
  .toast .ticked { flex: 0 0 auto; display: inline-flex; color: var(--good); margin-top: 2px; }
  .toast .words { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .toast .words b { font-size: 14px; }
  .toast .words span { font-size: 13px; color: var(--fg-soft); overflow-wrap: anywhere; }
  @keyframes toast {
    0% { opacity: 0; transform: translateY(-8px); }
    5% { opacity: 1; transform: none; }
    65% { opacity: 1; }
    100% { opacity: 0; }
  }
  .notice.problem { background: #2a1820; border-color: #5a2a3a; }
  .notice.done { background: #142a20; border-color: #24533b; }
  .notice .text { flex: 1; }
  .dragging {
    position: fixed; inset: 0; z-index: 45; pointer-events: none;
    border: 3px dashed var(--accent); background: rgba(13, 30, 46, 0.35);
    display: flex; align-items: flex-end; justify-content: center; padding-bottom: 40px;
  }
  .dragging span { padding: 12px 18px; border-radius: 12px; background: #10263a; border: 1px solid #1f4a6e; font-weight: 600; }
  .close { width: 32px; padding: 0; }
</style>
