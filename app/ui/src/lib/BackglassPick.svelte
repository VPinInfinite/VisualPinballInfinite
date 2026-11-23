<script>
  // Its backglass: the backglass files kept for the table, each with a picture of it — the one that
  // plays first — and one chosen to play, or none. The others stay beside the table, numbered as
  // VPin Studio keeps a backglass's versions, so either program sees the same ones.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, onclose } = $props();

  let choices = $state(null);
  let problem = $state('');
  let busy = $state(false);

  $effect(() => {
    invoke('backglass_choices', { id: t.id })
      .then((c) => (choices = c))
      .catch((e) => (problem = String(e)));
  });

  const playing = $derived(choices?.find((c) => c.n === 0));
  const size = (b) => (b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

  // `version`: its place among those kept (1 for the first), or null for none
  async function play(version) {
    busy = true;
    try {
      await invoke('backglass_play', { id: t.id, version: version === null ? null : version - 1 });
      notice(version === null ? `No backglass file plays for ${t.short} now.` : `${t.short} plays that backglass now.`, { kind: 'done' });
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="bg-title" data-loading={choices || problem ? undefined : ''}>
    <header>
      <h2 id="bg-title">Its backglass</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>

    {#if problem}
      <p class="problem">{problem}</p>
    {:else if !choices}
      <p class="soft">Looking…</p>
    {:else}
      <p class="soft">The one named as the table plays, in VPX too; the others are kept beside it, to choose from here.</p>
      <div class="grid">
        {#each choices as c (c.n)}
          <figure class:on={c.n === 0}>
            <img src={c.picture} alt="" />
            <figcaption>
              <span class="name" title={c.name}>{c.name}</span>
              <span class="soft">{size(c.bytes)}</span>
            </figcaption>
            {#if c.n === 0}
              <span class="badge now"><Icon name="check" size={13} width={2.6} />Plays</span>
            {:else}
              <button type="button" class="btn small go" disabled={busy} onclick={() => play(c.n)}>Play this one</button>
            {/if}
          </figure>
        {/each}
      </div>
      <footer>
        {#if playing}
          <button type="button" class="btn ghost" disabled={busy} title="Its backglass kept beside it with the others; a PinUP pack shows instead, where it has one" onclick={() => play(null)}>Play none</button>
        {/if}
        <button type="button" class="btn ghost" onclick={onclose}>Close</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(760px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; }
  figure { margin: 0; display: flex; flex-direction: column; gap: 8px; padding: 10px; border-radius: 12px; border: 1px solid var(--card-line); background: var(--card); }
  figure.on { border-color: var(--accent); }
  img { width: 100%; aspect-ratio: 4 / 3; object-fit: contain; border-radius: 8px; background: var(--picture-bg); }
  figcaption { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; }
  /* whole, as the names differ only at their ends (`… (1).directb2s`) */
  .name { overflow-wrap: anywhere; }
  figcaption .soft { white-space: nowrap; }
  .badge { align-self: flex-start; display: inline-flex; align-items: center; gap: 5px; padding: 3px 9px; border-radius: 6px; font-size: 12px; font-weight: 600; }
  .badge.now { background: var(--accent-bg); color: #fff; }
  figure .btn { align-self: flex-start; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  footer .btn:first-child { margin-right: auto; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
