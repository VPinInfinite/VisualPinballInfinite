<script>
  // A table's screenshots (a Screenshots tile beside its media opening them,
  // those taken at a high score and with the Screenshot key): each moment's pictures together, the
  // newest first; one opened large, left and right through the rest from there, by the arrows or
  // the arrow keys; one deleted on a right-click (to the Recycle Bin).
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, onclose } = $props();

  let shots = $state(null);
  let large = $state(null);
  let asking = $state(null);

  async function load() {
    shots = await invoke('screenshots', { id: t.id }).catch(() => []);
  }
  $effect(() => {
    void t.id;
    load();
  });

  // each moment's pictures, the newest first
  const moments = $derived.by(() => {
    const by = new Map();
    for (const s of shots || []) {
      if (!by.has(s.when)) by.set(s.when, []);
      by.get(s.when).push(s);
    }
    return [...by];
  });

  const when = (stamp) => {
    const m = stamp.match(/^(\d{4})-(\d{2})-(\d{2}) (\d{2})-(\d{2})-(\d{2})$/);
    return m ? new Date(+m[1], m[2] - 1, +m[3], +m[4], +m[5], +m[6]).toLocaleString(undefined, { day: 'numeric', month: 'long', year: 'numeric', hour: 'numeric', minute: '2-digit' }) : stamp;
  };

  async function remove(s) {
    asking = null;
    try {
      await invoke('delete_media', { id: t.id, url: s.url });
      if (large?.url === s.url) large = null;
      await load();
      notice('The screenshot went to the Recycle Bin.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // the one before or after the one open large, in the order they show, round from the last
  function step(by) {
    const list = shots || [];
    if (!large || list.length < 2) return;
    const at = list.findIndex((s) => s.url === large.url);
    large = list[(at + by + list.length) % list.length];
  }

  function key(e) {
    if (e.key === 'Escape') large ? (large = null) : onclose();
    else if (large && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      step(e.key === 'ArrowLeft' ? -1 : 1);
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ss-title">
    <header>
      <h2 id="ss-title">Screenshots of {t.short}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    {#if large}
      <figure class="large">
        <div class="view">
          <img src={large.url} alt="{large.window} at {when(large.when)}" />
          {#if shots.length > 1}
            <button type="button" class="arrow left" aria-label="The one before" onclick={() => step(-1)}><Icon name="chevL" size={22} /></button>
            <button type="button" class="arrow right" aria-label="The one after" onclick={() => step(1)}><Icon name="chevR" size={22} /></button>
          {/if}
        </div>
        <figcaption>{large.window}, {when(large.when)}{shots.length > 1 ? ` · ${shots.findIndex((s) => s.url === large.url) + 1} of ${shots.length}` : ''} <button type="button" class="btn small ghost" onclick={() => (large = null)}>Back to them all</button></figcaption>
      </figure>
    {:else if shots === null}
      <p class="soft">Looking…</p>
    {:else if !shots.length}
      <p class="soft">None yet. The Screenshot key takes one while you play (F12 unless you've set another on the Controls page), and a table can take one when a game makes the high scores (its settings).</p>
    {:else}
      {#each moments as [stamp, list] (stamp)}
        <section>
          <h3 class="section-title">{when(stamp)}</h3>
          <div class="grid">
            {#each list as s (s.url)}
              <button type="button" class="shot" onclick={() => (large = s)} oncontextmenu={(e) => { e.preventDefault(); asking = s; }}>
                <img src={s.url} alt="" loading="lazy" />
                <span>{s.window}</span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
      <p class="soft">Right-click one to delete it.</p>
    {/if}
    {#if asking}
      <div class="ask">
        <span>Delete the {asking.window} picture of {when(asking.when)}? It goes to the Recycle Bin.</span>
        <button type="button" class="btn small" onclick={() => remove(asking)}>Delete</button>
        <button type="button" class="btn small ghost" onclick={() => (asking = null)}>Cancel</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.7); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(1100px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  h3 { margin: 0 0 8px; }
  .soft { margin: 0; color: var(--muted); font-size: 14px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 10px; }
  .shot { display: flex; flex-direction: column; gap: 6px; padding: 0; border: 0; background: none; text-align: left; color: var(--fg-soft); font-size: 12px; }
  .shot img { width: 100%; aspect-ratio: 16 / 10; object-fit: contain; border-radius: 10px; background: var(--picture-bg); outline: 1px solid var(--control-line); }
  .shot:hover img { outline-color: var(--accent); }
  .large { margin: 0; display: flex; flex-direction: column; gap: 10px; }
  .view { position: relative; display: flex; justify-content: center; border-radius: 10px; background: var(--picture-bg); }
  .large img { max-width: 100%; max-height: 70vh; object-fit: contain; border-radius: 10px; }
  .arrow { position: absolute; top: 50%; width: 44px; height: 44px; margin-top: -22px; border: 1px solid var(--control-line); border-radius: 50%; background: rgba(13, 16, 22, 0.8); color: var(--fg); display: flex; align-items: center; justify-content: center; }
  .arrow:hover { border-color: var(--accent); }
  .arrow.left { left: 12px; }
  .arrow.right { right: 12px; }
  .large figcaption { display: flex; align-items: center; gap: 12px; color: var(--fg-soft); font-size: 13px; }
  .ask { position: sticky; bottom: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--control); border: 1px solid var(--control-line); }
  .ask span { flex: 1 1 300px; font-size: 13px; }
</style>
