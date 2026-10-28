<script>
  // A table taken out of the library, and what goes with it: each file and folder it holds, its ROM
  // and what's kept for it, its PinUP pack, PinUP Popper's media and game, its high scores, its
  // play log — each ticked or not to start with as the engine's plan says (anything another table
  // uses too unticked, with which), the ticked moved to the Recycle Bin or taken out. Opened from the
  // … menu on its page or a right-click on its tile.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, notice, goLibrary } from './store.svelte.js';

  let { t, onclose } = $props();

  let items = $state(null);
  let ticked = $state({});
  let busy = $state(false);
  let problem = $state('');

  invoke('remove_plan', { id: t.id })
    .then((list) => {
      items = list;
      ticked = Object.fromEntries(list.map((i) => [i.key, i.ticked]));
    })
    .catch((e) => (problem = String(e)));

  const chosen = $derived(items ? items.filter((i) => ticked[i.key]) : []);
  const files = $derived(chosen.reduce((n, i) => n + i.files, 0));
  const size = (b) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${Math.round(b / 1e6)} MB` : b >= 1e3 ? `${Math.round(b / 1e3)} KB` : `${b} bytes`);
  const names = (list) => (list.length > 2 ? `${list.slice(0, 2).join(', ')} and ${list.length - 2} more` : list.join(' and '));

  async function go() {
    busy = true;
    problem = '';
    try {
      const done = await invoke('remove', { id: t.id, keys: chosen.map((i) => i.key) });
      const what = done.tableGone ? `${t.short} is out of your library` : `Done for ${t.short}`;
      const moved = done.recycled ? `: ${done.recycled} moved to the Recycle Bin` : '';
      notice(`${what}${moved}.${done.problems.length ? ` ${done.problems.join(' ')}` : ''}`, { kind: done.problems.length ? 'problem' : 'done' });
      onclose();
      if (done.tableGone && app.page === 'table' && app.tableId === t.id) goLibrary();
    } catch (e) {
      problem = String(e);
    }
    busy = false;
  }

  function key(e) {
    if (e.key === 'Escape' && !busy) onclose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim">
  <div class="win" role="dialog" aria-modal="true" aria-labelledby="rm-title">
    <header>
      <div>
        <h1 id="rm-title" class="display">Remove {t.short}?</h1>
        <p>Ticked files go to the Recycle Bin, where you can get them back. Anything another table uses too starts unticked.</p>
      </div>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose} disabled={busy}><Icon name="x" /></button>
    </header>
    <div class="body">
      {#if !items && !problem}
        <p class="wait">Looking at what it holds…</p>
      {:else if items}
        <ul>
          {#each items as i (i.key)}
            <li class:off={!ticked[i.key]}>
              <label>
                <input type="checkbox" bind:checked={ticked[i.key]} disabled={busy} />
                <span class="what">{i.what}</span>
                <span class="name" title={i.name}>{i.name}</span>
                <span class="size">{i.files > 1 ? `${i.files} files` : ''}{i.files > 1 && i.bytes ? ' · ' : ''}{i.bytes ? size(i.bytes) : ''}</span>
              </label>
              {#if i.shared.length}<p class="note warn">Also used by {names(i.shared)}, which needs it too.</p>{/if}
              {#if i.key === 'play-log'}<p class="note">Forgotten for good: this can’t be undone.</p>{/if}
              {#if i.key === 'popper-game'}<p class="note">PinUP Popper’s list is kept as a copy first.</p>{/if}
            </li>
          {/each}
        </ul>
      {/if}
      {#if problem}<p class="problem">{problem}</p>{/if}
    </div>
    <footer>
      <button type="button" class="btn ghost" onclick={onclose} disabled={busy}>Cancel</button>
      <button type="button" class="btn danger" onclick={go} disabled={busy || !chosen.length}>
        <Icon name="trash" size={16} />{busy ? 'Removing…' : files ? `Move ${files} ${files === 1 ? 'file' : 'files'} to the Recycle Bin` : 'Remove what’s ticked'}
      </button>
    </footer>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 45; background: rgba(5, 7, 11, 0.72); display: flex; align-items: center; justify-content: center; padding: 28px; }
  .win { width: min(720px, 100%); max-height: 100%; display: flex; flex-direction: column; background: var(--panel); border: 1px solid var(--panel-line); border-radius: 18px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6); }
  header { display: flex; justify-content: space-between; gap: 16px; padding: 22px 24px 14px; border-bottom: 1px solid var(--line-soft); }
  h1 { font-size: 28px; line-height: 1.05; margin: 0; }
  header p { margin: 6px 0 0; color: var(--muted); }
  .body { overflow: auto; padding: 10px 24px 14px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { padding: 10px 0; border-bottom: 1px solid var(--line-soft); }
  li:last-child { border-bottom: 0; }
  label { display: flex; align-items: center; gap: 12px; cursor: pointer; }
  input[type='checkbox'] { width: 17px; height: 17px; flex: none; accent-color: var(--accent); }
  .what { flex: none; width: 210px; font-weight: 600; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-soft); }
  .size { flex: none; color: var(--faint); font-size: 13px; }
  li.off .what, li.off .name { color: var(--faint); }
  .note { margin: 4px 0 0 29px; font-size: 13px; color: var(--muted); }
  .note.warn { color: var(--warn); }
  .wait { color: var(--muted); }
  .problem { margin: 10px 0 0; color: #f0a0b0; font-size: 14px; }
  footer { display: flex; gap: 12px; justify-content: flex-end; padding: 14px 24px 18px; border-top: 1px solid var(--line-soft); }
  .danger { display: inline-flex; gap: 6px; align-items: center; color: #ffd0d0; border-color: #a64552; background: #3a1d24; }
  .danger:hover:not(:disabled) { background: #4a222b; }
</style>
