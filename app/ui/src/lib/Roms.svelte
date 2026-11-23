<script>
  // Tools › ROMs: every ROM set the library has or its tables ask for (`rom_list`), checked file
  // by file against PinMAME's own list, with the tables that play each; a set's files on its row
  // (`rom_files`), its zip shown in Explorer (`rom_show`); and the fixes that fit it — a misnamed
  // set renamed, a second copy removed, a set no table plays removed — each asked first on its row
  // (`rom_fix`), what's removed going to the Recycle Bin. Read again whenever the library is.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import SearchBox from './SearchBox.svelte';
  import { app, drop, notice, openTable } from './store.svelte.js';

  let rows = $state(null);
  let problem = $state('');
  // which sets show: all, those needing a look, those no table plays, those not installed
  let showing = $state('all');
  // the zip whose files show, and its files (or why they couldn't be read)
  let open = $state(null);
  let files = $state(null);
  // the fix asked about: { r, fix }
  let asking = $state(null);
  let working = $state(false);
  // the search: a set's name, its game, or a table that plays it
  let query = $state('');

  async function load() {
    try {
      rows = await invoke('rom_list');
      problem = '';
    } catch (e) {
      problem = String(e);
    }
  }
  $effect(() => {
    load();
    const off = listen('library-changed', () => load());
    return () => off.then((f) => f());
  });

  const needsLook = (r) => r.zip && r.look !== 'good';
  const notInstalled = (r) => r.status === 'not installed';
  const groups = $derived(
    rows
      ? [
          ['all', 'All', rows],
          ['look', 'Needs a look', rows.filter(needsLook)],
          ['unused', 'No table plays it', rows.filter((r) => r.unused)],
          ['missing', 'Not installed', rows.filter(notInstalled)],
        ]
      : [],
  );
  const matches = (r) => {
    const q = query.trim().toLowerCase();
    if (!q) return true;
    const names = [r.set, r.zip && stemOf(r.zip), r.description, r.holds, ...[...r.usedBy, ...r.parentOf, ...r.otherVersionOf.map(([p]) => p)].map((p) => tableAt(p)?.name ?? stemOf(p))];
    return names.some((n) => n && n.toLowerCase().includes(q));
  };
  const shown = $derived((groups.find(([k]) => k === showing)?.[2] ?? []).filter(matches));

  // a table by its file's path, as the library knows it
  const tableAt = (path) => app.tables.find((t) => t.path.toLowerCase() === path.toLowerCase());
  const fileName = (p) => p.split(/[\\/]/).pop();
  const stemOf = (p) => fileName(p).replace(/\.zip$/i, '');
  const size = (n) => (n < 1000 ? `${n} bytes` : n < 1e6 ? `${Math.round(n / 1000)} KB` : `${(n / 1e6).toFixed(n < 1e7 ? 1 : 0)} MB`);

  // a misnamed zip whose rightly named twin is there already, the same files: a spare copy
  const copyOf = (r) => r.fixes.find((f) => f.kind === 'removeCopy');

  function status(r) {
    if (r.status === 'misnamed' && copyOf(r)) return ['spare', `Extra copy of ${r.holds}`];
    switch (r.status) {
      case 'good':
        return r.unused && r.otherVersionOf.length ? ['spare', 'Spare version'] : ['good', 'Good'];
      case 'files missing':
        return ['bad', 'Files missing'];
      case 'wrong files':
        return ['bad', 'Wrong files'];
      case 'needs its parent':
        return ['warn', `Needs ${r.parent}`];
      case 'misnamed':
        return ['warn', `Named wrongly: it’s ${r.holds}`];
      case 'unknown':
        return ['warn', 'Holds no ROM PinMAME knows'];
      case 'unreadable':
        return ['bad', 'Can’t be read'];
      default:
        return ['bad', 'Not installed'];
    }
  }
  const ICON = { good: 'check', spare: 'minus', warn: 'alert', bad: 'alert' };

  const FILE = {
    good: ['good', 'Right'],
    'from its parent': ['good', 'From its parent’s zip'],
    'in its parent': ['warn', 'In its parent’s zip, which isn’t here'],
    missing: ['bad', 'Missing'],
    wrong: ['bad', 'Another file under its name'],
    optional: ['spare', 'Not there; it plays without it'],
    'not needed': ['spare', 'Not this set’s'],
  };

  async function toggleFiles(r) {
    if (open === r.zip) {
      open = null;
      return;
    }
    open = r.zip;
    files = null;
    try {
      files = await invoke('rom_files', { zip: r.zip });
    } catch (e) {
      files = String(e);
    }
  }

  const show = (r) => invoke('rom_show', { zip: r.zip }).catch((e) => notice(String(e), { kind: 'problem' }));

  const fixLabel = (f) => (f.kind === 'rename' ? `Rename it ${stemOf(f.to)}…` : f.kind === 'removeCopy' ? 'Remove this extra copy…' : 'Remove…');

  // what a fix does, asked before it's done
  function question(r, f) {
    const zip = fileName(r.zip);
    if (f.kind === 'rename') return `Rename ${zip} to ${fileName(f.to)}, the name PinMAME looks for? Its files are ${r.holds}’s, so tables asking for ${r.holds} play it after.`;
    if (f.kind === 'removeCopy') return `${zip} holds the same files as ${fileName(f.of)}, which stays and is the one PinMAME plays. Move this extra copy to the Recycle Bin?`;
    if (r.status === 'unknown') return `${zip} holds no ROM PinMAME knows. Move it to the Recycle Bin?`;
    const others = r.otherVersionOf.map(([p, set]) => `${tableAt(p)?.name ?? stemOf(p)} plays ${set}`).join('; ');
    return `No table plays ${r.set}${others ? ` (${others})` : ''}. Move ${zip} to the Recycle Bin? Keep it if a table you’ll add needs it.`;
  }
  const doLabel = (f) => (f.kind === 'rename' ? 'Rename it' : f.kind === 'removeCopy' ? 'Remove this extra copy' : 'Remove it');

  async function carryOut(r, f) {
    working = true;
    try {
      await invoke('rom_fix', { zip: r.zip, fix: f.kind });
      notice(f.kind === 'rename' ? `${fileName(r.zip)} is now ${fileName(f.to)}.` : `${fileName(r.zip)} went to the Recycle Bin.`, { kind: 'done' });
      asking = null;
      if (open === r.zip) open = null;
      await load();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      working = false;
    }
  }
</script>

{#snippet tables(paths)}
  {#each paths as p, i (p)}{@const t = tableAt(p)}{i ? ', ' : ''}{#if t}<button type="button" class="link" onclick={() => openTable(t.id)}>{t.name}</button>{:else}{stemOf(p)}{/if}{/each}
{/snippet}

{#snippet row(r)}
  {@const [look, words] = status(r)}
  {@const dropFor = r.usedBy.length ? tableAt(r.usedBy[0])?.id : undefined}
  <div class="set" class:opened={open && open === r.zip} class:over={dropFor && drop.dragging && drop.over === dropFor} data-drop-table={dropFor}>
    <span class="sname">
      <b title={r.zip ?? ''}>{r.zip ? stemOf(r.zip) : r.set}</b>
      {#if r.description}<span class="sub">{r.description}</span>{:else if copyOf(r)}<span class="sub">the same files as {fileName(copyOf(r).of)}</span>{:else if r.holds}<span class="sub">holds {r.holds}’s files</span>{/if}
    </span>
    <span class="sstate {look}" title={r.text}><Icon name={ICON[look]} size={15} width={2.4} />{words}</span>
    <span class="splayed">
      {#if r.usedBy.length}
        {@render tables(r.usedBy)}
      {:else if r.parentOf.length}
        {@render tables(r.parentOf)}<span class="sub">{r.parentOf.length === 1 ? 'plays a version' : 'play versions'} that take{r.parentOf.length === 1 ? 's' : ''} files from it</span>
      {:else if r.otherVersionOf.length}
        {#each r.otherVersionOf as [p, set] (p)}<span class="line">{@render tables([p])} plays {set}</span>{/each}
      {:else}
        <span class="faint">No table</span>
      {/if}
    </span>
    <span class="ssize">{r.zip ? size(r.size) : '—'}</span>
    <span class="sactions">
      {#if r.zip}<button type="button" class="btn small" aria-expanded={open === r.zip} onclick={() => toggleFiles(r)}>Files</button>{/if}
      {#each r.fixes as f (f.kind)}
        <button type="button" class="btn small" onclick={() => (asking = { r, f })}>{fixLabel(f)}</button>
      {/each}
      {#if !r.zip && r.usedBy.length && tableAt(r.usedBy[0])}
        <button type="button" class="btn small" title="Its page links to VPS, where its ROM is listed" onclick={() => openTable(tableAt(r.usedBy[0]).id)}>Its table</button>
      {/if}
    </span>
    {#if copyOf(r)}
      <p class="snote">{fileName(copyOf(r).of)}, with the same files, is already there, and it’s the one PinMAME plays: this copy is never used. Nothing needs renaming; remove this one to tidy up (it goes to the Recycle Bin).</p>
    {:else if look === 'warn' || look === 'bad'}<p class="snote {look}">{r.text}</p>{/if}
    {#if asking?.r === r}
      <div class="sask">
        <span>{question(r, asking.f)}</span>
        <button type="button" class="btn small" disabled={working} onclick={() => carryOut(r, asking.f)}>{doLabel(asking.f)}</button>
        <button type="button" class="btn small ghost" disabled={working} onclick={() => (asking = null)}>Cancel</button>
      </div>
    {/if}
    {#if open && open === r.zip}
      <div class="files">
        <div class="files-head">
          <span>{fileName(r.zip)}, checked against PinMAME’s list of {r.holds ?? r.set}’s files</span>
          <button type="button" class="btn small ghost" onclick={() => show(r)}>Show in Explorer</button>
        </div>
        {#if files === null}
          <p class="faint">Reading its files…</p>
        {:else if typeof files === 'string'}
          <p class="snote bad">{files}</p>
        {:else}
          <table>
            <thead><tr><th>File</th><th>Size</th><th>Checksum</th><th></th></tr></thead>
            <tbody>
              {#each files as f, i (i)}
                {@const [fl, fw] = FILE[f.status] ?? ['spare', f.status]}
                <tr><td class="mono">{f.name}</td><td>{size(f.size)}</td><td class="mono faint">{f.crc}</td><td class="fstate {fl}">{fw}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    {/if}
  </div>
{/snippet}

<div class="page" data-loading={rows || problem ? undefined : ''}>
  <header class="title">
    <h1 class="display">ROMs</h1>
    <p>Every ROM set your tables have or need, checked file by file against PinMAME’s own list. Fixes only ever ask first, and what’s removed goes to the Recycle Bin.</p>
  </header>

  {#if problem}
    <p class="problem">{problem} <button type="button" class="btn small" onclick={load}>Try again</button></p>
  {:else if !rows}
    <p class="note">Checking every set…</p>
  {:else if !rows.length}
    <p class="note">No ROM sets yet, and no table asks for one. A ROM dropped onto the app goes where PinMAME looks for it, checked first.</p>
  {:else}
    <div class="bar">
      <div class="seg" role="radiogroup" aria-label="Which sets show">
        {#each groups as [key, label, list] (key)}
          <button type="button" role="radio" aria-checked={showing === key} onclick={() => (showing = key)}>{label} <span class="n">{list.length}</span></button>
        {/each}
      </div>
      <SearchBox bind:value={query} placeholder="Search by table, set or game" />
    </div>
    {#if !shown.length}
      <p class="note">{query.trim() ? `No set here matches “${query.trim()}”.` : showing === 'look' ? 'Every set is as PinMAME expects it.' : showing === 'unused' ? 'Every set is played by a table.' : 'Every ROM your tables ask for is here.'}</p>
    {:else}
      <div class="scroll">
        <div class="head" aria-hidden="true"><span>Set</span><span>Status</span><span>Played by</span><span>Size</span><span></span></div>
        <div class="list">{#each shown as r (r.zip ?? `asked:${r.set}`)}{@render row(r)}{/each}</div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 18px; max-width: 2200px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px 16px; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .note { margin: 0; color: var(--faint); font-size: 13px; max-width: 860px; line-height: 1.5; }
  .problem { margin: 0; padding: 12px 14px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; }
  /* All, Needs a look, No table plays it, Not installed */
  .seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg .n { margin-left: 4px; color: var(--faint); font-size: 12px; }
  .seg button[aria-checked='true'] .n { color: inherit; opacity: 0.75; }
  /* the list scrolls on its own, its headings kept in view */
  .scroll { max-height: max(360px, calc(100vh - 300px)); overflow: auto; padding-right: 6px; border-radius: 12px; }
  :global(html[data-whole]) .scroll { max-height: none; }
  .head, .set { display: grid; grid-template-columns: minmax(150px, 1fr) 200px minmax(0, 1.3fr) 72px 260px; gap: 6px 14px; align-items: center; }
  .head { position: sticky; top: 0; z-index: 1; padding: 8px 12px; background: var(--bg); font-size: 12px; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .set { padding: 10px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .set.over { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, var(--card)); }
  .set.opened { border-color: var(--control-line); }
  .sname { min-width: 0; }
  .sname b { display: block; font: 600 13px ui-monospace, Consolas, monospace; color: var(--fg); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { display: block; color: var(--faint); font-size: 12px; margin-top: 2px; }
  .faint { color: var(--faint); }
  .sstate { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; min-width: 0; }
  .sstate.good { color: var(--good); }
  .sstate.spare { color: var(--faint); font-weight: 500; }
  .sstate.warn { color: var(--warn); }
  .sstate.bad { color: var(--bad); }
  .splayed { font-size: 14px; color: var(--fg-soft); min-width: 0; }
  .splayed .line { display: block; color: var(--faint); font-size: 13px; }
  .ssize { font-size: 13px; color: var(--fg-soft); white-space: nowrap; }
  .sactions { display: flex; flex-wrap: wrap; gap: 6px; justify-content: flex-end; }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; text-align: left; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
  .snote { grid-column: 1 / -1; margin: 0; font-size: 13px; line-height: 1.45; }
  .snote.warn { color: var(--warn); }
  .snote.bad { color: var(--bad); }
  .sask { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); }
  .sask span { flex: 1 1 260px; font-size: 13px; color: var(--fg-soft); }
  /* a set's files, under its row */
  .files { grid-column: 1 / -1; display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; border-radius: 8px; background: var(--bg); border: 1px solid var(--card-line); }
  .files-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .files table { width: 100%; border-collapse: collapse; font-size: 13px; }
  .files th { text-align: left; font-weight: 500; color: var(--faint); font-size: 12px; padding: 4px 8px; border-bottom: 1px solid var(--line); }
  .files td { padding: 5px 8px; border-bottom: 1px solid var(--line); color: var(--fg-soft); }
  .mono { font-family: ui-monospace, Consolas, monospace; }
  .fstate { font-weight: 600; }
  .fstate.good { color: var(--good); }
  .fstate.warn { color: var(--warn); }
  .fstate.bad { color: var(--bad); }
  .fstate.spare { color: var(--faint); font-weight: 500; }
  @media (max-width: 1100px) {
    .head { display: none; }
    .set { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
    .sactions { grid-column: 1 / -1; justify-content: flex-start; }
  }
</style>
