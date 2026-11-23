<script>
  // A table's parts down the side of the table editor, by kind: a kind's list folded or unfolded
  // (`prefs.foldedKinds`), shown on the playfield or not where it has a place there, its parts
  // found by name. In the achievements editor (`counting`) the parts achievements can't count are
  // greyed, each can be dragged onto a condition, and a click picks it into the one being picked. A
  // part hidden on a right-click (`hidePart`, here or on the playfield) shows a closed eye — a click
  // on it shows the part again — and can't be chosen meanwhile.
  import Icon from './Icon.svelte';
  import { COLOURS, ORDER } from './playfield.js';
  import { app, savePrefs, hiddenParts, hidePart } from './store.svelte.js';

  let { parts, chosen = null, hidden = $bindable({}), counting = false, picked = [], onpick, table = null } = $props();

  const off = $derived(new Set(table ? hiddenParts(table) : []));
  // the right-click's menu: the part and where it opened
  let menu = $state(null);

  // listed in this order, the parts with no place on the playfield after
  const LISTED = ORDER.concat(['Timer', 'Text box', 'Light sequencer', 'Decal', 'Reel', 'Part group', 'Ball', 'Other']);

  let query = $state('');
  const folded = $derived(app.prefs.foldedKinds || {});
  function fold(kind) {
    app.prefs.foldedKinds = { ...folded, [kind]: !folded[kind] };
    savePrefs();
  }
  const groups = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return LISTED.map((k) => [k, parts.filter((p) => p.kind === k && (!q || p.name.toLowerCase().includes(q)))]).filter(([, list]) => list.length);
  });
  // with a search, every kind with a match opens
  const open = (kind) => !!query.trim() || !folded[kind];
  const pickedNames = $derived(new Set(picked.map((n) => n.toLowerCase())));

  function drag(e, p) {
    if (!counting || !p.counts.length) {
      e.preventDefault();
      return;
    }
    e.dataTransfer.setData('application/x-vpi-part', p.name);
    e.dataTransfer.setData('text/plain', p.name);
    e.dataTransfer.effectAllowed = 'copy';
  }
</script>

<div class="parts" aria-label="The table’s parts">
  <input class="search" type="search" placeholder="Find a part by name" aria-label="Find a part by name" bind:value={query} spellcheck="false" />
  <p class="count">{parts.length} parts{#if counting} · bright ones achievements can count: drag one onto a condition{/if}</p>
  {#each groups as [kind, list] (kind)}
    <div class="kind">
      <div class="kind-head">
        <button type="button" class="fold" aria-expanded={open(kind)} aria-label="{open(kind) ? 'Fold' : 'Unfold'} {kind}" onclick={() => fold(kind)}>
          <Icon name={open(kind) ? 'chevD' : 'chevR'} size={13} />
          <span class="sw" style:background={(COLOURS[kind] || ['#445', '#556'])[1]}></span>
          <span class="kname">{kind}</span>
        </button>
        {#if COLOURS[kind]}
          <label class="eye" title={hidden[kind] ? 'Show them on the playfield' : 'Hide them on the playfield'}><input type="checkbox" checked={!hidden[kind]} onchange={(e) => (hidden = { ...hidden, [kind]: !e.currentTarget.checked })} aria-label="{kind} on the playfield" /></label>
        {/if}
        <span class="n">{list.length}</span>
      </div>
      {#if open(kind)}
        {#each list as p (p.i)}
          {@const cannot = counting && !p.counts.length}
          {@const gone = off.has(p.name)}
          <div class="itemrow">
            <button
              type="button"
              class="item"
              class:on={p.i === chosen && !gone}
              class:off={cannot || gone}
              class:inrow={pickedNames.has(p.name.toLowerCase())}
              draggable={counting && !cannot && !gone}
              ondragstart={(e) => drag(e, p)}
              title={gone ? 'Hidden: click its eye to show it again' : cannot ? 'Achievements can’t count it' : counting ? 'Click to pick it, or drag it onto a condition; right-click to hide it' : 'Right-click to hide it'}
              oncontextmenu={(e) => { if (!table) return; e.preventDefault(); menu = { p, x: e.clientX, y: e.clientY }; }}
              onclick={() => !gone && onpick?.(p)}>{p.name}</button>
            {#if gone}<button type="button" class="eyeoff" title="Hidden: show it again" aria-label="Show {p.name} again" onclick={() => hidePart(table, p.name, false)}><Icon name="eyeOff" size={14} /></button>{/if}
          </div>
        {/each}
      {/if}
    </div>
  {/each}
  {#if !groups.length}<p class="quiet">No part’s name has “{query.trim()}” in it.</p>{/if}
</div>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => e.key === 'Escape' && (menu = null)} />
{#if menu}
  <div class="ctx" role="menu" style="left: {Math.min(menu.x, window.innerWidth - 200)}px; top: {Math.min(menu.y, window.innerHeight - 60)}px">
    {#if off.has(menu.p.name)}
      <button type="button" role="menuitem" onclick={() => { hidePart(table, menu.p.name, false); menu = null; }}><Icon name="eye" size={14} />Show it</button>
    {:else}
      <button type="button" role="menuitem" onclick={() => { hidePart(table, menu.p.name, true); menu = null; }}><Icon name="eyeOff" size={14} />Hide it</button>
    {/if}
  </div>
{/if}

<style>
  .parts { padding: 12px; display: flex; flex-direction: column; gap: 6px; max-height: calc(100vh - 225px); overflow: auto; background: var(--card); border: 1px solid var(--card-line); border-radius: 14px; min-width: 0; }
  .search { width: 100%; box-sizing: border-box; height: 34px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--control-line); background: var(--bg); color: var(--fg); font: inherit; font-size: 14px; }
  .search:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .count { margin: 0; color: var(--faint); font-size: 12px; line-height: 1.4; }
  .kind { display: flex; flex-direction: column; gap: 1px; }
  .kind-head { display: flex; align-items: center; gap: 6px; }
  .fold { flex: 1; min-width: 0; display: flex; align-items: center; gap: 7px; padding: 5px 4px; border: 0; border-radius: 6px; background: transparent; color: var(--faint); font: inherit; font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; text-align: left; cursor: pointer; }
  .fold:hover { background: var(--hover); color: var(--fg-soft); }
  .sw { width: 10px; height: 10px; border-radius: 3px; flex: 0 0 auto; }
  .kname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .eye { display: inline-flex; align-items: center; cursor: pointer; }
  .eye input { accent-color: var(--accent); }
  .n { color: var(--faint); font-size: 12px; font-variant-numeric: tabular-nums; min-width: 22px; text-align: right; }
  .itemrow { display: flex; align-items: center; gap: 2px; }
  .eyeoff { flex: 0 0 auto; width: 24px; height: 24px; display: grid; place-items: center; border: 0; border-radius: 6px; background: transparent; color: var(--warn); cursor: pointer; }
  .eyeoff:hover { background: var(--hover); }
  .ctx { position: fixed; z-index: 60; min-width: 160px; padding: 5px; border-radius: 10px; background: #151a24; border: 1px solid var(--control-line); box-shadow: 0 14px 30px rgba(0, 0, 0, 0.5); display: flex; flex-direction: column; }
  .ctx button { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border: 0; border-radius: 7px; background: transparent; color: var(--fg); font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
  .ctx button:hover { background: var(--selected); }
  .item { display: block; flex: 1; min-width: 0; text-align: left; padding: 4px 8px 4px 26px; border: 0; border-radius: 6px; background: transparent; color: var(--fg-soft); font: 13px Consolas, ui-monospace, monospace; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item:hover { background: var(--hover); }
  .item.on { background: var(--accent-bg); color: #fff; }
  .item.off { opacity: 0.38; }
  .item.inrow:not(.on) { color: var(--accent); }
  .item[draggable='true'] { cursor: grab; }
  .item:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .quiet { margin: 0; color: var(--faint); font-size: 13px; }
</style>
