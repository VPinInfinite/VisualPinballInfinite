<script>
  // Settings › PinUP packs: playing the packs, which comes first where a pack and a backglass file
  // both show, the room around the packs' screens — VPX 5436's settings, which the player reads.
  // The packs themselves, each with its tables, layout and status, are under Tools › PinUP packs;
  // PinUP Popper's media under Tools › Media.
  import IniPage from './IniPage.svelte';

  // a pack first where VPX's renderer priorities say so, the higher first, the backglass file first
  // when they're equal; neither set: the pack first (the player's own)
  const first = (section, where) => ({
    label: `On the ${where}, first comes`,
    kind: 'choice',
    keys: [`${section}/Priority.PUP`, `${section}/Priority.B2S`],
    options: [{ label: 'The pack' }, { label: 'The backglass file' }],
    read: (get) => {
      const [pack, file] = [get(`${section}/Priority.PUP`), get(`${section}/Priority.B2S`)];
      if (pack === null && file === null) return 0;
      return Number(pack ?? 0) > Number(file ?? 0) ? 0 : 1;
    },
    write: (i, set) => {
      set(`${section}/Priority.PUP`, i === 0 ? null : '0');
      set(`${section}/Priority.B2S`, i === 0 ? null : '1');
    },
    what: 'Where a table has both, on that window.',
    vpx: `[${section}] Priority.PUP, Priority.B2S`,
  });
  const SIDES = ['Left', 'Right', 'Top', 'Bottom'];
  const pads = (prefix, where) => ({ label: `On the ${where}`, kind: 'custom', name: prefix, keys: SIDES.map((s) => `Plugin.PUP/${prefix}Pad${s}`), vpx: `[Plugin.PUP] ${prefix}PadLeft … ${prefix}PadBottom` });

  const parts = [
    {
      name: 'Playing them',
      rows: [
        { label: 'PinUP packs', kind: 'toggle', key: 'Plugin.PUP/Enable', def: true, offLabel: 'Off: tables play without their packs', vpx: '[Plugin.PUP] Enable' },
        first('Backglass', 'backglass'),
        first('ScoreView', 'score display'),
      ],
    },
    {
      name: 'Room around the packs’ screens',
      note: 'A pack’s screens drawn inside their window, this many pixels in from each side.',
      rows: [pads('BG', 'backglass'), pads('SV', 'score display'), pads('Topper', 'topper')],
    },
  ];

</script>

{#snippet padding(prefix, get, set)}
  {#each SIDES as side (side)}
    <label class="pad">{side}<input class="select" type="number" min="0" max="4096" step="1" value={get(`Plugin.PUP/${prefix}Pad${side}`) ?? 0} onchange={(e) => set(`Plugin.PUP/${prefix}Pad${side}`, e.currentTarget.value === '0' ? null : e.currentTarget.value)} /></label>
  {/each}
{/snippet}
{#snippet BG(get, set)}{@render padding('BG', get, set)}{/snippet}
{#snippet SV(get, set)}{@render padding('SV', get, set)}{/snippet}
{#snippet Topper(get, set)}{@render padding('Topper', get, set)}{/snippet}


<IniPage title="PinUP packs" lead="Packs of videos, sounds and pictures a table plays on the backglass, the score display and the topper: how they play. Your packs, each with its layout, are under Tools › PinUP packs, and PinUP Popper’s media under Tools › Media. Their volume is on the Sound page; where they’re kept, under Library › Folders." {parts} custom={{ BG, SV, Topper }} />

<style>
  .pad { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; color: var(--fg-soft); }
  .pad input { width: 76px; height: 30px; }
</style>
