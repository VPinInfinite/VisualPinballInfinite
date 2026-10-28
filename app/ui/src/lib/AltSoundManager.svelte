<script>
  // Tools › AltSound: every ROM table's AltSound pack at once — recorded music and callouts played
  // in place of its machine's own sound, as libaltsound finds them — each checked as the player
  // reads it (the library's checks), those made from a table's ROM said so with their Listen page;
  // for a table with none, whether one can be made from its ROM (`altsound_offer`, asked for each
  // as the page shows) and Make one… (the window that asks first), and VPS's list of them opened on
  // its page; a pack shown or removed (asked first, to the Recycle Bin, `remove`).
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import MakeAltSound from './MakeAltSound.svelte';
  import SearchBox from './SearchBox.svelte';
  import { app, drop, notice, openTable } from './store.svelte.js';

  let showing = $state('all');
  // whether a pack can be made, by table: { can, rom, why }, as each answer comes
  let offers = $state({});
  let making = $state(null);
  let asking = $state(null);

  const byName = (a, b) => a.name.localeCompare(b.name);
  const roms = $derived(app.tables.filter((t) => t.rom && !t.hiddenTable).sort(byName));
  const look = (t) => (t.holds.altsound ? (t.altCheck ? t.altCheck[0] : 'good') : 'none');
  const groups = $derived([
    ['all', 'All ROM tables', roms],
    ['has', 'Have a pack', roms.filter((t) => t.holds.altsound)],
    ['look', 'Need a look', roms.filter((t) => t.holds.altsound && look(t) !== 'good')],
    ['make', 'Can make one', roms.filter((t) => !t.holds.altsound && offers[t.id]?.can)],
  ]);
  // the search: a table's name or its ROM
  let query = $state('');
  const matches = (t) => {
    const q = query.trim().toLowerCase();
    return !q || [t.name, t.rom].some((n) => n && n.toLowerCase().includes(q));
  };
  const shown = $derived((groups.find(([k]) => k === showing)?.[2] ?? []).filter(matches));

  // asked one table at a time, for those without a pack, as the page shows
  $effect(() => {
    const wanting = roms.filter((t) => !t.holds.altsound && t.holds.rom && !(t.id in offers)).map((t) => t.id);
    let stop = false;
    (async () => {
      for (const id of wanting) {
        if (stop) return;
        const o = await invoke('altsound_offer', { id }).catch(() => null);
        if (!stop && o) offers = { ...offers, [id]: o };
      }
    })();
    return () => (stop = true);
  });

  const fileName = (p) => p.split(/[\\/]/).pop();
  const STATE = { good: 'Complete', warn: 'Needs a look', bad: 'Not right' };

  const show = (t) => invoke('show_file', { id: t.id, what: 'altSound' }).catch((e) => notice(String(e), { kind: 'problem' }));
  const listen = (t) => invoke('altsound_listen', { id: t.id }).catch((e) => notice(String(e), { kind: 'problem' }));

  function find(t) {
    app.findOnOpen = 'altSound';
    openTable(t.id);
  }

  async function askRemove(t) {
    try {
      const item = (await invoke('remove_plan', { id: t.id })).find((i) => i.key === 'altsound');
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
      notice(done.problems.length ? done.problems.join(' ') : `${t.name}’s AltSound pack went to the Recycle Bin.`, { kind: done.problems.length ? 'problem' : 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
</script>

<div class="page">
  <header class="title">
    <h1 class="display">AltSound</h1>
    <p>An AltSound pack plays recorded music and callouts in place of a machine’s own sound. Here are every ROM table’s, checked as the player reads them, and for those without one, whether one can be made from the table’s ROM.</p>
  </header>

  {#if !roms.length}
    <p class="note">No table plays a ROM machine yet.</p>
  {:else}
    <p class="note">{groups[1][2].length} of {roms.length} ROM tables have a pack. Settings › Sound turns packs off for every game; a game’s own settings choose its sound.</p>
    <div class="bar">
      <div class="seg" role="radiogroup" aria-label="Which tables show">
        {#each groups as [key, label, list] (key)}
          <button type="button" role="radio" aria-checked={showing === key} onclick={() => (showing = key)}>{label} <span class="n">{list.length}</span></button>
        {/each}
      </div>
      <SearchBox bind:value={query} placeholder="Search by table or ROM" />
    </div>
    {#if !shown.length}
      <p class="note">{query.trim() ? `No table here matches “${query.trim()}”.` : showing === 'look' ? 'Every pack is as the player reads it.' : showing === 'make' ? 'None of the tables without a pack can have one made from its ROM yet, or they’re still being asked.' : 'None has a pack yet.'}</p>
    {:else}
      <div class="scroll">
        <div class="head" aria-hidden="true"><span>Table</span><span>ROM</span><span>Pack</span><span></span></div>
        <div class="list">
          {#each shown as t (t.id)}
            {@const l = look(t)}
            {@const o = offers[t.id]}
            <div class="row" class:over={drop.dragging && drop.over === t.id} data-drop-table={t.id}>
              <span class="tname"><button type="button" class="link" onclick={() => openTable(t.id)}>{t.name}</button>{#if t.maker || t.year}<span class="sub">{[t.maker, t.year].filter(Boolean).join(' · ')}</span>{/if}</span>
              <span class="mono">{t.rom}</span>
              <span class="state">
                {#if t.holds.altsound}
                  <span class="st {l}"><Icon name={l === 'good' ? 'check' : 'alert'} size={15} width={2.4} />{STATE[l] ?? 'Added'}</span>
                  <span class="sub" title={t.holds.altsound}>{fileName(t.holds.altsound)}{t.altMade ? ', made from its ROM' : ''}</span>
                  {#if l !== 'good' && t.altCheck}<span class="sub {l}">{t.altCheck[1]}</span>{/if}
                {:else if !t.holds.rom}
                  <span class="faint">None · its ROM isn’t there yet</span>
                {:else if o && !o.can}
                  <span class="faint">None</span><span class="sub">{o.why}</span>
                {:else}
                  <span class="faint">None{t.vpsHas?.altSound ? ` · VPS lists ${t.vpsHas.altSound}` : ''}</span>
                {/if}
              </span>
              <span class="actions">
                {#if t.holds.altsound}
                  {#if t.altMade}<button type="button" class="btn small" onclick={() => listen(t)}>Listen</button>{/if}
                  <button type="button" class="btn small" onclick={() => show(t)}>Show</button>
                  <button type="button" class="btn small bin" aria-label="Remove {t.name}’s AltSound pack…" title="Move it to the Recycle Bin" onclick={() => askRemove(t)}><Icon name="trash" size={15} /></button>
                {:else}
                  {#if t.vpsHas?.altSound}<button type="button" class="btn small" onclick={() => find(t)}>Find on VPS</button>{/if}
                  {#if o?.can}<button type="button" class="btn small" onclick={() => (making = { t, rom: o.rom })}>Make one…</button>{/if}
                {/if}
              </span>
              {#if asking?.t === t}
                <div class="ask">
                  <span>Move {t.name}’s AltSound pack ({asking.item.name}) to the Recycle Bin? It plays its machine’s own sound after.{asking.item.shared.length ? ` Also used by ${asking.item.shared.join(', ')}.` : ''}</span>
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

{#if making}
  <MakeAltSound t={making.t} rom={making.rom} onclose={() => (making = null)} />
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
  .head, .row { display: grid; grid-template-columns: minmax(0, 1.4fr) 130px minmax(0, 1.6fr) 220px; gap: 6px 14px; align-items: center; }
  .head { position: sticky; top: 0; z-index: 1; padding: 8px 12px; background: var(--bg); font-size: 12px; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .row { padding: 10px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .row.over { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, var(--card)); }
  .tname { min-width: 0; font-size: 14px; }
  .sub { display: block; color: var(--faint); font-size: 12px; margin-top: 2px; overflow: hidden; text-overflow: ellipsis; }
  .sub.warn { color: var(--warn); }
  .sub.bad { color: var(--bad); }
  .mono { font: 13px ui-monospace, Consolas, monospace; color: var(--fg-soft); }
  .state { min-width: 0; font-size: 13px; }
  .st { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; }
  .st.good { color: var(--good); }
  .st.warn { color: var(--warn); }
  .st.bad { color: var(--bad); }
  .faint { color: var(--faint); }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; justify-content: flex-end; }
  .bin { padding: 0 8px; }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; text-align: left; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
  .ask { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); }
  .ask span { flex: 1 1 260px; font-size: 13px; color: var(--fg-soft); }
</style>
