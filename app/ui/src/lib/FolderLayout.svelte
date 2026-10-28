<script>
  // Settings › Library › Folders: where tables keep their files — shared folders, as VPX and PinUP
  // Popper lay them out, or a folder for each game, as VPX 10.8.1 reads them. Choosing a folder
  // for each game works out the move first (`layout_plan`: what moves and is copied where, nothing
  // moved) and asks; moving tells each game's folder as it's begun (`layout-progress`); a switch
  // made here is kept to put back (`layout_put_back`). Where the tables are now: `layout_state`.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { notice } from './store.svelte.js';

  let st = $state(null);
  let problem = $state('');
  // the move worked out, while it's asked about; working it out; moving, as told
  let plan = $state(null);
  let planning = $state(false);
  let moving = $state(null);
  let seeing = $state(false);
  // putting a switch back: asked, then under way
  let asking = $state(false);
  let putting = $state(false);

  async function load() {
    try {
      st = await invoke('layout_state');
      problem = '';
    } catch (e) {
      problem = String(e);
    }
  }
  $effect(() => {
    load();
    const off = listen('library-changed', () => load());
    const told = listen('layout-progress', (e) => (moving = e.payload));
    return () => {
      off.then((f) => f());
      told.then((f) => f());
    };
  });

  // a folder for each game: a switch made here, or every table in a folder of its own already
  const mode = $derived(!st ? null : st.switched || (st.own > 0 && st.together === 0) ? 'game' : 'shared');

  async function choose(m) {
    if (!st || m === mode || moving || putting) return;
    if (m === 'game') {
      asking = false;
      planning = true;
      try {
        plan = await invoke('layout_plan');
      } catch (e) {
        notice(String(e), { kind: 'problem' });
      } finally {
        planning = false;
      }
    } else if (st.switched) {
      plan = null;
      asking = true;
    } else {
      notice('Your tables were put in folders of their own before this app, so there’s no move here to put back.', { kind: 'info' });
    }
  }

  async function move() {
    moving = { done: 0, of: Object.keys(plan.folders).length, name: '' };
    try {
      await invoke('layout_switch');
      notice(`Done: ${plan.tables} tables are in folders of their games. Put them back here whenever you like.`, { kind: 'done' });
      plan = null;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      moving = null;
      load();
    }
  }

  async function putBack() {
    putting = true;
    try {
      await invoke('layout_put_back');
      notice('Every table and file is back where it was.', { kind: 'done' });
      asking = false;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      putting = false;
      load();
    }
  }

  const count = (n, one, many) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
  const bytes = (n) => (n < 1e6 ? `${Math.max(1, Math.round(n / 1000))} KB` : n < 1e9 ? `${(n / 1e6).toFixed(n < 1e7 ? 1 : 0)} MB` : `${(n / 1e9).toFixed(1)} GB`);
  const name = (p) => p.split(/[\\/]/).pop();
  const drive = (p) => p.slice(0, 2).toLowerCase();
  // a stamp, `2025-01-02 18-04-05`, as a day and a time
  function when(stamp) {
    const m = /^(\d{4})-(\d\d)-(\d\d) (\d\d)-(\d\d)/.exec(stamp ?? '');
    if (!m) return '';
    const d = new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5]);
    return d.toLocaleString(undefined, { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' });
  }
  const folders = $derived(plan ? Object.entries(plan.folders).sort(([a], [b]) => a.localeCompare(b)) : []);
  const sameDrive = $derived(plan ? plan.moves.every((m) => m.copy || drive(m.from) === drive(m.to)) : true);
</script>

<div class="block" data-loading={st || problem ? (planning ? '' : undefined) : ''}>
  <h3 class="section-title">Where tables keep their files</h3>
  {#if problem}
    <p class="what bad">{problem}</p>
  {:else if st}
    <div class="modes" role="radiogroup" aria-label="Where tables keep their files">
      <button type="button" role="radio" class="mode" class:on={mode === 'shared'} aria-checked={mode === 'shared'} onclick={() => choose('shared')}>
        <span class="ring" aria-hidden="true"></span>
        <span><b>Shared folders</b><span class="what">Tables together, ROMs in VPinMAME’s folder, PinUP packs in PinUP’s: as VPX and PinUP Popper lay them out.</span></span>
      </button>
      <button type="button" role="radio" class="mode" class:on={mode === 'game'} aria-checked={mode === 'game'} onclick={() => choose('game')}>
        <span class="ring" aria-hidden="true"></span>
        <span><b>A folder for each game</b><span class="what">Each game’s tables with their ROM, packs, coloring and music. Easy to copy, back up or delete one game.</span><span class="what warnline">Only VPX 10.8.1 and later read this layout. VPX 10.8.0 — and PinUP Popper or any program starting tables through it — can’t start ROM tables kept this way, nor find their PinUP packs, AltSound or colorings. Our player and VPin Studio read both.</span></span>
      </button>
    </div>

    {#if st.switched && !asking}
      <p class="what">In a folder for each game since {when(st.switched.at)}: {count(st.switched.tables, 'table', 'tables')} in {count(st.switched.folders, 'folder', 'folders')}. Choose Shared folders to put everything back.</p>
    {:else if mode === 'shared' && st.own > 0 && !plan}
      <p class="what">{count(st.own, 'table is', 'tables are')} in folders of their own already; the others share the tables folder.</p>
    {/if}

    {#if planning}
      <div class="panel"><span class="what">Working out what would move…</span></div>
    {:else if plan}
      <div class="panel">
        <b>Move your library into a folder for each game?</b>
        {#if plan.tables}
          <span>
            {count(plan.tables, 'table', 'tables')} into {count(folders.length, 'folder', 'folders')}: {count(plan.moved[1], 'file', 'files')} ({bytes(plan.moved[0])}) moved.
            {#if sameDrive}They’re on the same drive, so it takes seconds.{/if}
            {#if plan.copied[1]}{count(plan.copied[1], 'small file is', 'small files are')} copied.{/if}
            {#if plan.popper}PinUP Popper’s list follows them.{/if}
          </span>
          {#if plan.shared.length}<span class="what">{count(plan.shared.length, 'thing stays', 'things stay')} where {plan.shared.length === 1 ? 'it is' : 'they are'}, used by tables of more than one game.</span>{/if}
          <span class="what">VPX 10.8.0 doesn’t read ROMs kept beside a table: if you still play VPX 10.8.0, its ROM tables would stop starting. Nothing is deleted, and Put them back undoes it all.</span>
        {:else}
          <span class="what">Every table is in a folder of its own already.</span>
        {/if}
        {#each plan.problems as p (p)}<span class="warnline">{p}: nothing can be moved until it’s sorted.</span>{/each}
        {#if moving}
          <div class="meter" role="progressbar" aria-valuemin="0" aria-valuemax={moving.of} aria-valuenow={moving.done}><span style:width="{moving.of ? (100 * moving.done) / moving.of : 0}%"></span></div>
          <span class="what">Moving{moving.name ? `: ${moving.name}…` : '…'} {moving.done} of {moving.of}</span>
        {:else}
          <div class="row">
            <button type="button" class="btn small go" disabled={!plan.tables || plan.problems.length > 0} onclick={move}>Move them</button>
            <button type="button" class="btn small ghost" onclick={() => (plan = null)}>Cancel</button>
            {#if plan.tables}<button type="button" class="link" aria-expanded={seeing} onclick={() => (seeing = !seeing)}>{seeing ? 'Hide' : 'See'} each game’s folder</button>{/if}
          </div>
          {#if seeing}
            <div class="list">
              {#each folders as [folder, tables] (folder)}
                <div class="frow"><span class="fname">{name(folder)}</span><span class="what">{tables.map(name).join(', ')}</span></div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {:else if asking}
      <div class="panel">
        <b>Put your library back in shared folders?</b>
        <span>Every table and file goes back where it was before {when(st.switched?.at)}, and PinUP Popper’s list with them. A machine’s memory VPX played on in a game’s folder goes back too; the older copy goes to the Recycle Bin.</span>
        <div class="row">
          <button type="button" class="btn small go" disabled={putting} onclick={putBack}>{putting ? 'Putting them back…' : 'Put them back'}</button>
          <button type="button" class="btn small ghost" disabled={putting} onclick={() => (asking = false)}>Cancel</button>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .block { display: flex; flex-direction: column; gap: 10px; }
  .block h3 { margin: 0; }
  .what { color: var(--faint); font-size: 13px; margin: 0; line-height: 1.5; }
  .bad { color: var(--bad); }
  .modes { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 8px; max-width: 900px; }
  .mode { display: flex; align-items: flex-start; gap: 12px; padding: 12px 14px; border-radius: 10px; border: 1px solid var(--card-line); background: var(--card); color: var(--fg-soft); text-align: left; font: inherit; cursor: pointer; }
  .mode:hover { border-color: var(--control-line); }
  .mode.on { border-color: var(--accent); background: var(--accent-bg); }
  .mode > span:last-child { display: flex; flex-direction: column; gap: 4px; font-size: 14px; }
  .mode b { color: var(--fg); }
  .ring { flex: 0 0 auto; width: 16px; height: 16px; margin-top: 2px; border-radius: 50%; border: 2px solid var(--control-line); box-sizing: border-box; }
  .mode.on .ring { border-color: var(--accent); background: radial-gradient(circle, var(--accent) 0 4px, transparent 5px); }
  .panel { display: flex; flex-direction: column; gap: 8px; max-width: 900px; padding: 14px 16px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); font-size: 14px; color: var(--fg-soft); line-height: 1.5; }
  .panel b { color: var(--fg); font-size: 15px; }
  .warnline { color: var(--warn); font-size: 13px; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 4px; }
  .btn.go { background: var(--accent-bg); border-color: var(--accent); color: #fff; }
  .link { padding: 0; border: 0; background: none; color: var(--fg-soft); font: inherit; font-size: 13px; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
  .meter { height: 8px; border-radius: 999px; background: var(--control); overflow: hidden; }
  .meter span { display: block; height: 100%; background: var(--accent); transition: width 0.2s; }
  .list { display: flex; flex-direction: column; max-height: 320px; overflow: auto; border-top: 1px solid var(--line); }
  :global(html[data-whole]) .list { max-height: none; }
  .frow { display: grid; grid-template-columns: minmax(200px, 320px) minmax(0, 1fr); gap: 12px; padding: 6px 2px; border-bottom: 1px solid var(--line-soft); font-size: 13px; }
  .fname { color: var(--fg-soft); }
</style>
