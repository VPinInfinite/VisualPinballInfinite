<script>
  // The table inspector, the table editor's first stage: a table seen from above — its real look
  // with its parts' outlines over it — the parts by kind down the left, folded or not
  // (`PartsPanel`), the chosen part on the right: what it is, what achievements can count of it
  // and everything it's set to. Read only: nothing in the table changes. Its achievements, the
  // second stage, open on the same parts. A table chosen in its header, found by name; with none
  // chosen yet, the finder fills the page.
  import { invoke } from '@tauri-apps/api/core';
  import AchievementsEditor from './AchievementsEditor.svelte';
  import Icon from './Icon.svelte';
  import PartsPanel from './PartsPanel.svelte';
  import PlayfieldView from './PlayfieldView.svelte';
  import { ORDER } from './playfield.js';
  import { app, leaving, hiddenParts, hidePart } from './store.svelte.js';

  let { id } = $props();

  let data = $state(null);
  let problem = $state(null);
  /** the kinds left off the drawing */
  let hidden = $state({ Flasher: true, Primitive: true });
  /** the chosen part, by its place in the table */
  let chosen = $state(null);

  const table = $derived(app.tables.find((t) => t.id === id));

  $effect(() => {
    const wanted = id;
    data = null;
    problem = null;
    chosen = null;
    if (!wanted) return;
    invoke('inspect_table', { id: wanted })
      .then((d) => {
        if (wanted !== id) return;
        d.parts.forEach((p, i) => (p.i = i));
        data = d;
        chosen = (d.parts.find((p) => p.kind === 'Flipper') || d.parts[0])?.i ?? null;
      })
      .catch((e) => {
        if (wanted === id) problem = String(e);
      });
  });

  const editing = $derived(app.inspectMode === 'achievements');
  const drawn = $derived(data ? data.parts.filter((p) => !hidden[p.kind] && p.outline.kind !== 'none').sort((a, b) => ORDER.indexOf(b.kind) - ORDER.indexOf(a.kind)) : []);
  const part = $derived(data && chosen !== null ? data.parts[chosen] : null);

  // the table finder: the library's tables by name
  let finding = $state(false);
  let find = $state('');
  let finder = $state();
  const found = $derived.by(() => {
    const q = find.trim().toLowerCase();
    const all = [...app.tables].sort((a, b) => a.name.localeCompare(b.name));
    return q ? all.filter((t) => t.name.toLowerCase().includes(q) || t.file.toLowerCase().includes(q)) : all;
  });
  function open(t) {
    leaving(() => {
      finding = false;
      find = '';
      app.inspectId = t.id;
    });
  }
  $effect(() => {
    if (finding) finder?.focus();
  });

  // a setting's name in words: is_visible is "Is visible"
  function words(key) {
    const w = key.replace(/_/g, ' ');
    return w.charAt(0).toUpperCase() + w.slice(1);
  }
  const value = (v) => (typeof v === 'boolean' ? (v ? 'Yes' : 'No') : String(v));

  // what an achievement counts of it, in words
  function phrase(p, e) {
    if (e === 'spin') return 'each spin';
    if (e === 'slingshot') return 'each time its slingshot fires';
    return `each time it’s ${{ Trigger: 'rolled over', Kicker: 'entered', Gate: 'passed through' }[p.kind] || 'hit'}`;
  }
  // why achievements can't count it
  function why(p) {
    const s = Object.fromEntries(p.settings);
    if (s.is_collidable === false) return 'The ball passes through it, so achievements can’t count it.';
    return 'Nothing the ball does to it is told to achievements.';
  }
  // what's worth saying of a part they can count
  function about(p) {
    const s = Object.fromEntries(p.settings);
    const notes = [];
    if (s.hit_event === false || s.use_hit_event === false) notes.push('The table has its hits off: the player watches it for achievements itself, the table playing as it always does.');
    if (s.is_enabled === false) notes.push('It’s switched off when the table starts: it counts once the table switches it on.');
    return notes;
  }
</script>

{#snippet tableFinder(big)}
  <div class="finder" class:big>
    <label class="find">
      <Icon name="search" size={16} />
      <input bind:this={finder} type="search" placeholder="Find a table to open" aria-label="Find a table to open" bind:value={find} spellcheck="false" />
    </label>
    <div class="tables">
      {#each found as t (t.id)}
        <button type="button" class="tbl" class:on={t.id === id} onclick={() => open(t)}>
          <span class="name">{t.name}</span>
          <span class="by">{[t.maker, t.year, t.version && `version ${t.version}`].filter(Boolean).join(' · ')}</span>
        </button>
      {/each}
      {#if !found.length}<p class="none">No table’s name has “{find.trim()}” in it.</p>{/if}
    </div>
  </div>
{/snippet}

{#snippet titleRow()}
  <h1 class="display">{table?.name ?? data?.file ?? 'A table'}</h1>
  <div class="pick-table">
    <button type="button" class="btn small ghost" aria-expanded={finding} title="Open another table" onclick={() => (finding = !finding)}><Icon name="search" size={15} />Another table</button>
    {#if finding}
      <div class="drop" role="dialog" aria-label="Open another table">{@render tableFinder(false)}</div>
    {/if}
  </div>
  <div class="modes" role="tablist" aria-label="What it shows">
    <button type="button" role="tab" class="btn small" class:on={!editing} aria-selected={!editing} onclick={() => (app.inspectMode = 'parts')}><Icon name="layers" size={15} />Its parts</button>
    <button type="button" role="tab" class="btn small" class:on={editing} aria-selected={editing} onclick={() => (app.inspectMode = 'achievements')}><Icon name="trophy" size={15} />Its achievements</button>
  </div>
{/snippet}

<div class="inspector" data-loading={id && !data && !problem ? '' : undefined}>
  <header class="head">
    <div class="row">
      <span class="eyebrow">{editing ? 'The achievements editor' : 'The table inspector'}</span>
      <span class="dev">full editor still in development</span>
    </div>
    {#if id && !(editing && data)}
      <div class="title-row">{@render titleRow()}</div>
    {:else if !id}
      <h1 class="display">Table editor</h1>
      <p>Open a table to see it from above, part by part, and to make its achievements. Tables are still made in VPX’s editor and played here as it makes them; changing a table here comes later.</p>
    {/if}
  </header>

  {#if !id}
    {@render tableFinder(true)}
  {:else if problem}
    <p class="problem"><Icon name="alert" size={16} />{problem}</p>
  {:else if !data}
    <p class="quiet">Reading the table’s parts…</p>
  {:else if editing}
    <AchievementsEditor {id} {data} bind:hidden header={titleRow} />
  {:else}
    <div class="grid">
      <PartsPanel parts={data.parts} {chosen} bind:hidden table={id} onpick={(p) => (chosen = p.i)} />

      <div class="panel field">
        <PlayfieldView {id} bounds={data.bounds} {drawn} glows={part ? [{ p: part, strong: true }] : []} onpart={(i) => (chosen = i)} />
      </div>

      <div class="panel details" aria-live="polite">
        {#if part}
          {@const gone = hiddenParts(id).includes(part.name)}
          <div class="namerow">
            <h2>{part.name}</h2>
            <button type="button" class="eyebtn" class:gone title={gone ? 'Hidden on the playfield: show it' : 'Hide it on the playfield'} aria-label={gone ? 'Show it' : 'Hide it'} onclick={() => hidePart(id, part.name, !gone)}><Icon name={gone ? 'eyeOff' : 'eye'} size={16} /></button>
          </div>
          <div class="facts">
            <span class="chip k">{part.kind}</span>
            <span class="chip">{part.visible ? 'Seen' : 'Hidden'}</span>
            {#if part.layer}<span class="chip">Layer: {part.layer}</span>{/if}
            {#if part.outline.kind === 'none'}<span class="chip">Not on the playfield</span>{/if}
          </div>
          {#if part.counts.length}
            <div class="counts">
              <p>Achievements can count {part.counts.map((e) => phrase(part, e)).join(' and ')} — 10 times in one ball, say.</p>
              {#each about(part) as note (note)}<p class="aside">{note}</p>{/each}
              <p class="in-pack">In a pack:</p>
              {#each part.counts as e (e)}<code>{`{ "part": "${part.name}", "does": "${e}" }`}</code>{/each}
            </div>
          {:else}
            <p class="quiet why">{why(part)}</p>
          {/if}
          <p class="note">{part.settings.length} settings, as the table file keeps them. Read only for now.</p>
          <table>
            <tbody>
              {#each part.settings as [k, v] (k)}
                <tr><td>{words(k)}</td><td>{value(v)}</td></tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="quiet">The table has no parts to show.</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .inspector { padding: 14px 28px 24px; display: flex; flex-direction: column; gap: 12px; }
  .head { display: flex; flex-direction: column; gap: 4px; align-items: flex-start; }
  .head .row { display: flex; align-items: baseline; gap: 12px; }
  .eyebrow { font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.12em; color: var(--accent); }
  .dev { font-size: 12px; color: var(--faint); }
  .head h1 { margin: 0; font-size: 32px; line-height: 1; text-wrap: balance; }
  .head p { margin: 4px 0 0; color: var(--muted); font-size: 14px; line-height: 1.5; max-width: 720px; }
  .title-row { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; }
  .display { margin: 0; font-size: 32px; line-height: 1; text-wrap: balance; }
  .modes { display: flex; gap: 6px; }
  .pick-table { position: relative; }
  .drop { position: absolute; top: 36px; left: 0; z-index: 30; width: min(560px, 80vw); padding: 10px; border-radius: 12px; background: #151a24; border: 1px solid var(--control-line); box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55); }
  .finder { display: flex; flex-direction: column; gap: 8px; }
  .finder.big { max-width: 1000px; }
  .find { display: flex; align-items: center; gap: 8px; height: 38px; padding: 0 12px; border-radius: 9px; border: 1px solid var(--control-line); background: var(--bg); color: var(--faint); }
  .find:focus-within { border-color: var(--accent); }
  .find input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: var(--fg); font: inherit; font-size: 14px; }
  .tables { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 4px; max-height: 340px; overflow: auto; padding-right: 4px; }
  .finder.big .tables { max-height: calc(100vh - 290px); }
  .tbl { display: flex; flex-direction: column; align-items: flex-start; gap: 1px; padding: 7px 10px; border: 1px solid transparent; border-radius: 8px; background: var(--panel); color: var(--fg); font: inherit; text-align: left; cursor: pointer; min-width: 0; }
  .tbl:hover { border-color: var(--accent); }
  .tbl.on { border-color: var(--accent); }
  .tbl .name { font-size: 14px; font-weight: 600; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tbl .by { font-size: 12px; color: var(--faint); }
  .none { margin: 0; color: var(--faint); font-size: 13px; }
  .problem { display: flex; gap: 8px; align-items: center; margin: 0; color: var(--bad); }
  .quiet { margin: 0; color: var(--faint); font-size: 13px; line-height: 1.5; }
  .grid { display: grid; grid-template-columns: 260px minmax(0, 1fr) 360px; gap: 14px; align-items: start; }
  .panel { background: var(--card); border: 1px solid var(--card-line); border-radius: 14px; min-width: 0; }
  .field { padding: 10px; }
  .details { padding: 16px; display: flex; flex-direction: column; gap: 12px; max-height: calc(100vh - 225px); overflow: auto; }
  .details h2 { margin: 0; font: 600 18px Consolas, ui-monospace, monospace; word-break: break-all; }
  .namerow { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; }
  .eyebtn { flex: 0 0 auto; width: 30px; height: 30px; display: grid; place-items: center; border: 1px solid var(--control-line); border-radius: 8px; background: var(--control); color: var(--fg-soft); cursor: pointer; }
  .eyebtn.gone { color: var(--warn); border-color: color-mix(in srgb, var(--warn) 50%, var(--control-line)); }
  .facts { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { padding: 3px 9px; border-radius: 999px; background: var(--control); color: var(--fg-soft); font-size: 12px; }
  .chip.k { background: var(--accent-bg); color: #fff; }
  .counts { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border-radius: 10px; background: #13202c; border: 1px solid #1e3a50; }
  .counts p { margin: 0; color: var(--fg-soft); font-size: 13px; line-height: 1.5; }
  .counts .in-pack { color: var(--faint); font-size: 12px; }
  .counts .aside { color: var(--muted); font-size: 12px; }
  .counts code { font: 12px Consolas, ui-monospace, monospace; color: var(--accent-soft); word-break: break-all; }
  .why { padding: 10px 12px; border-radius: 10px; background: var(--panel); border: 1px solid var(--panel-line); }
  .note { margin: 0; color: var(--faint); font-size: 12px; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  td { padding: 5px 4px; border-top: 1px solid var(--line-soft); vertical-align: top; }
  td:first-child { color: var(--faint); width: 50%; }
  td:last-child { color: var(--fg); font-family: Consolas, ui-monospace, monospace; word-break: break-word; }
  @media (max-width: 1180px) {
    .grid { grid-template-columns: 1fr; }
    .details { max-height: 420px; }
  }
</style>
