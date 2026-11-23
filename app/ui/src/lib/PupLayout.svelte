<script>
  // A PinUP pack's Layout window: its layouts as their authors named them, the one in place
  // marked. Choosing one has the app make the copies its option file describes, inside the pack's
  // folder — the file itself never run — and one the app won't apply says why, with its file to
  // run by hand. Try it starts the table with a layout before it's chosen, on the user's screens,
  // the pack left as it is (`try_layout`); the window stays for the choice once the table's closed.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { path, title, tableId = null, onclose, ondone = () => {} } = $props();

  let pack = $state(null);
  let problem = $state('');
  let picked = $state(null);
  let busy = $state(false);

  $effect(() => {
    invoke('pup_pack', { path })
      .then((p) => {
        pack = p;
        picked = p.layouts.current ?? null;
      })
      .catch((e) => (problem = String(e)));
  });

  const current = $derived(pack?.layouts.current ?? null);
  // what's there now, in words, above the choices
  const now = $derived.by(() => {
    if (!pack) return '';
    const l = pack.layouts.layouts.find((x) => x.file === current);
    if (l) return `In place now: ${l.name}.`;
    if (pack.facts.look === 'bad') return 'None is chosen yet, so the pack shows nothing: pick the one for your screens.';
    return 'In place now: a setup of its own, none of these. Choosing one replaces it.';
  });

  async function use() {
    busy = true;
    try {
      const r = await invoke('pup_use_layout', { path, file: picked });
      notice(r.said, { kind: r.look });
      ondone();
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  async function tryIt(l) {
    try {
      await invoke('try_layout', { id: tableId, path, file: l.file });
      notice(`${title} is starting with “${l.name}”, the pack left as it is. Close the table to come back and choose.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  function showFile(file) {
    invoke('pup_pack_show', { path, file }).catch((e) => notice(String(e), { kind: 'problem' }));
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="pl-title" data-loading={pack || problem ? undefined : ''}>
    <header>
      <h2 id="pl-title">{title}’s PinUP layout</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>

    {#if problem}
      <p class="problem">{problem}</p>
    {:else if !pack}
      <p class="soft">Reading the pack’s layouts…</p>
    {:else}
      <p class="lead">Packs are made for different screens. Pick the layout that matches yours, as its author named it.{#if tableId}{' '}<b>Try it</b> starts {title} with that layout on your screens, without changing the pack; close the table to come back here.{/if}</p>
      <p class="now">{now}</p>
      <div class="list" role="radiogroup" aria-label="Its layouts">
        {#each pack.layouts.layouts as l (l.file)}
          <div class="opt" class:off={l.refused} class:on={picked === l.file}>
            <label class="pick">
              <input type="radio" name="layout" value={l.file} bind:group={picked} disabled={!!l.refused || busy} />
              <span class="oname">{l.name}{#if l.file === current}<span class="badge now">In place</span>{/if}</span>
            </label>
            {#if tableId && !l.refused}<button type="button" class="btn small ghost try" title="Starts {title} with this layout on your screens, without changing the pack" onclick={() => tryIt(l)}><Icon name="play" size={13} />Try it</button>{/if}
            {#if l.refused}
              <span class="why">{l.refused} <button type="button" class="btn small ghost" onclick={() => showFile(l.file)}>Its file</button></span>
            {:else if l.note}
              <span class="note">{l.note}</span>
            {/if}
          </div>
        {/each}
      </div>
      <p class="soft">The app copies what’s described in the option’s files, inside the pack’s folder. It never runs the files themselves. A file of yours it replaces goes to the Recycle Bin.</p>
      <footer>
        <button type="button" class="btn ghost" onclick={() => showFile(null)}><Icon name="folder" size={15} />Files</button>
        <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
        <button type="button" class="btn go" disabled={busy || !picked || picked === current} onclick={use}><Icon name="check" size={16} />{busy ? 'Making its copies…' : 'Use this layout'}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(680px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .lead { margin: 0; color: var(--fg-soft); font-size: 14px; }
  .now { margin: 0; color: var(--fg); font-size: 14px; font-weight: 600; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .list { display: flex; flex-direction: column; gap: 4px; max-height: 50vh; overflow: auto; }
  .opt { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 4px 10px; align-items: center; padding: 8px 12px; border-radius: 10px; border: 1px solid var(--card-line); background: var(--card); color: var(--fg); }
  .pick { display: grid; grid-template-columns: 22px minmax(0, 1fr); gap: 10px; align-items: center; min-height: 30px; cursor: pointer; }
  .try { gap: 5px; }
  .opt:hover:not(.off) { border-color: var(--accent); }
  .opt.on { border-color: var(--accent); }
  .opt.off { opacity: 0.75; }
  .opt.off .pick { cursor: default; }
  .opt input { accent-color: var(--accent); margin: 0; }
  .oname { font-size: 14px; display: inline-flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .why, .note { grid-column: 1 / -1; padding-left: 32px; font-size: 12px; line-height: 1.45; }
  .why { color: var(--warn); }
  .note { color: var(--muted); }
  .badge { padding: 1px 7px; border-radius: 5px; font-size: 11px; font-weight: 600; white-space: nowrap; }
  .badge.now { background: var(--accent-bg); color: #fff; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  footer .btn:first-child { margin-right: auto; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
