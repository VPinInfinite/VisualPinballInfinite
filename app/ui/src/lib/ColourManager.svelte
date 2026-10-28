<script>
  // Tools › Color DMD: every ROM table's coloring at once — a Serum or PIN2DMD file the player plays
  // over its machine's score display, as VPX's plugins find it — which have one, which VPS lists one
  // for, each one's file shown or removed (asked first, to the Recycle Bin, `remove`), and VPS's list
  // of them opened on a table's page; beside each, what's recorded of its machine for coloring it
  // (`colour_recordings_all`, its folder shown, `colour_show`, and written out for ColorizingDMD,
  // `colour_for_colorizingdmd`). Searched by table or ROM; a file dropped on a row is for its table.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import ColourIt from './ColourIt.svelte';
  import SearchBox from './SearchBox.svelte';
  import { app, drop, notice, openTable, colourPrograms, loadColourPrograms, openColourProgram } from './store.svelte.js';

  // the coloring programs, other people's: Install the first time, then Open
  $effect(() => {
    loadColourPrograms();
  });
  const program = (key) => colourPrograms.list?.find((p) => p.key === key);
  const pc = $derived(program('colorizingdmd'));
  const pp = $derived(program('ppuc'));
  const programLabel = (p) => (!p ? '…' : colourPrograms.installing === p.key ? `Installing ${p.key === 'ppuc' ? 'PPUC' : p.name}…` : !p.released && !p.installed ? 'PPUC: not released yet' : `${p.installed ? 'Open' : 'Install'} ${p.key === 'ppuc' ? 'PPUC' : p.name}`);
  // ColorizingDMD opened with the table's recordings written out for it first, where it has any
  async function openColorizing(t) {
    if (recordedOf(t)) await forColorizingDmd(t);
    await openColourProgram(program('colorizingdmd'));
  }

  let showing = $state('all');
  // the table whose display is being coloured: the Colour it… window
  let colouring = $state(null);
  // the colouring asked about removing: { t, item }
  let asking = $state(null);

  const byName = (a, b) => a.name.localeCompare(b.name);
  const roms = $derived(app.tables.filter((t) => t.rom && !t.hiddenTable).sort(byName));
  const groups = $derived([
    ['all', 'All ROM tables', roms],
    ['has', 'Colored', roms.filter((t) => t.holds.colour)],
    ['none', 'Not colored', roms.filter((t) => !t.holds.colour)],
    ['vps', 'VPS lists one', roms.filter((t) => !t.holds.colour && t.vpsHas?.colour)],
  ]);
  // the search: a table's name or its ROM
  let query = $state('');
  const matches = (t) => {
    const q = query.trim().toLowerCase();
    return !q || [t.name, t.rom].some((n) => n && n.toLowerCase().includes(q));
  };
  const shown = $derived((groups.find(([k]) => k === showing)?.[2] ?? []).filter(matches));

  // what's recorded of each machine, by its ROM
  let recorded = $state({});
  async function loadRecorded() {
    recorded = await invoke('colour_recordings_all').catch(() => ({}));
  }
  $effect(() => {
    loadRecorded();
    const offs = [listen('library-changed', loadRecorded), listen('colour-recorded', loadRecorded)];
    return () => offs.forEach((o) => o.then((f) => f()));
  });
  const recordedOf = (t) => recorded[t.rom?.toLowerCase()];
  const showRecordings = (t) => invoke('colour_show', { id: t.id }).catch((e) => notice(String(e), { kind: 'problem' }));
  async function forColorizingDmd(t) {
    try {
      await invoke('colour_for_colorizingdmd', { id: t.id });
      notice(`${t.rom}.txt is ready for ColorizingDMD: its folder is open.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  const coloured = $derived(groups[1][2].length);

  const folderName = (p) => p.split(/[\\/]/).pop();
  // where its colouring is kept: beside the table in its own folder, or in VPinMAME's
  const where = (t) => (t.holds.colour.folder.toLowerCase().includes('\\vpinmame\\') ? 'VPinMAME’s altcolor folder' : 'the table’s own folder');

  const show = (t) => invoke('show_file', { id: t.id, what: 'colour' }).catch((e) => notice(String(e), { kind: 'problem' }));

  // VPS's colour files for its game, on the table's page
  function find(t) {
    app.findOnOpen = 'colour';
    openTable(t.id);
  }

  async function askRemove(t) {
    try {
      const item = (await invoke('remove_plan', { id: t.id })).find((i) => i.key === 'colour');
      if (item) asking = { t, item };
      else notice('It isn’t there any more.', { kind: 'problem' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function remove() {
    const { t, item } = asking;
    asking = null;
    try {
      const done = await invoke('remove', { id: t.id, keys: [item.key] });
      notice(done.problems.length ? done.problems.join(' ') : `${t.name}’s coloring went to the Recycle Bin.`, { kind: done.problems.length ? 'problem' : 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
</script>

<div class="page">
  <header class="title">
    <h1 class="display">Color DMD</h1>
    <p>A coloring turns a machine’s one-color score display into a colored one, scene by scene. Here are every ROM table’s, as the player finds them — Serum’s files and PIN2DMD’s, beside the table or in VPinMAME’s altcolor folder — and what you’ve recorded of each machine to color it yourself.</p>
  </header>

  {#if !roms.length}
    <p class="note">No table plays a ROM machine yet.</p>
  {:else}
    <p class="note">{coloured} of {roms.length} ROM tables have a coloring. A coloring you’ve downloaded or made: drop it on its table’s row here, or on its page.</p>
    <div class="bar">
      <div class="seg" role="radiogroup" aria-label="Which tables show">
        {#each groups as [key, label, list] (key)}
          <button type="button" role="radio" aria-checked={showing === key} onclick={() => (showing = key)}>{label} <span class="n">{list.length}</span></button>
        {/each}
      </div>
      <SearchBox bind:value={query} placeholder="Search by table or ROM" />
    </div>
    {#if !shown.length}
      <p class="note">{query.trim() ? `No table here matches “${query.trim()}”.` : showing === 'has' ? 'None has a coloring yet.' : showing === 'vps' ? 'VPS lists none for the tables without one.' : 'Every ROM table has one.'}</p>
    {:else}
      <div class="scroll">
        <div class="head" aria-hidden="true"><span>Table</span><span>ROM</span><span>Your recordings</span><span>Coloring it plays</span><span></span></div>
        <div class="list">
          {#each shown as t (t.id)}
            {@const rec = recordedOf(t)}
            <div class="row" class:over={drop.dragging && drop.over === t.id} data-drop-table={t.id}>
              <span class="tname"><button type="button" class="link" onclick={() => openTable(t.id)}>{t.name}</button>{#if t.maker || t.year}<span class="sub">{[t.maker, t.year].filter(Boolean).join(' · ')}</span>{/if}</span>
              <span class="mono">{t.rom}</span>
              <span class="state">
                {#if rec}
                  <button type="button" class="filelink" title="Show them in Explorer" onclick={() => showRecordings(t)}><Icon name="folder" size={14} />{rec.count} {rec.count === 1 ? 'recording' : 'recordings'}</button>
                  <span class="sub">{rec.frames ? `${rec.frames.toLocaleString()} frames · ` : ''}<button type="button" class="minilink" onclick={() => (colouring = t)}>Record more…</button></span>
                {:else}
                  <button type="button" class="minilink" onclick={() => (colouring = t)}>Record…</button>
                {/if}
              </span>
              <span class="state">
                {#if t.holds.colour}
                  <button type="button" class="filelink good" title="Show it in Explorer" onclick={() => show(t)}><Icon name="check" size={15} width={2.4} />{t.holds.colour.kind}</button><span class="sub" title={t.holds.colour.folder}>{folderName(t.holds.colour.folder)}, in {where(t)}</span>
                {:else if t.vpsHas?.colour}
                  <span class="faint">None · VPS lists {t.vpsHas.colour}</span>
                {:else}
                  <span class="faint">None</span>
                {/if}
              </span>
              <span class="actions">
                <button type="button" class="btn small" disabled={!pc || !!colourPrograms.installing} title="A coloring program by other people, not ours{rec ? `: its recordings written out as ${t.rom}.txt first, the file it opens` : ''}" onclick={() => openColorizing(t)}><Icon name="ext" size={13} />{programLabel(pc)}</button>
                <button type="button" class="btn small" disabled={!pp || !!colourPrograms.installing || (!pp.released && !pp.installed)} title={pp && !pp.released && !pp.installed ? 'PPUC’s coloring program isn’t released yet' : 'A coloring program by other people, not ours'} onclick={() => openColourProgram(pp)}><Icon name="ext" size={13} />{programLabel(pp)}</button>
                {#if t.holds.colour}
                  <button type="button" class="btn small bin" aria-label="Remove {t.name}’s coloring…" title="Move it to the Recycle Bin" onclick={() => askRemove(t)}><Icon name="trash" size={15} /></button>
                {:else if t.vpsHas?.colour}
                  <button type="button" class="btn small" onclick={() => find(t)}>Find on VPS</button>
                {/if}
              </span>
              {#if asking?.t === t}
                <div class="ask">
                  <span>Move {t.name}’s coloring ({asking.item.name}) to the Recycle Bin? Its score display plays in its own color after.{asking.item.shared.length ? ` Also used by ${asking.item.shared.join(', ')}.` : ''}</span>
                  <button type="button" class="btn small" onclick={remove}>Remove it</button>
                  <button type="button" class="btn small ghost" onclick={() => (asking = null)}>Cancel</button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>

{#if colouring}
  <ColourIt t={colouring} onclose={() => (colouring = null)} />
{/if}

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 18px; max-width: 2200px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px 16px; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .note { margin: 0; color: var(--faint); font-size: 13px; max-width: 860px; line-height: 1.5; }
  .seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg .n { margin-left: 4px; color: var(--faint); font-size: 12px; }
  .seg button[aria-checked='true'] .n { color: inherit; opacity: 0.75; }
  .scroll { max-height: max(360px, calc(100vh - 300px)); overflow: auto; padding-right: 6px; border-radius: 12px; }
  :global(html[data-whole]) .scroll { max-height: none; }
  .head, .row { display: grid; grid-template-columns: minmax(0, 1.4fr) 120px minmax(0, 1fr) minmax(0, 1.2fr) 420px; gap: 6px 14px; align-items: center; }
  .minilink { padding: 0; border: 0; background: none; color: var(--accent-soft, var(--accent)); font: inherit; font-size: 12px; cursor: pointer; text-decoration: underline; text-underline-offset: 2px; }
  .head { position: sticky; top: 0; z-index: 1; padding: 8px 12px; background: var(--bg); font-size: 12px; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .row { padding: 10px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .row.over { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, var(--card)); }
  .filelink { display: inline-flex; align-items: center; gap: 6px; padding: 0; border: 0; background: none; color: var(--fg); font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; }
  .filelink.good { color: var(--good); }
  .filelink:hover { color: var(--accent); }
  .tname { min-width: 0; font-size: 14px; }
  .sub { display: block; color: var(--faint); font-size: 12px; margin-top: 2px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mono { font: 13px ui-monospace, Consolas, monospace; color: var(--fg-soft); }
  .state { min-width: 0; font-size: 13px; }
  .good { display: inline-flex; align-items: center; gap: 6px; color: var(--good); font-weight: 600; }
  .faint { color: var(--faint); }
  .actions { display: flex; gap: 6px; justify-content: flex-end; }
  .bin { padding: 0 8px; }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; text-align: left; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
  .ask { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); }
  .ask span { flex: 1 1 260px; font-size: 13px; color: var(--fg-soft); }
</style>
