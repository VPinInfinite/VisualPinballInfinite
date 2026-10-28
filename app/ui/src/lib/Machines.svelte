<script>
  // Settings › Machines: VPinMAME's settings for every machine a ROM
  // table drives, kept in `Machine settings.ini` beside the settings file, its `[default]` section
  // — a game's own, on its table's settings, win over these, and a table's script may set
  // them too, as VPinMAME lets it. The player reads each today.
  import IniPage from './IniPage.svelte';

  const M = (name) => `Machine.default/${name}`;
  const parts = [
    {
      name: 'For every machine',
      note: 'A game’s own, on its table’s settings (Its machine), win over these.',
      rows: [
        { label: 'Its sound', kind: 'toggle', key: M('sound'), def: true, what: 'The machine’s own music, callouts and effects, from its ROM.', vpx: 'sound' },
        { label: 'Its volume', kind: 'slider', key: M('volume'), min: -32, max: 32, step: 1, def: 0, fmt: (v) => `${Number(v) > 0 ? '+' : ''}${Number(v)} dB`, what: 'Only the ROM’s sound, louder or softer: one loud game turned down alone. AltSound packs keep their own mix.', vpx: 'volume' },
        {
          label: 'Its sound comes from',
          kind: 'choice',
          options: [
            { label: 'Its own sound', set: { [M('sound_mode')]: [null, '0'] } },
            { label: 'Its AltSound pack, if it has one', set: { [M('sound_mode')]: ['1', '2', '3'] } },
          ],
          what: 'An AltSound pack plays recorded music and callouts in the machine’s place (the Sound page turns packs off for every game).',
          vpx: 'sound_mode',
        },
        { label: 'Older machines’ recorded sounds', kind: 'toggle', key: M('samples'), def: false, what: 'Off unless a game’s own settings turn them on.', vpx: 'samples' },
        { label: 'Skip its start-up tests', kind: 'toggle', key: M('cheat'), def: false, what: 'PinMAME’s cheat mode, which skips some games’ tests at start.', vpx: 'cheat' },
        { label: 'A real DMD panel for it', kind: 'toggle', key: M('showpindmd'), def: false, what: 'Off: the panel stays dark for that game.', vpx: 'showpindmd' },
        { label: 'Its floating score window', kind: 'toggle', key: M('showwindmd'), def: true, what: 'On one screen, the score display floating over the table.', vpx: 'showwindmd' },
      ],
    },
    {
      name: 'Each machine’s memory',
      note: 'A real pinball machine remembers things when it’s switched off: its high scores, its credits, the operator’s settings (balls per game, free play, difficulty) and its audits (games played). PinMAME keeps the same for each ROM, in a small file per game.',
      rows: [
        { label: 'Where it comes from', kind: 'info', text: 'The first time a table plays here, its machine starts from VPX’s copy of that memory, if VPX has one, so your high scores and settings come along. From then on it keeps its own, in this app’s data folder, and VPX’s copy is never changed.' },
        { label: 'A machine new to both', kind: 'info', text: 'With no memory yet, it’s switched on once by itself for 15 seconds before play, as a new machine would be, so it starts with its factory settings rather than an error.' },
      ],
    },
  ];
</script>

<IniPage title="Machines" lead="PinMAME’s settings for every machine a ROM table drives. All of it works in the player today." {parts} kept="Kept in Machine settings.ini, beside the settings file, as VPinMAME keeps them for every game." />
