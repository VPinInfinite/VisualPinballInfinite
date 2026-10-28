<script>
  // Arrange: a stand-in for one of the player's windows, opened by the Screens page where that
  // window goes, to be dragged by its middle and sized by its edges and corners on the real
  // screens; a double-click makes it fill its screen. Done and Cancel on each stand-in too (Enter
  // and Esc), as the playfield's covers the app on its screen; the page reads where each was left.
  // One keeping its shape (`keep=1`) is held to it by the app as Windows sizes it.
  import { getCurrentWindow, currentMonitor, PhysicalPosition, PhysicalSize } from '@tauri-apps/api/window';
  import { emitTo } from '@tauri-apps/api/event';
  import { KINDS, params, shapeName } from './windows.js';

  const kind = KINDS[params('arrange').get('id')] || KINDS.playfield;
  const keep = params('arrange').get('keep') === '1';
  const win = getCurrentWindow();
  let size = $state('');

  async function measure() {
    const s = await win.innerSize();
    size = `${s.width} × ${s.height} · ${shapeName(s.width / s.height)}`;
  }
  measure();
  win.onResized(measure);

  async function fill() {
    const m = await currentMonitor();
    if (!m) return;
    await win.setPosition(new PhysicalPosition(m.position.x, m.position.y));
    await win.setSize(new PhysicalSize(m.size.width, m.size.height));
  }

  function grab(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    if (e.detail === 2) fill();
    else win.startDragging();
  }

  const edge = (dir) => (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    win.startResizeDragging(dir);
  };

  const finish = (keep) => emitTo('main', 'arrange-finish', { keep });

  function key(e) {
    if (e.key === 'Enter') finish(true);
    else if (e.key === 'Escape') finish(false);
  }

  const edges = [
    ['n', 'North'], ['s', 'South'], ['e', 'East'], ['w', 'West'],
    ['ne', 'NorthEast'], ['nw', 'NorthWest'], ['se', 'SouthEast'], ['sw', 'SouthWest'],
  ];
</script>

<svelte:window onkeydown={key} />

<div class="stand" style="--c: {kind.colour}" role="presentation" onpointerdown={grab}>
  <div class="bar" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
    <button type="button" class="btn small" onclick={() => finish(false)}>Cancel</button>
    <button type="button" class="btn small done" onclick={() => finish(true)}>Done</button>
  </div>
  <b>{kind.label}</b>
  <span class="size">{size}</span>
  <span class="how">Drag to move · drag an edge or corner to size it{keep ? ', its shape kept' : ''} · double-click to fill the screen · Enter when done</span>
  {#each edges as [cls, dir] (cls)}
    <div class="edge {cls}" role="presentation" onpointerdown={edge(dir)}></div>
  {/each}
</div>

<style>
  .stand {
    position: fixed; inset: 0; cursor: move; user-select: none; overflow: hidden;
    background: color-mix(in srgb, var(--c) 18%, #0d1016); border: 4px solid var(--c);
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; text-align: center; padding: 12px;
  }
  .bar { position: absolute; top: 14px; left: 50%; transform: translateX(-50%); display: flex; gap: 8px; cursor: default; }
  .done { background: var(--accent-bg); border-color: var(--accent); }
  b { font-size: clamp(16px, 6vmin, 56px); font-family: var(--display); }
  .size { font-size: clamp(13px, 3vmin, 26px); color: #fff; }
  .how { font-size: clamp(11px, 1.8vmin, 15px); color: var(--fg-soft); }
  .edge { position: absolute; }
  .n, .s { left: 14px; right: 14px; height: 10px; cursor: ns-resize; }
  .n { top: 0; }
  .s { bottom: 0; }
  .e, .w { top: 14px; bottom: 14px; width: 10px; cursor: ew-resize; }
  .e { right: 0; }
  .w { left: 0; }
  .ne, .nw, .se, .sw { width: 18px; height: 18px; }
  .ne { top: 0; right: 0; cursor: nesw-resize; }
  .sw { bottom: 0; left: 0; cursor: nesw-resize; }
  .nw { top: 0; left: 0; cursor: nwse-resize; }
  .se { bottom: 0; right: 0; cursor: nwse-resize; background: linear-gradient(135deg, transparent 50%, var(--c) 50%); }
</style>
