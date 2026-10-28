<script>
  // A table seen from above in the table editor: its parts' outlines over its real look — the
  // picture our player draws of it from straight above, as Make media does (`own_picture`, drawn
  // here on a click where it has none), laid under the outlines where they meet: the picture is the
  // playfield cut out with a few pixels round it — or the outlines alone. The parts given glow where
  // they're chosen or counted; in picking, those achievements can't count fade. A click names the
  // part under it (`onpart`).
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { COLOURS, colours, points, viewBox } from './playfield.js';
  import { app, makePictures, savePrefs, hiddenParts, hidePart } from './store.svelte.js';

  let { id, bounds, drawn, glows = [], picking = false, onpart, children } = $props();

  // the parts the user hid, by name: not drawn, so not chosen either
  const off = $derived(new Set(hiddenParts(id)));
  const shown = $derived(drawn.filter((p) => !off.has(p.name)));
  // a right-click on a part: hide it
  let menu = $state(null);
  function context(e) {
    const at = e.target.closest?.('[data-i]');
    if (!at) return;
    e.preventDefault();
    const p = drawn.find((x) => x.i === Number(at.dataset.i));
    if (p) menu = { p, x: e.clientX, y: e.clientY };
  }

  // the real look under the outlines, as the user last chose
  const look = $derived(app.prefs.editorLook ?? true);
  function setLook(on) {
    app.prefs.editorLook = on;
    savePrefs();
  }

  let picture = $state(null);
  let size = $state(null);
  async function loadPicture() {
    picture = await invoke('own_picture', { id, kind: 'playfield' }).catch(() => null);
  }
  $effect(() => {
    void id;
    size = null;
    loadPicture();
  });
  $effect(() => {
    const src = picture;
    if (!src) return;
    const img = new Image();
    img.onload = () => src === picture && (size = [img.naturalWidth, img.naturalHeight]);
    img.onerror = () => src === picture && (picture = null);
    img.src = src;
  });
  const drawing = $derived(app.makingIds.includes(id));
  async function draw() {
    await makePictures(id, { kinds: ['playfield'] }, { quiet: true });
    await loadPicture();
  }

  // where the picture sits: the playfield's corners `spare` pixels in from its edges, as the
  // player cuts it
  const placed = $derived.by(() => {
    if (!size) return null;
    const [l, t, r, b] = bounds;
    const spare = 4;
    const sx = (r - l) / Math.max(1, size[0] - 2 * spare);
    const sy = (b - t) / Math.max(1, size[1] - 2 * spare);
    return { x: l - spare * sx, y: t - spare * sy, w: size[0] * sx, h: size[1] * sy };
  });
  const showing = $derived(look && placed);

  function click(e) {
    const at = e.target.closest?.('[data-i]');
    if (at) onpart?.(Number(at.dataset.i));
  }
</script>

<div class="view">
  <div class="bar">
    <span class="seg" role="radiogroup" aria-label="How the table shows">
      <button type="button" role="radio" aria-checked={look} onclick={() => setLook(true)}>Its look</button>
      <button type="button" role="radio" aria-checked={!look} onclick={() => setLook(false)}>Outlines</button>
    </span>
    {#if look && !picture}
      <button type="button" class="btn small" disabled={drawing} onclick={draw}><Icon name="image" size={14} />{drawing ? 'Drawing it from above…' : 'Draw its look'}</button>
      <span class="hint">Our player draws it from straight above, a few seconds, kept after.</span>
    {/if}
    {@render children?.()}
  </div>
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <svg viewBox={viewBox(bounds)} role="img" aria-label="The playfield seen from above" class:picking class:withlook={showing} onclick={click} oncontextmenu={context}>
    {#if showing}
      <image href={picture} x={placed.x} y={placed.y} width={placed.w} height={placed.h} preserveAspectRatio="none" />
    {/if}
    {#each shown as p (p.i)}
      {@const [fill, stroke] = colours(p)}
      {@const title = p.counts.length ? `${p.name} (${p.kind}) — achievements can count it` : `${p.name} (${p.kind})`}
      {#if p.outline.kind === 'polygon'}
        <polygon class="part" class:countable={p.counts.length > 0} data-i={p.i} points={points(p.outline)} fill={showing ? 'rgba(0,0,0,0.001)' : fill} {stroke} stroke-width="2"><title>{title}</title></polygon>
      {:else if p.outline.kind === 'path'}
        <polyline class="part" class:countable={p.counts.length > 0} data-i={p.i} points={points(p.outline)} fill="none" stroke={p.kind === 'Ramp' ? COLOURS.Ramp[showing ? 1 : 0] : stroke} stroke-width={Math.max(3, p.outline.width)} stroke-linecap="round" stroke-linejoin="round"><title>{title}</title></polyline>
      {:else if p.outline.kind === 'circle'}
        <circle class="part" class:countable={p.counts.length > 0} data-i={p.i} cx={p.outline.at[0]} cy={p.outline.at[1]} r={Math.max(4, p.outline.radius)} fill={showing ? 'rgba(0,0,0,0.001)' : fill} {stroke} stroke-width="2"><title>{title}</title></circle>
      {/if}
    {/each}
    {#each glows.filter((g) => !off.has(g.p.name)) as g, n (n)}
      {@const p = g.p}
      {@const line = g.strong ? '#4cc2ff' : 'rgba(76,194,255,0.5)'}
      {#if p.outline.kind === 'polygon'}
        <polygon class="glow" points={points(p.outline)} fill="rgba(76,194,255,0.35)" stroke={line} stroke-width="6" />
      {:else if p.outline.kind === 'path'}
        <polyline class="glow" points={points(p.outline)} fill="none" stroke={line} stroke-width={Math.max(6, p.outline.width + 6)} stroke-linecap="round" stroke-linejoin="round" />
      {:else if p.outline.kind === 'circle'}
        <circle class="glow" cx={p.outline.at[0]} cy={p.outline.at[1]} r={Math.max(4, p.outline.radius)} fill="rgba(76,194,255,0.35)" stroke={line} stroke-width="6" />
      {/if}
    {/each}
  </svg>
</div>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => e.key === 'Escape' && (menu = null)} />
{#if menu}
  <div class="ctx" role="menu" style="left: {Math.min(menu.x, window.innerWidth - 200)}px; top: {Math.min(menu.y, window.innerHeight - 60)}px">
    <span class="ctxname">{menu.p.name}</span>
    <button type="button" role="menuitem" onclick={() => { hidePart(id, menu.p.name, true); menu = null; }}><Icon name="eyeOff" size={14} />Hide it</button>
  </div>
{/if}

<style>
  .view { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 10px; min-height: 30px; }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 28px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .hint { color: var(--faint); font-size: 12px; }
  svg { width: 100%; max-height: calc(100vh - 290px); display: block; }
  .part { cursor: pointer; }
  .part:hover { filter: brightness(1.6); }
  /* over its look, the outlines thin and faint, bright where pointed at */
  .withlook .part { stroke-opacity: 0.5; }
  /* a ramp's or a wire's whole width, faint over the picture until pointed at */
  .withlook polyline.part { stroke-opacity: 0.12; }
  .withlook .part:hover { stroke-opacity: 0.9; filter: none; }
  .picking .part:not(.countable) { opacity: 0.18; cursor: default; }
  .picking .part.countable { filter: brightness(1.35); stroke-opacity: 1; }
  .glow { pointer-events: none; }
  .ctx { position: fixed; z-index: 60; min-width: 160px; padding: 5px; border-radius: 10px; background: #151a24; border: 1px solid var(--control-line); box-shadow: 0 14px 30px rgba(0, 0, 0, 0.5); display: flex; flex-direction: column; }
  .ctxname { padding: 4px 10px 6px; color: var(--faint); font: 12px Consolas, ui-monospace, monospace; }
  .ctx button { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border: 0; border-radius: 7px; background: transparent; color: var(--fg); font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
  .ctx button:hover { background: var(--selected); }
</style>
