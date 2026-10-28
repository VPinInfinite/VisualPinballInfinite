<script>
  // Settings › Physics sets: VPX's eight global physics sets — a name and the table's and the
  // flippers' values each, kept in the settings file under VPX's names (`[Player]`
  // `PhysicsSetName<n>`, `TablePhysics…<n>`, `FlipperPhysics…<n>`, numbered from 0), which the
  // player reads — one set shown at a time, with the tables that play it (a table's Physics set,
  // set in VPX's editor, plays that set in its own values' place). Unset values show VPX's defaults,
  // from the settings catalog.
  import IniPage from './IniPage.svelte';
  import { app, openTable } from './store.svelte.js';

  let set = $state(1);
  let defaults = $state({});
  import('./settings-catalog.json').then((m) => {
    const list = Array.isArray(m.default) ? m.default : (m.default.settings ?? []);
    defaults = Object.fromEntries(list.filter((s) => s.s === 'Player' && /Physics/.test(s.k)).map((s) => [s.k, s.def]));
  });

  // each value in plain words, the table's eight then the flippers' ten
  const TABLE = [
    ['TablePhysicsGravityConstant', 'Gravity', 'How hard the ball is pulled down the playfield.'],
    ['TablePhysicsContactFriction', 'Friction', 'Of the playfield and the table’s walls.'],
    ['TablePhysicsElasticity', 'Bounciness', 'Of the playfield and the table’s walls.'],
    ['TablePhysicsElasticityFalloff', 'Bounciness falloff', 'How much less it bounces the harder it hits.'],
    ['TablePhysicsScatterAngle', 'Scatter', 'A little chance in the ball’s path off the playfield, in degrees.'],
    ['TablePhysicsContactScatterAngle', 'Parts’ scatter', 'The same off parts that have none of their own.'],
    ['TablePhysicsMinSlope', 'Slope, easiest', 'The playfield’s slope in degrees at the easiest difficulty.'],
    ['TablePhysicsMaxSlope', 'Slope, hardest', 'At the hardest; the table’s difficulty sets where between.'],
  ];
  const FLIPPERS = [
    ['FlipperPhysicsMass', 'Mass', ''],
    ['FlipperPhysicsStrength', 'Strength', ''],
    ['FlipperPhysicsElasticity', 'Bounciness', ''],
    ['FlipperPhysicsScatter', 'Scatter', 'In degrees.'],
    ['FlipperPhysicsReturnStrength', 'Return', 'How fast a flipper falls back.'],
    ['FlipperPhysicsElasticityFalloff', 'Bounciness falloff', ''],
    ['FlipperPhysicsFriction', 'Friction', ''],
    ['FlipperPhysicsCoilRampUp', 'Coil ramp-up', 'How fast the coil comes to full strength.'],
    ['FlipperPhysicsEOSTorque', 'End-of-stroke hold', 'How firmly a flipper holds at the end of its stroke.'],
    ['FlipperPhysicsEOSTorqueAngle', 'End-of-stroke angle', 'How many degrees before the end the hold begins.'],
  ];
  const row = (n) => ([key, label, what]) => ({
    label,
    kind: 'number',
    key: `Player/${key}${n}`,
    step: 'any',
    placeholder: defaults[`${key}${n}`] !== undefined ? String(defaults[`${key}${n}`]) : 'VPX’s own',
    what: what || undefined,
    vpx: `[Player] ${key}${n}`,
  });

  const players = (s) => app.tables.filter((t) => t.physicsSet === s);
  const parts = $derived.by(() => {
    const n = set - 1;
    return [
      { name: 'Which set', rows: [{ label: 'Set', kind: 'custom', name: 'picker' }] },
      { name: `Set ${set}`, rows: [{ label: 'Its name', kind: 'text', key: `Player/PhysicsSetName${n}`, placeholder: `Set ${set}`, vpx: `[Player] PhysicsSetName${n}` }] },
      { name: 'The table', note: 'Unset: VPX’s own value, shown greyed where VPX declares it.', rows: TABLE.map(row(n)) },
      { name: 'Its flippers', rows: FLIPPERS.map(row(n)) },
    ];
  });
</script>

{#snippet picker()}
  <div class="sets">
    <div class="seg" role="radiogroup" aria-label="Which set">
      {#each [1, 2, 3, 4, 5, 6, 7, 8] as s (s)}
        <button type="button" role="radio" aria-checked={set === s} onclick={() => (set = s)}>{s}{#if players(s).length}<span class="n">{players(s).length}</span>{/if}</button>
      {/each}
    </div>
    <p class="note">
      {#if players(set).length}
        Played by {#each players(set) as t, i (t.id)}{i ? ', ' : ''}<button type="button" class="link" onclick={() => openTable(t.id)}>{t.name}</button>{/each}: {players(set).length === 1 ? 'it plays' : 'they play'} these values in {players(set).length === 1 ? 'its' : 'their'} own’s place.
      {:else}
        No table of yours plays set {set}: a table plays one only where its Physics set, set in VPX’s editor, names it.
      {/if}
    </p>
  </div>
{/snippet}

{#key set}
  <IniPage title="Physics sets" lead="Almost every table carries its own physics, set by its author: how the ball rolls and bounces, how strong the flippers are. A few say instead “use physics set 1” (to 8), chosen by the author in VPX’s editor, and then play the values kept here, for every table that names that set. Change them only if you mean to: a game played with them changed counts as Softcore for achievements. One table’s own physics are changed in its physics window, on its settings." {parts} custom={{ picker }} />
{/key}

<style>
  .sets { display: flex; flex-direction: column; gap: 8px; }
  .seg { display: inline-flex; flex-wrap: wrap; align-self: flex-start; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg .n { margin-left: 5px; padding: 0 6px; border-radius: 999px; background: var(--control-line); color: var(--fg); font-size: 11px; }
  .note { margin: 0; color: var(--faint); font-size: 13px; line-height: 1.5; max-width: 860px; }
  .link { padding: 0; border: 0; background: none; color: var(--fg-soft); font: inherit; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
</style>
