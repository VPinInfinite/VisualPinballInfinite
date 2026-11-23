<script>
  // Tools › PinUP packs: the packs themselves: each one, the tables it plays for, its
  // layout and its status (`pup_packs`), in a list that scrolls on its own. Layout… chooses another,
  // Files opens its folder, VPS opens its game's page there, Unpack… hands a zipped pack to the drop
  // window, and any pack can go to the Recycle Bin, asked first, saying which tables play it. A
  // pack no table plays names the game its name points to on VPS, a possible match, and one renamed
  // here for a table says so, with Undo. Each button keeps its own place on every row. Files dropped
  // onto a row are for its table, as onto a table's page. Read again whenever the library is.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import PupLayout from './PupLayout.svelte';
  import SearchBox from './SearchBox.svelte';
  import { drop, dropFiles, notice, openLink, openTable } from './store.svelte.js';

  let packs = $state(null);
  let problem = $state('');
  // which packs show: all, those a table plays (used), or those none plays (unused)
  let showing = $state('all');
  // the pack whose Layout window shows, the one asked about removing, and one asked about renaming
  // for a table: { p, table, to }
  let choosing = $state(null);
  let asking = $state(null);
  let renaming = $state(null);

  async function load() {
    try {
      packs = await invoke('pup_packs');
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

  const byTable = (a, b) => a.tables[0][1].localeCompare(b.tables[0][1]);
  // the search: a pack's name, or a table it plays for or might
  let query = $state('');
  const matches = (p) => {
    const q = query.trim().toLowerCase();
    if (!q) return true;
    return [p.name, ...p.tables.map(([, n]) => n), ...(p.gameTables || []).map((g) => g.name)].some((n) => n && n.toLowerCase().includes(q));
  };
  const usedAll = $derived(packs ? packs.filter((p) => p.tables.length).sort(byTable) : []);
  const unusedAll = $derived(packs ? packs.filter((p) => !p.tables.length) : []);
  const used = $derived(usedAll.filter(matches));
  const unused = $derived(unusedAll.filter(matches));
  const unchosen = $derived(usedAll.filter((p) => p.facts.setup === 'unchosen').length);
  const wrong = $derived(usedAll.filter((p) => p.facts.look === 'bad' && p.facts.setup !== 'unchosen').length);

  const ICON = { good: 'check', warn: 'alert', bad: 'alert', none: 'minus' };
  const layoutOf = (p) => (p.facts.layout ? p.facts.layout : { unchosen: 'Not chosen yet', own: 'Its own setup', none: '—' }[p.facts.setup] ?? '—');
  const titleOf = (p) => (p.tables.length ? p.tables[0][1] : p.name);

  function files(p) {
    invoke('pup_pack_show', { path: p.path, file: null }).catch((e) => notice(String(e), { kind: 'problem' }));
  }

  async function rename(r) {
    renaming = null;
    try {
      await invoke('pup_pack_rename', { path: r.p.path, to: r.to });
      notice(`${r.p.name} is now ${r.to}, so your ${r.table} plays it. Undo it on its row if it doesn’t fit.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function undoRename(p) {
    try {
      await invoke('pup_pack_undo_rename', { path: p.path });
      notice(`${p.name} is ${p.renamedFrom} again.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // what removing a pack means, asked before it goes
  const names = (p) => p.tables.map(([, n]) => n).join(', ');
  const removing = (p) =>
    p.popper
      ? 'PinUP Popper uses this pack for its own menu. Are you sure?'
      : p.facts.look === 'none'
        ? 'It’s an empty folder, with nothing to play. Delete it?'
        : p.tables.length
          ? `This pack is used by ${names(p)}. Are you sure? ${p.tables.length === 1 ? 'It plays' : 'They play'} without it after.`
          : 'This PinUP pack is unused. Delete it?';

  async function remove(p) {
    asking = null;
    try {
      await invoke('pup_pack_remove', { path: p.path });
      notice(`${p.name} went to the Recycle Bin.`, { kind: 'done' });
      load();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
</script>

{#snippet row(p)}
  <div class="pack" class:over={drop.dragging && p.dropTable && drop.over === p.dropTable} data-drop-table={p.dropTable ?? undefined}>
    <span class="pname" title={p.path}>{p.name}{#if p.renamedFrom}<span class="renamed">renamed from {p.renamedFrom} <button type="button" class="link small" onclick={() => undoRename(p)}>Undo</button></span>{/if}</span>
    <span class="ptables">
      {#if p.popper}
        <span class="faint">PinUP Popper’s menu</span>
      {:else if p.tables.length}
        {#each p.tables as [id, name], i (id)}{i ? ', ' : ''}<button type="button" class="link" onclick={() => openTable(id)}>{name}</button>{/each}
        {#if p.misnamed}<span class="sub warnword">named wrong for it: rename it on its page</span>{/if}
      {:else if p.game || p.gameTables.length}
        <span class="faint">Possible table match:</span>
        {#if p.game}<button type="button" class="link" title="Its page on VPS" onclick={() => openLink(p.game.url)}>{p.game.name}</button>{/if}
        {#each p.gameTables as g (g.id)}
          <span class="sub">Your <button type="button" class="link" onclick={() => openTable(g.id)}>{g.name}</button> {g.plays}{#if g.renameTo}{' '}<button type="button" class="btn small ghost rename" onclick={() => (renaming = { p, table: g.name, to: g.renameTo })}>Rename it {g.renameTo}…</button>{/if}</span>
        {/each}
      {:else}
        <span class="faint">No table here plays it, and VPS has no game by its name</span>
      {/if}
    </span>
    <span class="playout" class:unset={p.facts.setup === 'unchosen'} title={layoutOf(p)}>{layoutOf(p)}</span>
    <span class="pstate {p.facts.look}" title={p.facts.text}><Icon name={ICON[p.facts.look]} size={15} width={2.4} />{p.facts.state}</span>
    <!-- each button in its own place, the same on every row: Layout… (or Unpack… for a zipped
         pack), Files, VPS, the bin -->
    <span class="pactions">
      <span>
        {#if p.facts.layouts}<button type="button" class="btn small" onclick={() => (choosing = p)}>Layout…</button>
        {:else if p.archives.length}<button type="button" class="btn small" title="Opens it in the drop window, as if dropped" onclick={() => dropFiles(p.archives, p.dropTable)}>Unpack…</button>{/if}
      </span>
      <span><button type="button" class="btn small" onclick={() => files(p)}>Files</button></span>
      <span>{#if p.game}<button type="button" class="btn small ghost" title="{p.game.name} on VPS" onclick={() => openLink(p.game.url)}>VPS<Icon name="ext" size={13} /></button>{/if}</span>
      <span><button type="button" class="btn small bin" aria-label="Remove {p.name}…" title="Move it to the Recycle Bin" onclick={() => (asking = p)}><Icon name="trash" size={15} /></button></span>
    </span>
    {#if p.facts.look === 'warn' || p.facts.look === 'bad'}<p class="pnote {p.facts.look}">{p.facts.text}</p>{/if}
    {#if renaming?.p === p}
      <div class="pask">
        <span>Rename the folder {p.name} to {renaming.to}, the name your {renaming.table} asks for? It may not fit: a pack made for another version of a table can show things at the wrong moments. It stays where it is, PinUP Popper finds it by the new name too, and you can undo it here.</span>
        <button type="button" class="btn small" onclick={() => rename(renaming)}>Rename it</button>
        <button type="button" class="btn small ghost" onclick={() => (renaming = null)}>Cancel</button>
      </div>
    {/if}
    {#if asking === p}
      <div class="pask">
        <span>{removing(p)} It goes to the Recycle Bin, where you can bring it back.</span>
        <button type="button" class="btn small" onclick={() => remove(p)}>Delete it</button>
        <button type="button" class="btn small ghost" onclick={() => (asking = null)}>Cancel</button>
      </div>
    {/if}
  </div>
{/snippet}

<section class="packs" aria-label="Your packs" data-loading={packs || problem ? undefined : ''}>
  <h2 class="partname">Your packs</h2>
  {#if problem}
    <p class="problem">{problem} <button type="button" class="btn small" onclick={load}>Try again</button></p>
  {:else if !packs}
    <p class="note">Reading your packs…</p>
  {:else if !packs.length}
    <p class="note">No PinUP packs yet. A pack dropped onto the app goes into PinUP’s folder, ready for its table.</p>
  {:else}
    <p class="note">
      {packs.length} {packs.length === 1 ? 'pack' : 'packs'}{#if unchosen}{' · '}<b class="warn">{unchosen} {unchosen === 1 ? 'needs' : 'need'} a layout chosen</b>{/if}{#if wrong}{` · ${wrong} not right`}{/if}{#if unusedAll.length}{` · ${unusedAll.length} no table plays`}{/if}.
      A pack plays for the table whose script asks for it by name; its layout says which of your screens shows what. Drop a pack onto a row to add it for that row’s table.
    </p>
    <div class="bar">
      <div class="seg" role="radiogroup" aria-label="Which packs show">
        {#each [['all', 'All', packs.length], ['used', 'Used', usedAll.length], ['unused', 'Unused', unusedAll.length]] as [key, label, n] (key)}
          <button type="button" role="radio" aria-checked={showing === key} onclick={() => (showing = key)}>{label} <span class="n">{n}</span></button>
        {/each}
      </div>
      <SearchBox bind:value={query} placeholder="Search by table or pack" />
    </div>
    {#if query.trim() && !used.length && !unused.length}<p class="note">No pack here matches “{query.trim()}”.</p>{/if}
    <div class="scroll">
      <div class="head" aria-hidden="true"><span>Pack</span><span>Table</span><span>Layout</span><span>Status</span><span></span></div>
      {#if used.length && showing !== 'unused'}
        <div class="list">{#each used as p (p.path)}{@render row(p)}{/each}</div>
      {/if}
      {#if unused.length && showing !== 'used'}
        {#if showing === 'all'}<h3 class="sub-head">No table here plays these</h3>{/if}
        <p class="note">Their table isn’t here, or its script asks for another name. A possible match is named where a table of yours asks for a name like the pack’s, or VPS has a game by its name (its ROM’s, mostly).</p>
        <div class="list">{#each unused as p (p.path)}{@render row(p)}{/each}</div>
      {/if}
    </div>
  {/if}
</section>

{#if choosing}
  <PupLayout path={choosing.path} title={titleOf(choosing)} tableId={choosing.tables[0]?.[0] ?? null} onclose={() => (choosing = null)} ondone={load} />
{/if}

<style>
  .packs { display: flex; flex-direction: column; gap: 10px; padding-top: 22px; border-top: 1px solid var(--line); }
  .partname { margin: 0 0 4px; font-size: 20px; font-weight: 700; color: var(--fg); }
  .sub-head { margin: 16px 0 6px; font-size: 15px; color: var(--fg); }
  .note { margin: -4px 0 0; color: var(--faint); font-size: 13px; max-width: 900px; line-height: 1.5; }
  .scroll .note { margin: 0 0 8px; }
  .note .warn { color: var(--warn); font-weight: 600; }
  .problem { margin: 0; padding: 12px 14px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; }
  /* All, Used, Unused */
  .seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg .n { margin-left: 4px; color: var(--faint); font-size: 12px; }
  .seg button[aria-checked='true'] .n { color: inherit; opacity: 0.75; }
  /* the list scrolls on its own, its headings kept in view */
  .bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px 16px; }
  .scroll { max-height: max(360px, calc(100vh - 300px)); overflow: auto; padding-right: 6px; border-radius: 12px; }
  :global(html[data-whole]) .scroll { max-height: none; }
  /* the same columns in the heading and every row: the buttons' column a fixed width */
  .head, .pack { display: grid; grid-template-columns: minmax(110px, 170px) minmax(0, 1.4fr) minmax(0, 1fr) 150px 278px; gap: 6px 14px; align-items: center; }
  .head { position: sticky; top: 0; z-index: 1; padding: 8px 12px; background: var(--bg); font-size: 12px; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .pack { padding: 10px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .pack.over { border-color: var(--accent); background: var(--accent-bg); }
  .pname { font: 600 13px ui-monospace, Consolas, monospace; color: var(--fg); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ptables { font-size: 14px; color: var(--fg-soft); min-width: 0; }
  .faint { color: var(--faint); }
  .sub { display: block; color: var(--faint); font-size: 12px; margin-top: 2px; }
  .warnword { color: var(--warn); }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; text-align: left; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
  .playout { font-size: 13px; color: var(--fg-soft); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .playout.unset { color: var(--warn); font-weight: 600; }
  .pstate { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; white-space: nowrap; }
  .pstate.good { color: var(--good); }
  .pstate.warn { color: var(--warn); }
  .pstate.bad { color: var(--bad); }
  .pstate.none { color: var(--faint); }
  /* the buttons' places: Layout… or Unpack…, Files, VPS, the bin */
  .pactions { display: grid; grid-template-columns: 92px 64px 64px 34px; gap: 6px; justify-items: stretch; }
  .pactions .btn { width: 100%; justify-content: center; gap: 4px; }
  .bin { padding: 0 8px; }
  .renamed { display: block; margin-top: 2px; font: 400 12px var(--body); color: var(--faint); white-space: normal; }
  .link.small { font-size: 12px; }
  .rename { height: 24px; margin-left: 4px; padding: 0 8px; font-size: 12px; }
  .pnote { grid-column: 2 / -1; margin: 0; font-size: 13px; line-height: 1.45; }
  .pnote.warn { color: var(--warn); }
  .pnote.bad { color: var(--bad); }
  .pask { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); }
  .pask span { flex: 1 1 260px; font-size: 13px; color: var(--fg-soft); }
  @media (max-width: 1100px) {
    .head { display: none; }
    .pack { grid-template-columns: minmax(0, 1fr) 278px; }
    .pnote { grid-column: 1 / -1; }
  }
</style>
