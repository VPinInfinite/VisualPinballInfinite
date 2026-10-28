<script>
  // The player hub's high scores: every table's at once (`high_scores_all`) — the best its machine's
  // memory or its script keeps, ours or VPX's, whichever is higher, and the player's own best from
  // the games played here (its play log) — highest first or by name. A table opens its page, where
  // its whole list is.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { app, openTable } from './store.svelte.js';

  let all = $state(null);
  let problem = $state('');
  let showing = $state('scores');

  async function load() {
    try {
      all = await invoke('high_scores_all');
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

  // a table's best: the first place of the copy whose first place is higher
  function best(hs) {
    const tops = (hs?.kept || []).map((k) => ({ by: k.by, e: k.scores.find((e) => e.score) })).filter((x) => x.e);
    return tops.sort((a, b) => b.e.score - a.e.score)[0] ?? null;
  }
  const mine = (t) => Math.max(0, ...(t.played?.log || []).flatMap((g) => g.scores || []));

  const rows = $derived(
    all
      ? all
          .map((x) => ({ t: app.tables.find((t) => t.id === x.id), hs: x.scores }))
          .filter((r) => r.t && !r.t.hiddenTable)
          .map((r) => ({ ...r, top: best(r.hs), me: mine(r.t) }))
      : [],
  );
  const groups = $derived([
    ['scores', 'With scores', rows.filter((r) => r.top)],
    ['mine', 'Played here', rows.filter((r) => r.me > 0)],
    ['all', 'All tables', rows],
  ]);
  const shown = $derived((groups.find(([k]) => k === showing)?.[2] ?? []).slice().sort((a, b) => a.t.name.localeCompare(b.t.name)));
  const number = (n) => (n == null ? '' : n.toLocaleString());
  const KIND = { machine: 'Its machine’s memory', script: 'Its script', none: '—' };
</script>

<div class="page" data-loading={all || problem ? undefined : ''}>
  <header class="title">
    <h1 class="display">High scores</h1>
    <p>Every table’s best, as its machine’s memory or its script keeps it — ours and VPX’s, whichever is higher — beside your own best from the games you played here. A table’s page has its whole list.</p>
  </header>

  {#if problem}
    <p class="problem">{problem}</p>
  {:else if !all}
    <p class="note">Reading every table’s high scores…</p>
  {:else}
    <div class="seg" role="radiogroup" aria-label="Which tables show">
      {#each groups as [key, label, list] (key)}
        <button type="button" role="radio" aria-checked={showing === key} onclick={() => (showing = key)}>{label} <span class="n">{list.length}</span></button>
      {/each}
    </div>
    {#if !shown.length}
      <p class="note">{showing === 'mine' ? 'Play a table here and your scores show up.' : 'None of your tables keeps scores that can be read yet.'}</p>
    {:else}
      <div class="scroll">
        <div class="head" aria-hidden="true"><span>Table</span><span>Its best</span><span>Your best</span><span>Kept by</span></div>
        <div class="list">
          {#each shown as r (r.t.id)}
            <div class="row">
              <span class="tname"><button type="button" class="link" onclick={() => openTable(r.t.id)}>{r.t.name}</button>{#if r.t.maker || r.t.year}<span class="sub">{[r.t.maker, r.t.year].filter(Boolean).join(' · ')}</span>{/if}</span>
              <span class="best">
                {#if r.top}
                  <b>{number(r.top.e.score)}</b> <span class="who">{r.top.e.initials || '—'}</span>
                  <span class="sub">{r.top.e.label ?? 'First place'}{r.top.by === 'vpx' ? ', played in VPX' : ''}</span>
                {:else}
                  <span class="faint">{r.hs?.problem ? 'Can’t be read yet' : 'None kept'}</span>
                {/if}
              </span>
              <span class="mine">{#if r.me}<b>{number(r.me)}</b>{:else}<span class="faint">—</span>{/if}</span>
              <span class="kind">{KIND[r.hs?.kind] ?? '—'}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 18px; max-width: 1240px; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .note { margin: 0; color: var(--faint); font-size: 13px; max-width: 860px; line-height: 1.5; }
  .problem { margin: 0; padding: 12px 14px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; }
  .seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg .n { margin-left: 4px; color: var(--faint); font-size: 12px; }
  .seg button[aria-checked='true'] .n { color: inherit; opacity: 0.75; }
  .scroll { max-height: min(72vh, 820px); overflow: auto; padding-right: 6px; border-radius: 12px; }
  :global(html[data-whole]) .scroll { max-height: none; }
  .head, .row { display: grid; grid-template-columns: minmax(0, 1.5fr) minmax(0, 1.2fr) 150px 170px; gap: 6px 14px; align-items: center; }
  .head { position: sticky; top: 0; z-index: 1; padding: 8px 12px; background: var(--bg); font-size: 12px; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .row { padding: 10px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .tname { min-width: 0; font-size: 14px; }
  .sub { display: block; color: var(--faint); font-size: 12px; margin-top: 2px; }
  .best b, .mine b { font-variant-numeric: tabular-nums; color: var(--fg); }
  .who { margin-left: 6px; font: 600 13px ui-monospace, Consolas, monospace; color: var(--accent); }
  .kind { font-size: 13px; color: var(--fg-soft); }
  .faint { color: var(--faint); }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; text-align: left; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
</style>
