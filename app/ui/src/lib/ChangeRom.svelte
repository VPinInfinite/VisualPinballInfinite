<script>
  // Change the ROM: the other ROMs known for the table, which are installed
  // and which aren't; choosing one says first what will change and asks — the script beside the
  // table made or changed, never the table file — and there's always the way back. "Select from
  // all ROMs" lists every set in VPinMAME's roms, one not known for the table with a warning.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, onclose } = $props();

  let choices = $state(null);
  let problem = $state('');
  let picked = $state(null);
  let effects = $state(null);
  let fromAll = $state(false);
  let query = $state('');
  let busy = $state(false);

  $effect(() => {
    invoke('rom_choices', { id: t.id })
      .then((c) => (choices = c))
      .catch((e) => (problem = String(e)));
  });

  const known = (set) => choices?.known.some((k) => k.set === set);
  const allShown = $derived(choices ? choices.all.filter((s) => !query.trim() || s.includes(query.trim().toLowerCase())).slice(0, 200) : []);

  async function pick(set) {
    picked = set;
    effects = null;
    try {
      effects = await invoke('rom_effects', { id: t.id, to: set });
    } catch (e) {
      effects = [String(e)];
    }
  }

  async function apply(to) {
    busy = true;
    try {
      await invoke('change_rom', { id: t.id, to });
      notice(to ? `${t.short} plays ${to} now.` : `${t.short} plays its own ROM again.`, { kind: 'done' });
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="cr-title" data-loading={choices || problem ? undefined : ''}>
    <header>
      <h2 id="cr-title">Change the ROM for {t.short}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>

    {#if problem}
      <p class="problem">{problem}</p>
    {:else if !choices}
      <p class="soft">Asking PinMAME about the ROMs known for it…</p>
    {:else if picked}
      <div class="chosen">
        <Icon name="chip" size={18} />
        <span><b>{picked}</b>{choices.known.find((k) => k.set === picked)?.description ? ` · ${choices.known.find((k) => k.set === picked).description}` : ''}</span>
        <button type="button" class="btn small ghost" onclick={() => (picked = null)}>Another</button>
      </div>
      {#if !known(picked)}
        <p class="warn"><Icon name="alert" size={16} />This ROM doesn’t match {t.name}’s, and may not work.</p>
      {/if}
      <h3 class="section-title">What changes</h3>
      {#if effects === null}
        <p class="soft">Looking…</p>
      {:else}
        <ul class="effects">{#each effects as e (e)}<li>{e}</li>{/each}</ul>
      {/if}
      <p class="soft">You can go back to its own ROM any time, here.</p>
      <footer>
        <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
        <button type="button" class="btn go" disabled={busy || effects === null} onclick={() => apply(picked)}><Icon name="check" size={16} />Change it</button>
      </footer>
    {:else}
      {#if choices.changed}
        <div class="was">
          <span>Changed here from <b>{choices.changed.from}</b>, its own.</span>
          <button type="button" class="btn small" disabled={busy} onclick={() => apply(null)}><Icon name="refresh" size={14} />Back to {choices.changed.from}</button>
        </div>
      {/if}
      {#if !fromAll}
        <h3 class="section-title">The ROMs known for it</h3>
        <div class="list">
          {#each choices.known as k (k.set)}
            <button type="button" class="rom" disabled={k.current || !k.have} onclick={() => pick(k.set)}>
              <span class="set">{k.set}</span>
              <span class="desc">{k.description || (k.have ? '' : 'Not one PinMAME knows')}</span>
              <span class="badges">
                {#if k.current}<span class="badge now">Its ROM now</span>{/if}
                {#if k.have}<span class="badge have">You have it</span>{:else}<span class="badge no">Not here</span>{/if}
                {#if k.onVps}<span class="badge">On VPS</span>{/if}
              </span>
            </button>
          {/each}
        </div>
        <p class="soft">One that isn’t here can be added from VPS’s ROMs on its page, then chosen here.</p>
        <footer>
          <button type="button" class="btn ghost" onclick={() => (fromAll = true)}>Select from all ROMs…</button>
          <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
        </footer>
      {:else}
        <h3 class="section-title">Every ROM you have</h3>
        <label class="search"><Icon name="search" /><input type="search" bind:value={query} placeholder="A set’s name" aria-label="Search your ROMs" /></label>
        <div class="list">
          {#each allShown as s (s)}
            <button type="button" class="rom" disabled={s === choices.current} onclick={() => pick(s)}>
              <span class="set">{s}</span>
              <span class="badges">{#if s === choices.current}<span class="badge now">Its ROM now</span>{:else if !known(s)}<span class="badge no">Not known for it</span>{/if}</span>
            </button>
          {/each}
        </div>
        <footer>
          <button type="button" class="btn ghost" onclick={() => (fromAll = false)}>Back to those known for it</button>
          <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
        </footer>
      {/if}
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(640px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  h3 { margin: 4px 0 0; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .list { display: flex; flex-direction: column; gap: 4px; max-height: 46vh; overflow: auto; }
  .rom { display: grid; grid-template-columns: 120px minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 9px 12px; border-radius: 10px; border: 1px solid var(--card-line); background: var(--card); text-align: left; color: var(--fg); }
  .rom:hover:not(:disabled) { border-color: var(--accent); }
  .rom:disabled { opacity: 0.7; cursor: default; }
  .set { font-weight: 700; font-family: ui-monospace, Consolas, monospace; font-size: 13px; }
  .desc { color: var(--muted); font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badges { display: flex; gap: 4px; }
  .badge { padding: 1px 7px; border-radius: 5px; background: var(--tag-bg); color: var(--tag-fg); font-size: 11px; font-weight: 600; white-space: nowrap; }
  .badge.have { background: #143024; color: var(--good); }
  .badge.no { background: #2a2214; color: var(--warn); }
  .badge.now { background: var(--accent-bg); color: #fff; }
  .chosen, .was { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 12px; background: var(--card); border: 1px solid var(--accent); }
  .was { border-color: var(--card-line); }
  .chosen span, .was span { flex: 1; min-width: 0; }
  .warn { display: flex; align-items: center; gap: 8px; margin: 0; color: var(--warn); font-size: 14px; }
  .effects { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 6px; font-size: 14px; color: var(--fg-soft); }
  .search { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 40px; border-radius: 10px; background: var(--bg); border: 1px solid #2a3242; color: var(--faint); }
  .search input { flex: 1; min-width: 0; background: transparent; border: 0; outline: none; color: var(--fg); font-size: 14px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  footer .btn:first-child { margin-right: auto; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
