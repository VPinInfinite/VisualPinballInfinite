<script>
  // One kind of a table's media — VPS's, a front end's (PinUP Popper's, PinballX's, PinballY's), ours: each with where it's from, the one used marked with a blue check and any other
  // made the one used; one opened large, left and right through the rest from there, by the arrows
  // or the arrow keys; one deleted, or one of VPS's hidden, on a
  // right-click. A kind with one opens on it, large; the one a front end shows said, and the one used put in PinUP Popper's
  // folder on a word (Settings › PinUP packs does it each time, when on).
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, notice, toast } from './store.svelte.js';

  let { t, kind, onclose } = $props();

  const items = $derived(t.media.filter((m) => m.kind === kind));
  const label = $derived(items[0]?.label ?? 'Media');
  const alone = $derived(items.length === 1);
  // a kind with one: that one, large
  let large = $state(null);
  $effect.pre(() => {
    if (!large && alone) large = items[0];
  });
  let asking = $state(null);
  let putting = $state(false);

  const FROM = { VPS: 'From VPS', Popper: 'From PinUP Popper', PinballX: 'From PinballX', PinballY: 'From PinballY', ours: 'Made by our player' };
  // a front end's own, by its name in words
  const FRONT = { Popper: 'PinUP Popper', PinballX: 'PinballX', PinballY: 'PinballY' };
  // where one is from: a copy the app put in a front end's folder said as what it copies, and in
  // full, where it is
  const from = (m) => FROM[m.copy || m.from];
  const fromFull = (m) => (m.copy ? `${FROM[m.copy]}, copied into ${FRONT[m.from]}’s folder` : FROM[m.from]);

  // a kind with none left closes; one opened large that's gone shows the one used in its place
  // (put in PinUP Popper's folder, ours gives way to Popper's copy), or them all
  $effect(() => {
    if (!items.length) onclose();
    else if (large && !items.some((m) => m.url === large.url)) large = alone ? items[0] : items.find((m) => m.chosen && FRONT[m.from]) || null;
    // the one open large as the library has it now (chosen, turned)
    else if (large) {
      const now = items.find((m) => m.url === large.url);
      if (now !== large) large = now;
    }
  });

  // the one used put where PinUP Popper shows it, on the user's word
  async function putInPopper() {
    putting = true;
    try {
      const moved = await invoke('popper_media_put', { id: t.id, kind });
      if (moved) toast('Put in PinUP Popper’s folder', [`${t.short}’s ${label.toLowerCase()} is the one PinUP Popper shows now. Popper’s own one before it is kept beside it, numbered; a copy put in before goes to the Recycle Bin.`]);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    putting = false;
  }

  // one lying on its side turned the other way, where the app's match was wrong
  async function turnOther(m) {
    try {
      await invoke('turn_media', { id: t.id, pick: m.pick, turn: m.turn === 90 ? 270 : 90 });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function use(m) {
    try {
      await invoke('choose_media', { id: t.id, kind, pick: m.pick });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function remove(m) {
    asking = null;
    try {
      const done = await invoke('delete_media', { id: t.id, url: m.url });
      notice(done === 'hidden' ? `VPS’s ${m.label.toLowerCase()} is hidden from ${t.short}’s page.` : `The ${m.label.toLowerCase()} went to the Recycle Bin.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // the one before or after the one open large, round from the last to the first
  function step(by) {
    if (!large || items.length < 2) return;
    const at = items.findIndex((m) => m.url === large.url);
    large = items[(at + by + items.length) % items.length];
  }

  function key(e) {
    if (e.key === 'Escape') {
      if (asking) asking = null;
      else if (large && !alone) large = null;
      else onclose();
    } else if (large && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      step(e.key === 'ArrowLeft' ? -1 : 1);
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

<!-- a front end's part: the one it shows, or the one used put in PinUP Popper's folder on a word -->
{#snippet popper(m)}
  {#if m.front}
    <span class="popper" title="In {FRONT[m.from]}’s folder for it, under the name {FRONT[m.from]} gives the game">{FRONT[m.from]} shows this one</span>
  {:else if m.chosen && app.popper}
    <button type="button" class="btn small ghost" disabled={putting} onclick={putInPopper} title="Copied into PinUP Popper’s folder for it, named as Popper names the game; Popper’s own one it shows now is kept beside it, numbered, and a copy put in before goes to the Recycle Bin">{putting ? 'Putting it in…' : 'Put it in PinUP Popper’s folder'}</button>
  {/if}
{/snippet}

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ms-title">
    <header>
      <h2 id="ms-title">{t.short}’s {label.toLowerCase()}{alone ? '' : ` (${items.length})`}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    <!-- what's picked here is for browsing alone: never what the table plays with -->
    <p class="soft play-note">For browsing only: pick which one shows for this table on its tile in the library, on this page and in cabinet mode. It doesn’t change the table or what it shows when you play.{kind.startsWith('backglass') ? ' When you play, its backglass comes from its backglass file, under What it holds.' : kind.startsWith('dmd') ? ' When you play, its score display comes from the table itself.' : ''}</p>
    {#if app.popper}<p class="soft">PinUP Popper shows the one in its own folder: put the one shown here there when it’s another.</p>{/if}
    {#if large}
      <figure class="large">
        <!-- one kept on its side (a front end's playfield for a cabinet) shown upright -->
        <div class="view" class:turn-frame={large.turn}>
          {#if large.video}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video src={large.url} class:turned={large.turn === 90 || large.turn === 270} class:ccw={large.turn === 270} class:upside={large.turn === 180} autoplay muted loop playsinline controls={!large.turn}></video>
          {:else}
            <img src={large.url} alt="{large.label}, {from(large).toLowerCase()}" class:contain={large.contain} class:turned={large.turn === 90 || large.turn === 270} class:ccw={large.turn === 270} class:upside={large.turn === 180} />
          {/if}
          {#if !alone}
            <button type="button" class="arrow left" aria-label="The one before" onclick={() => step(-1)}><Icon name="chevL" size={22} /></button>
            <button type="button" class="arrow right" aria-label="The one after" onclick={() => step(1)}><Icon name="chevR" size={22} /></button>
          {/if}
        </div>
        <figcaption>
          <span>{fromFull(large)}{alone ? '' : ` · ${items.findIndex((m) => m.url === large.url) + 1} of ${items.length}`}</span>
          {#if large.chosen}
            <span class="inuse"><Icon name="check" size={14} width={2.8} />Shown while browsing</span>
          {:else}
            <button type="button" class="btn small" onclick={() => use(large)}><Icon name="check" size={14} />Show this one</button>
          {/if}
          {@render popper(large)}
          {#if large.turn}<button type="button" class="btn small ghost" onclick={() => turnOther(large)}>Turn it the other way</button>{/if}
          {#if !alone}<button type="button" class="btn small ghost" onclick={() => (large = null)}>Back to them all</button>{/if}
        </figcaption>
      </figure>
    {:else}
      <div class="grid">
        {#each items as m (m.url)}
          <div class="item" class:chosen={m.chosen}>
            <button type="button" class="pic" class:turn-frame={m.turn} aria-label="Open the {m.label.toLowerCase()} {fromFull(m).toLowerCase()}" onclick={() => (large = m)} oncontextmenu={(e) => { e.preventDefault(); asking = m; }}>
              {#if m.video}
                <!-- svelte-ignore a11y_media_has_caption -->
                <video src="{m.url}#t=0.5" class:turned={m.turn === 90 || m.turn === 270} class:ccw={m.turn === 270} class:upside={m.turn === 180} muted preload="metadata" playsinline tabindex="-1"></video>
                <span class="vid"><Icon name="play" size={18} fill /></span>
              {:else}
                <img src={m.url} alt="" loading="lazy" class:contain={m.contain} class:turned={m.turn === 90 || m.turn === 270} class:ccw={m.turn === 270} class:upside={m.turn === 180} />
              {/if}
              {#if m.chosen}<span class="badge" title="Shown while browsing"><Icon name="check" size={14} width={3} /></span>{/if}
            </button>
            <div class="under">
              <span title={fromFull(m)}>{from(m)}</span>
              {#if m.chosen}
                <span class="inuse">Shown</span>
              {:else}
                <button type="button" class="btn small ghost" onclick={() => use(m)}>Show this one</button>
              {/if}
            </div>
            {#if m.front || (m.chosen && app.popper)}<div class="under">{@render popper(m)}</div>{/if}
          </div>
        {/each}
      </div>
      <p class="soft">Right-click one to delete it, or to hide one of VPS’s.</p>
    {/if}
    {#if asking}
      <div class="ask">
        {#if asking.from === 'VPS'}
          <span>Hide VPS’s {asking.label.toLowerCase()} from this page? It’s VPS’s, so it can’t be deleted, only hidden.</span>
          <button type="button" class="btn small" onclick={() => remove(asking)}>Hide it</button>
        {:else}
          <span>Delete this {asking.label.toLowerCase()} ({fromFull(asking).toLowerCase()})? It goes to the Recycle Bin, where you can get it back.</span>
          <button type="button" class="btn small danger" onclick={() => remove(asking)}>Delete</button>
        {/if}
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
  .soft { margin: 0; color: var(--muted); font-size: 14px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 14px; }
  .item { display: flex; flex-direction: column; gap: 8px; }
  .pic { position: relative; display: block; padding: 0; border: 0; border-radius: 10px; overflow: hidden; aspect-ratio: 4 / 3; background: var(--picture-bg); outline: 1px solid var(--control-line); }
  .item.chosen .pic { outline: 2px solid var(--accent); outline-offset: 2px; }
  .pic:hover { outline-color: var(--accent); }
  /* each shown whole, to choose between */
  .pic img, .pic video { display: block; width: 100%; height: 100%; object-fit: contain; pointer-events: none; }
  .pic img.contain { padding: 8px; }
  .vid {
    position: absolute; left: 50%; top: 50%; width: 34px; height: 34px; margin: -17px 0 0 -17px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center; color: #fff; background: rgba(8, 10, 15, 0.7); padding-left: 2px;
  }
  /* the blue check on the one used */
  .badge { position: absolute; top: 8px; right: 8px; width: 24px; height: 24px; border-radius: 50%; background: var(--accent); color: #06121c; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5); }
  .under { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 30px; font-size: 13px; color: var(--fg-soft); }
  .inuse { display: inline-flex; align-items: center; gap: 6px; color: var(--accent); font-size: 13px; font-weight: 600; }
  .popper { font-size: 12px; color: var(--good); }
  .large { margin: 0; display: flex; flex-direction: column; gap: 10px; }
  .view { position: relative; display: flex; justify-content: center; border-radius: 10px; background: var(--picture-bg); }
  .large img, .large video { max-width: 100%; max-height: 66vh; object-fit: contain; border-radius: 10px; }
  /* one shown turned: its view a fixed height for it to fit */
  .view.turn-frame { height: 66vh; }
  .arrow { position: absolute; top: 50%; width: 44px; height: 44px; margin-top: -22px; border: 1px solid var(--control-line); border-radius: 50%; background: rgba(13, 16, 22, 0.8); color: var(--fg); display: flex; align-items: center; justify-content: center; }
  .arrow:hover { border-color: var(--accent); }
  .arrow.left { left: 12px; }
  .arrow.right { right: 12px; }
  .large img.contain { padding: 12px; }
  .large figcaption { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; color: var(--fg-soft); font-size: 13px; }
  .ask { position: sticky; bottom: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--control); border: 1px solid var(--control-line); }
  .ask span { flex: 1 1 300px; font-size: 13px; }
  .danger { background: #4a1d26; border-color: #7a2c3a; }
  .danger:hover:not(:disabled) { background: #5a2430; }
  .play-note { margin-top: -4px; color: var(--fg-soft); }
</style>
