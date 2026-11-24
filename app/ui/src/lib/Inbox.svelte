<script>
  // What's downloaded in the VPS browser's panel, waiting to be imported, in the top bar on every
  // screen while there's anything: how many, those still downloading, and a list with Import (the
  // drop window, each file to the table it was got for) and each one taken out, to the Recycle Bin.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { vps, notice, importWaiting } from './store.svelte.js';

  let open = $state(false);
  // the panel's page out of the way of the list
  $effect(() => {
    vps.inboxOpen = open;
    return () => (vps.inboxOpen = false);
  });
  const waiting = $derived(vps.inbox.filter((w) => !w.downloading));
  const downloading = $derived(vps.inbox.filter((w) => w.downloading));
  const size = (n) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} GB` : n >= 1e6 ? `${(n / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);

  function remove(w) {
    invoke('inbox_remove', { name: w.name }).then((l) => (vps.inbox = l)).catch((e) => notice(String(e), { kind: 'problem' }));
  }
  function importAll() {
    open = false;
    importWaiting();
  }
</script>

<!-- the list closed by a click anywhere else -->
<svelte:window onclick={(e) => open && !e.target.closest?.('.inbox-wrap') && (open = false)} />

<div class="inbox-wrap">
  <button type="button" class="btn" class:waiting={waiting.length} aria-expanded={open} onclick={() => (open = !open)} title="What you’ve downloaded in the VPS browser, waiting to be put in place">
    <Icon name="download" />{waiting.length} to import
  </button>
  {#if open}
    <div class="pop" role="dialog" aria-label="Waiting for import">
      <div class="section-title">Waiting for import</div>
      <div class="list">
        {#each vps.inbox as w (w.path)}
          <div class="item">
            <span class="name" title={w.name}>{w.name}</span>
            <span class="size">{w.downloading ? 'Downloading…' : size(w.size)}</span>
            {#if !w.downloading}<button type="button" class="btn icon ghost small" aria-label="Take {w.name} out" title="Take it out (to the Recycle Bin)" onclick={() => remove(w)}><Icon name="x" size={14} /></button>{/if}
          </div>
        {/each}
      </div>
      {#if downloading.length}<p class="quiet">{downloading.length} still downloading: they can be imported once they’re done.</p>{/if}
      <div class="buttons">
        <button type="button" class="btn small ghost" onclick={() => invoke('inbox_show').catch((e) => notice(String(e), { kind: 'problem' }))}><Icon name="folder" size={14} />Show the folder</button>
        <button type="button" class="btn small go" disabled={!waiting.length} onclick={importAll}><Icon name="inbox" size={14} />Import{waiting.length > 1 ? ` all ${waiting.length}` : ''}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .inbox-wrap { position: relative; }
  .waiting { border-color: var(--accent); color: var(--fg); }
  .pop {
    position: absolute; right: 0; top: calc(100% + 8px); z-index: 70; width: 400px; max-width: calc(100vw - 40px);
    display: flex; flex-direction: column; gap: 10px; padding: 12px;
    background: #182031; border: 1px solid #2a3550; border-radius: 12px; box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5);
  }
  .list { display: flex; flex-direction: column; gap: 4px; max-height: 320px; overflow-y: auto; }
  .item { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; background: #141a26; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .size { flex: 0 0 auto; color: var(--faint); font-size: 12px; }
  .quiet { margin: 0; color: var(--muted); font-size: 12.5px; }
  .buttons { display: flex; justify-content: space-between; gap: 8px; }
</style>
