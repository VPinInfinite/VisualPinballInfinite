<script>
  // The score display placed over a table's backglass picture by hand, for a VPX that keeps its
  // place in the backglass file: the picture with the place as a box — dragged to move it, by its
  // corner to size it (the DMD's own shape kept while Keep its shape is ticked), the arrow keys
  // nudging it — or its numbers typed, in the picture's own pixels from its top left; found in the
  // picture as the app finds it by itself; saved into the file as VPX's own menu saves it, which the
  // app's own placing then leaves as it is. Closed only by its buttons, so a drag ending outside it
  // loses nothing.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';
  import { KINDS, shapeName } from './windows.js';

  let { t, onclose } = $props();

  let view = $state(null);
  let problem = $state('');
  let box = $state(null);
  let saved = $state(null);
  let keepShape = $state(true);
  let busy = $state(false);
  let finding = $state(false);
  let frame = $state(null);
  let drag = null;

  // the smallest it's let be, in the picture's pixels
  const MIN = 8;
  const same = (a, b) => !!a && !!b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
  const changed = $derived(!!box && !same(box, saved));
  const colour = KINDS.scoreView.colour;

  load();

  async function load() {
    try {
      const v = await invoke('dmd_place_open', { id: t.id });
      saved = v.place ? { x: v.place.x, y: v.place.y, w: v.place.w, h: v.place.h } : null;
      // a place of another shape than the DMD's (stretched by hand before) keeps its own
      if (saved) keepShape = Math.abs(saved.w / saved.h / v.shape - 1) < 0.03;
      view = v;
      box = saved ? { ...saved } : (await found()) || fallback();
    } catch (e) {
      problem = String(e);
    }
  }

  // where it fits the picture, as the app finds it by itself
  async function found() {
    try {
      const p = await invoke('dmd_place_find', { id: t.id });
      return p ? fit(p) : null;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
      return null;
    }
  }

  // none found: across the middle, in the speaker panel where the picture has one, else low in it
  function fallback() {
    const w = view.width * 0.45;
    const h = w / view.shape;
    const middle = view.grill > 0 ? view.height - view.grill / 2 : view.height * 0.8;
    return fit({ x: (view.width - w) / 2, y: middle - h / 2, w, h });
  }

  async function findAgain() {
    finding = true;
    const p = await found();
    finding = false;
    if (p) box = p;
    else notice('No place for it was found in the picture: drag the box there.', { kind: 'problem' });
  }

  // kept inside the picture, whole pixels, at least a few across
  function fit(b) {
    const w = Math.max(MIN, Math.min(Math.round(b.w), view.width));
    const h = Math.max(MIN, Math.min(Math.round(b.h), view.height));
    return { x: Math.max(0, Math.min(Math.round(b.x), view.width - w)), y: Math.max(0, Math.min(Math.round(b.y), view.height - h)), w, h };
  }

  function down(e, mode) {
    if (e.button !== 0 || !frame) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.focus?.();
    const r = frame.getBoundingClientRect();
    drag = { mode, x: e.clientX, y: e.clientY, start: { ...box }, scale: view.width / r.width };
  }

  function move(e) {
    if (!drag) return;
    const dx = (e.clientX - drag.x) * drag.scale;
    const dy = (e.clientY - drag.y) * drag.scale;
    const s = drag.start;
    if (drag.mode === 'move') {
      box = fit({ ...s, x: s.x + dx, y: s.y + dy });
      return;
    }
    // by its corner: as wide as dragged within the picture, as high as its shape makes it or as dragged
    let w = Math.max(MIN, Math.min(s.w + dx, view.width - s.x));
    let h = keepShape ? w / view.shape : Math.max(MIN, Math.min(s.h + dy, view.height - s.y));
    if (keepShape && s.y + h > view.height) {
      h = view.height - s.y;
      w = h * view.shape;
    }
    box = fit({ x: s.x, y: s.y, w, h });
  }

  function up() {
    drag = null;
  }

  // the arrow keys move it a pixel, ten with Shift
  function nudge(e) {
    const step = e.shiftKey ? 10 : 1;
    const by = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
    if (!by) return;
    e.preventDefault();
    box = fit({ ...box, x: box.x + by[0], y: box.y + by[1] });
  }

  // a number typed; its width or height, while its shape is kept, changing the other with it
  function typed(key, value) {
    const v = Number(value);
    if (!Number.isFinite(v)) return;
    const next = { ...box, [key]: v };
    if (keepShape && key === 'w') next.h = v / view.shape;
    if (keepShape && key === 'h') next.w = v * view.shape;
    box = fit(next);
  }

  // ticked: its height brought to its width at the DMD's shape
  function shapeTicked() {
    if (keepShape) box = fit({ ...box, h: box.w / view.shape });
  }

  async function save() {
    busy = true;
    try {
      await invoke('dmd_place_save', { id: t.id, x: box.x, y: box.y, w: box.w, h: box.h });
      notice(`${t.short}’s score display is placed: VPX shows it there from its next game.`, { kind: 'done' });
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  const pct = (v, of) => `${(v / of) * 100}%`;
  const FIELDS = [
    ['x', 'Left'],
    ['y', 'Top'],
    ['w', 'Width'],
    ['h', 'Height'],
  ];
</script>

<svelte:window onpointermove={move} onpointerup={up} onpointercancel={up} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dp-title" data-loading={(view && box) || problem ? undefined : ''}>
    <header>
      <h2 id="dp-title">{t.short}’s score display on its backglass</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>

    {#if problem}
      <p class="problem">{problem}</p>
      <footer><span class="gap"></span><button type="button" class="btn ghost" onclick={onclose}>Close</button></footer>
    {:else if !view || !box}
      <p class="soft">Looking…</p>
    {:else}
      <p class="soft">Drag the box onto the window in the picture where the score display belongs, and its corner to size it; the arrow keys move it a pixel at a time. VPX shows it there from the table’s next game, and the app leaves a place you set as it is.</p>
      <div class="stage">
        <div class="frame" bind:this={frame} style="width: min(100%, calc(56vh * {view.width / view.height}))">
          <img src={view.picture} alt="{t.short}’s backglass" draggable="false" />
          {#if view.grill > 0}
            <div class="grill" style="top: {pct(view.height - view.grill, view.height)}"><span>Speaker panel</span></div>
          {/if}
          <div
            class="box"
            role="button"
            tabindex="0"
            aria-label="The score display: drag it to move it, or move it with the arrow keys"
            style="left: {pct(box.x, view.width)}; top: {pct(box.y, view.height)}; width: {pct(box.w, view.width)}; height: {pct(box.h, view.height)}; --c: {colour}"
            onpointerdown={(e) => down(e, 'move')}
            onkeydown={nudge}
          >
            <span class="label">Score display<small>{box.w} × {box.h} · {shapeName(box.w / box.h)}</small></span>
            <span class="corner" role="presentation" title="Drag to size it" onpointerdown={(e) => down(e, 'size')}></span>
          </div>
        </div>
      </div>
      <div class="numbers">
        {#each FIELDS as [key, label] (key)}
          <label class="num">
            <span>{label}</span>
            <input type="number" min="0" max={key === 'x' || key === 'w' ? view.width : view.height} value={box[key]} onchange={(e) => typed(key, e.currentTarget.value)} />
          </label>
        {/each}
        <label class="check"><input type="checkbox" bind:checked={keepShape} onchange={shapeTicked} />Keep its shape ({shapeName(view.shape)}, the DMD’s)</label>
      </div>
      <p class="soft small">In the picture’s own pixels, from its top left: {view.width} × {view.height}{view.grill > 0 ? `, its speaker panel the bottom ${view.grill}` : ''}. Saved into {view.file}, as VPX’s own menu saves it; the file as it came is kept by the app the first time.</p>
      <footer>
        <button type="button" class="btn ghost" disabled={finding} onclick={findAgain}><Icon name="search" size={15} />{finding ? 'Looking…' : 'Find it in the picture'}</button>
        <button type="button" class="btn ghost" disabled={!saved || !changed} onclick={() => (box = { ...saved })}>Undo changes</button>
        <span class="gap"></span>
        <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
        <button type="button" class="btn go" disabled={busy || !changed} onclick={save}><Icon name="check" size={15} />{busy ? 'Saving…' : 'Save'}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(1100px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .small { font-size: 12px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .stage { display: flex; justify-content: center; }
  .frame { position: relative; user-select: none; }
  .frame img { display: block; width: 100%; height: auto; border-radius: 8px; background: var(--picture-bg); pointer-events: none; }
  /* the score display's place, in the Screens page's colour for it */
  .box { position: absolute; box-sizing: border-box; border: 2px solid var(--c); border-radius: 4px; background: color-mix(in srgb, var(--c) 16%, transparent); box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.7); cursor: move; touch-action: none; display: flex; align-items: center; justify-content: center; }
  .box:focus-visible { outline: 2px solid #fff; outline-offset: 2px; }
  .label { pointer-events: none; display: flex; flex-direction: column; align-items: center; gap: 1px; color: #fff; font-size: 13px; font-weight: 600; line-height: 1.2; text-align: center; text-shadow: 0 1px 3px #000, 0 0 6px #000; }
  .label small { font-weight: 400; font-size: 12px; opacity: 0.9; }
  .corner { position: absolute; right: -8px; bottom: -8px; width: 16px; height: 16px; border-radius: 4px; background: var(--c); box-shadow: 0 0 0 2px #131822; cursor: nwse-resize; touch-action: none; }
  .grill { position: absolute; left: 0; right: 0; border-top: 1px dashed rgba(255, 255, 255, 0.5); pointer-events: none; }
  .grill span { position: absolute; right: 8px; top: 3px; font-size: 11px; color: rgba(255, 255, 255, 0.8); text-shadow: 0 1px 2px #000; }
  .numbers { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 16px; }
  .num { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; color: var(--fg-soft); }
  .num input { width: 84px; height: 32px; box-sizing: border-box; padding: 0 9px; border-radius: 8px; border: 1px solid var(--control-line); background: var(--bg); color: var(--fg); font: inherit; font-size: 14px; }
  .check { display: inline-flex; align-items: center; gap: 7px; font-size: 13px; }
  footer { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding-top: 4px; }
  .gap { flex: 1; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
