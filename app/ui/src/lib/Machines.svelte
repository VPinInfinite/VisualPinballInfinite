<script>
  // Settings › Machines: PinMAME's settings for every machine a ROM table drives, where each PinMAME
  // reads them — VPX's own PinMAME add-on's in VPX's settings (`[Plugin.PinMAME]`), the classic
  // VPinMAME's for every game where it keeps them (its `default`, here `Machine.default`); a game's
  // own, on its table's settings, win over these, and a table's script may set them too, as
  // VPinMAME lets it. Settings › Library says which PinMAME plays the tables.
  import IniPage from './IniPage.svelte';

  const M = (name) => `Machine.default/${name}`;
  const parts = [
    {
      name: 'VPX’s own PinMAME',
      note: 'When VPX’s own add-on plays the machines (Settings › Library › Its add-ons): these hold for every machine. Its AltSound packs and colorings are VPX’s add-ons too, on the Sound and Picture pages.',
      rows: [
        { label: 'Skip their start-up tests', kind: 'toggle', key: 'Plugin.PinMAME/Cheat', def: false, what: 'PinMAME’s cheat mode, which skips some games’ tests at start.', vpx: '[Plugin.PinMAME] Cheat' },
      ],
    },
    {
      name: 'The classic VPinMAME, for every machine',
      note: 'When VPinMAME plays the machines: its own settings for every game, kept where VPinMAME keeps them. A game’s own, on its table’s settings (Its machine), win over these.',
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
          what: 'An AltSound pack plays recorded music and callouts in the machine’s place.',
          vpx: 'sound_mode',
        },
        { label: 'Older machines’ recorded sounds', kind: 'toggle', key: M('samples'), def: false, what: 'Off unless a game’s own settings turn them on.', vpx: 'samples' },
        { label: 'Skip its start-up tests', kind: 'toggle', key: M('cheat'), def: false, what: 'PinMAME’s cheat mode, which skips some games’ tests at start.', vpx: 'cheat' },
        { label: 'A real DMD panel for it', kind: 'toggle', key: M('showpindmd'), def: false, what: 'Off: the panel stays dark for that game.', vpx: 'showpindmd' },
        { label: 'Its own score window', kind: 'toggle', key: M('showwindmd'), def: true, what: 'VPinMAME’s own window showing the score display.', vpx: 'showwindmd' },
      ],
    },
    {
      name: 'Each machine’s memory',
      note: 'A real pinball machine remembers things when it’s switched off: its high scores, its credits, the operator’s settings (balls per game, free play, difficulty) and its audits (games played). PinMAME keeps the same for each ROM, in a small file per game.',
      rows: [
        { label: 'Where it’s kept', kind: 'info', text: 'Where VPX’s PinMAME keeps it: beside the table’s own ROMs, in its pinmame\\nvram folder, else in PinMAME’s folder.' },
        { label: 'Carried over', kind: 'info', text: 'The first time a table plays in VPX, the memory this app’s own player kept for it comes along where VPX has none, so your high scores and settings carry on.' },
        { label: 'A machine new to both', kind: 'info', text: 'With no memory yet, it’s switched on once by itself, unseen, before its first game, as a new machine would be, so it starts with its factory settings rather than an error.' },
      ],
    },
  ];
</script>

<IniPage title="Machines" tag="vpx" lead="PinMAME’s settings for every machine a ROM table drives, where each PinMAME reads them." {parts} kept="VPX’s own PinMAME’s in VPX’s settings; the classic VPinMAME’s where it keeps them, in your part of Windows’ registry." />
