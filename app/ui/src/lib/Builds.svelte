<script>
  // A table's builds, at the top of What it holds: where the library shows a table with more than
  // one table file once (Settings › Library), a tab for each of its table files — picking one opens
  // its page, which follows it: its files, what PLAY plays, its plays and its achievements — the
  // one picked chosen as the table's main build (its tile shows and plays it, whichever was played
  // last), or taken out ("Not this table"), and a table file put in by hand ("Add a build…").
  // Above them, in either view, a table file that looks like this one saved twice: which to keep.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, buildsOf, buildLabel, combined, notice, openTable, tableById, versionOrder } from './store.svelte.js';

  let { t } = $props();

  // newest first, as their versions read; a tab's place never moves with the one picked
  const builds = $derived(
    [...buildsOf(t)].sort((a, b) => versionOrder(b.ownVersion || b.version, a.ownVersion || a.version) || b.added - a.added || a.file.localeCompare(b.file)),
  );
  const many = $derived(combined() && builds.length > 1);
  const lastPlayed = $derived(builds.reduce((best, b) => ((b.played?.last || 0) > (best?.played?.last || 0) ? b : best), null));

  // a table file put in by hand: the library's others, by name
  let adding = $state(false);
  let addId = $state('');
  const others = $derived(adding ? app.tables.filter((x) => x.group !== t.group).sort((a, b) => a.name.localeCompare(b.name) || a.file.localeCompare(b.file)) : []);

  // the same build twice, asked about until kept or answered (Not now: until the app starts again)
  const twin = $derived(t.twin ? tableById(t.twin) : null);
  let notNow = $state(new Set());
  const asking = $derived(twin && !notNow.has([t.id, twin.id].sort().join('|')));
  // the one to keep: the one played (most), else the one named without a copy's mark
  const copyMark = /(\s*\(\d{1,3}\)|\s*-\s*copy(\s*\(\d+\))?|\s+copy)$/i;
  const keep = $derived.by(() => {
    if (!twin) return null;
    const plays = (x) => x.played?.count || 0;
    if (plays(t) !== plays(twin)) return plays(t) > plays(twin) ? t : twin;
    if (copyMark.test(t.file) !== copyMark.test(twin.file)) return copyMark.test(t.file) ? twin : t;
    return (t.played?.last || 0) >= (twin.played?.last || 0) ? t : twin;
  });
  const drop = $derived(keep && twin ? (keep.id === t.id ? twin : t) : null);

  const size = (b) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);
  const day = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { month: 'long', day: 'numeric', year: 'numeric' }) : '—');
  const plays = (b) => (b.played?.count ? `${b.played.count === 1 ? 'Once' : `${b.played.count} times`}, last on ${day(b.played.last)}` : 'Never');

  async function call(cmd, args, done) {
    try {
      await invoke(cmd, args);
      if (done) done();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  function apart() {
    const id = t.id;
    const name = buildLabel(t);
    call('build_apart', { id, apart: true }, () =>
      notice(`${name} is a table of its own now.`, { kind: 'done', action: { label: 'Undo', run: () => call('build_apart', { id, apart: false }) } }),
    );
  }

  function add() {
    if (!addId) return;
    const id = addId;
    call('build_add', { id, to: t.id }, () => {
      adding = false;
      addId = '';
      notice(`${tableById(id)?.file ?? 'The table file'} is a build of ${t.short} now.`, { kind: 'done' });
    });
  }
</script>

{#if asking}
  <div class="twice" role="group" aria-label="This table file is here twice">
    <div class="head"><Icon name="alert" size={17} width={2.4} />{t.short} is here twice</div>
    <p>{twin.file} looks like this table file saved again: the same name but for a copy’s mark{t.rom ? `, the same ROM (${t.rom})` : ''}. The app can’t tell what changed inside, so it asks which to keep.</p>
    <div class="cmp">
      <table>
        <thead><tr><th></th><th>{t.file}</th><th>{twin.file}</th></tr></thead>
        <tbody>
          <tr><td>Size</td><td>{size(t.size)}</td><td>{size(twin.size)}</td></tr>
          <tr><td>File date</td><td>{day(t.changed)}</td><td>{day(twin.changed)}</td></tr>
          <tr><td>Your plays</td><td>{plays(t)}</td><td>{plays(twin)}</td></tr>
        </tbody>
      </table>
    </div>
    <div class="actions">
      <button type="button" class="btn small primary" onclick={() => (app.removing = drop)}>Keep {keep.played?.count ? 'the one you’ve played' : keep.id === t.id ? 'this one' : 'the other one'}</button>
      <button type="button" class="btn small" onclick={() => call('builds_keep_both', { id: t.id })}>Keep both as builds</button>
      <button type="button" class="btn small ghost" onclick={() => (notNow = new Set([...notNow, [t.id, twin.id].sort().join('|')]))}>Not now</button>
    </div>
    <p class="soft">Keeping {keep.file} opens the window to remove {drop.file}: its table file, its backglass and its settings file go to the Recycle Bin, ticked as you choose there.</p>
  </div>
{/if}

{#if many || (combined() && adding)}
  <div class="tabs" role="tablist" aria-label="Its builds">
    {#each builds as b (b.id)}
      <button type="button" role="tab" class="tab" aria-selected={b.id === t.id} onclick={() => b.id !== t.id && openTable(b.id)}>
        <b>{#if b.mainBuild}<Icon name="star" size={13} fill />{/if}{buildLabel(b)}</b>
        <small>{[b.rom, lastPlayed?.id === b.id ? 'played last' : '', b.mainBuild ? 'main build' : ''].filter(Boolean).join(' · ') || b.file}</small>
      </button>
    {/each}
    <button type="button" class="tab add" aria-expanded={adding} onclick={() => (adding = !adding)}><Icon name="plus" size={14} />Add a build…</button>
  </div>
  <div class="this">
    <span class="soft">{t.file}</span>
    {#if many}
      <button type="button" class="btn small ghost" aria-pressed={t.mainBuild} title="Its tile shows and plays this build, whichever you played last" onclick={() => call('build_main', { id: t.id, on: !t.mainBuild })}><Icon name="star" size={14} fill={t.mainBuild} />{t.mainBuild ? 'Its main build' : 'Make it the main build'}</button>
      <button type="button" class="btn small ghost" title="Show this table file as a table of its own" onclick={apart}>Not this table</button>
    {/if}
  </div>
{:else if combined()}
  <div class="this solo">
    <button type="button" class="btn small ghost" aria-expanded={adding} onclick={() => (adding = !adding)}><Icon name="plus" size={14} />Add a build…</button>
  </div>
{/if}

{#if adding}
  <form class="adding" onsubmit={(e) => { e.preventDefault(); add(); }}>
    <label>
      <span>Another table file of {t.short}</span>
      <select class="select" bind:value={addId}>
        <option value="">Choose a table file…</option>
        {#each others as x (x.id)}<option value={x.id}>{x.name} — {x.file}</option>{/each}
      </select>
    </label>
    <button type="submit" class="btn small" disabled={!addId}>Add it as a build</button>
    <button type="button" class="btn small ghost" onclick={() => { adding = false; addId = ''; }}>Cancel</button>
  </form>
{/if}

<style>
  .twice { margin: 4px 0 14px; padding: 14px 16px; border-radius: 12px; border: 1px solid #5a4a22; background: #2a2414; display: flex; flex-direction: column; gap: 10px; }
  .twice .head { display: flex; align-items: center; gap: 8px; color: var(--warn); font-weight: 700; }
  .twice p { margin: 0; color: var(--fg-soft); font-size: 13px; }
  .cmp { overflow-x: auto; border: 1px solid #5a4a22; border-radius: 10px; background: rgba(13, 16, 22, 0.55); }
  .cmp table { border-collapse: collapse; width: 100%; font-size: 13px; }
  .cmp th, .cmp td { text-align: left; padding: 7px 10px; border-bottom: 1px solid rgba(90, 74, 34, 0.5); vertical-align: top; }
  .cmp tr:last-child td { border-bottom: 0; }
  .cmp th { font-weight: 600; color: var(--fg); word-break: break-word; }
  .cmp td:first-child { color: var(--muted); white-space: nowrap; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .soft { color: var(--muted); font-size: 13px; }
  .tabs { display: flex; flex-wrap: wrap; gap: 4px 18px; border-bottom: 1px solid var(--panel-line); margin: 2px 0 8px; }
  .tab { appearance: none; background: none; border: 0; border-bottom: 2px solid transparent; margin-bottom: -1px; padding: 6px 2px 9px; color: var(--muted); font: inherit; text-align: left; cursor: pointer; display: flex; flex-direction: column; gap: 1px; }
  .tab b { display: inline-flex; align-items: center; gap: 5px; font-size: 14px; font-weight: 600; color: inherit; }
  .tab b :global(svg) { color: #ffc65c; }
  .tab small { font-size: 12px; color: var(--muted); }
  .tab[aria-selected='true'] { color: var(--fg); border-bottom-color: var(--accent); }
  .tab:hover { color: var(--fg); }
  .tab.add { flex-direction: row; align-items: center; gap: 6px; color: var(--accent); font-size: 13px; font-weight: 600; }
  .this { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 10px; margin: 0 0 10px; }
  .this .soft { flex: 1 1 260px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .this.solo { margin-top: -4px; }
  .adding { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 8px; margin: 0 0 12px; }
  .adding label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: var(--muted); flex: 1 1 320px; min-width: 0; }
  .adding select { width: 100%; }
</style>
