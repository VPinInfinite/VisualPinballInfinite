<script>
  // Where the playfield's sounds come from in each of VPX 5436's speaker layouts (`[Player]`
  // `Sound3D`): a
  // cabinet seen from above, its speakers named by the sound card's outputs they're plugged into,
  // and a ball rolling about the playfield, each speaker glowing as loud as it plays the ball — a
  // sketch of VPX's mixing (its pan left to right and its fade from the far end to the lockbar),
  // not a measure of it. The backbox's pair, where the layout leaves the front outputs to the
  // backglass, shows its music and the machine's sound.
  import { onMount } from 'svelte';

  let { layout = 0 } = $props();

  // the speakers' places: on the backbox, at the playfield's far end, by the lockbar
  const PLACES = {
    box: [[48, 35], [172, 35]],
    far: [[66, 86], [154, 86]],
    near: [[66, 262], [154, 262]],
  };

  // each layout's speakers: which outputs go where, and whether they play the table's sounds
  const SETUPS = [
    [{ at: 'box', out: 'Front', table: true }],
    [{ at: 'near', out: 'Rear', table: true }, { at: 'box', out: 'Front', table: false }],
    [{ at: 'far', out: 'Front', table: true }, { at: 'near', out: 'Rear', table: true }],
    [{ at: 'far', out: 'Rear', table: true }, { at: 'near', out: 'Front', table: true }],
    [{ at: 'far', out: 'Side', table: true }, { at: 'near', out: 'Rear', table: true }, { at: 'box', out: 'Front', table: false }],
    [{ at: 'far', out: 'Side', table: true }, { at: 'near', out: 'Rear', table: true }, { at: 'box', out: 'Front', table: false }],
  ];

  let t = $state(0);
  const ball = $derived({ x: 110 + 34 * Math.sin(t * 1.1), y: 170 + 74 * Math.sin(t * 0.55 + 0.6) });

  /** How loud a speaker plays the ball: its side's share of the pan, and, where the layout fades
   *  from the far end to the lockbar, its end's share of that. */
  function level(s, side) {
    if (!s.table) return 0;
    const pan = (ball.x - 110) / 34;
    const lr = side ? (1 + pan) / 2 : (1 - pan) / 2;
    const setup = SETUPS[layout] || SETUPS[0];
    if (!setup.some((o) => o.table && o.at !== s.at)) return lr;
    const fade = (ball.y - 170) / 74;
    return lr * (s.at === 'near' ? (1 + fade) / 2 : (1 - fade) / 2);
  }

  onMount(() => {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
      t = 2.2;
      return;
    }
    let frame = 0;
    const start = performance.now();
    const step = (now) => {
      t = (now - start) / 1000;
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  });
</script>

<svg viewBox="0 0 220 312" role="img" aria-label="A cabinet seen from above: which speakers play the table’s sounds">
  <!-- the backbox, the cabinet's body and its playfield, the lockbar, the player -->
  <rect class="box" x="30" y="14" width="160" height="42" rx="6" />
  <text class="part" x="110" y="12">backbox</text>
  <rect class="body" x="44" y="62" width="132" height="222" rx="8" />
  <rect class="field" x="56" y="72" width="108" height="196" rx="4" />
  <rect class="lockbar" x="44" y="280" width="132" height="8" rx="3" />
  <text class="part" x="110" y="302">lockbar · you</text>

  <circle class="ball" cx={ball.x} cy={ball.y} r="6" />

  {#each SETUPS[layout] || SETUPS[0] as s (s.at)}
    {#each PLACES[s.at] as [x, y], side (side)}
      {@const v = level(s, side)}
      <!-- named outward of the cabinet's body, inward on the backbox -->
      {@const out = s.at === 'box' ? !side : !!side}
      <g class="spk" class:table={s.table}>
        <circle class="glow" cx={x} cy={y} r={8 + 10 * v} style="opacity: {s.table ? 0.12 + 0.5 * v : 0}" />
        <rect x={x - 8} y={y - 8} width="16" height="16" rx="4" />
        <circle class="cone" cx={x} cy={y} r="4" />
        <text class="out" x={x + (out ? 13 : -13)} y={y + 4} text-anchor={out ? 'start' : 'end'}>{s.out} {side ? 'R' : 'L'}</text>
      </g>
    {/each}
  {/each}
</svg>
<p class="key"><span class="sw table"></span>The table’s sounds{#if (SETUPS[layout] || []).some((s) => !s.table)}<span class="sw bg"></span>Left for the backglass{/if}</p>

<style>
  svg { width: 100%; max-width: 240px; display: block; margin: 0 auto; overflow: visible; }
  .box, .body { fill: #121823; stroke: #2c3546; stroke-width: 1.5; }
  .field { fill: #0b1018; stroke: #222b3a; }
  .lockbar { fill: #3a4a64; }
  .part { fill: var(--faint); font-size: 10px; text-anchor: middle; }
  .ball { fill: #e6ecf5; stroke: #9aa6b8; }
  .spk rect { fill: #1b2230; stroke: #4a5568; stroke-width: 1.5; }
  .spk .cone { fill: #4a5568; }
  .spk.table rect { stroke: var(--accent); }
  .spk.table .cone { fill: var(--accent); }
  .glow { fill: var(--accent); }
  .spk:not(.table) rect { stroke: #b48cff; }
  .spk:not(.table) .cone { fill: #b48cff; }
  .out { fill: var(--fg-soft); font-size: 10px; }
  .key { margin: 8px 0 0; display: flex; align-items: center; justify-content: center; gap: 6px; font-size: 12px; color: var(--muted); flex-wrap: wrap; }
  .sw { width: 10px; height: 10px; border-radius: 3px; background: var(--accent); }
  .sw.bg { background: #b48cff; margin-left: 10px; }
</style>
