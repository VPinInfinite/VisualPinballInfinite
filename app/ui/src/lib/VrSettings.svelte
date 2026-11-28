<script>
  // Settings › VR: VPX's tables played in a VR headset, as VPX itself plays them — its BGFX
  // program, through a VR program on the PC (its OpenXR runtime: Meta Horizon Link, SteamVR,
  // Virtual Desktop and others). The VR program VPX plays with, chosen here and handed to VPX for
  // its game alone (Windows' own choice left as it is), the headset checked through it, what VPX
  // said of its last game in VR; VPX's own VR settings, as VPX names them: whether it plays in VR,
  // the headset's refresh rate and resolution, the PC's screen meanwhile, what's added in the
  // headset, the lockbar's size and the seat's tilt; where each VR program sets its own.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import IniPage from './IniPage.svelte';

  // the VR programs found, the one chosen, VPX's last game in VR
  let vr = $state(null);
  onMount(async () => {
    vr = await invoke('vr_state').catch(() => ({ programs: [], chosen: null, last: null }));
  });
  // the headset's check: running, then what it found
  let checking = $state(false);
  let checked = $state(null);
  async function check() {
    checking = true;
    checked = null;
    try {
      checked = await invoke('vr_check');
    } catch (e) {
      checked = { ready: false, problem: String(e) };
    }
    checking = false;
  }

  // where each kind of VR program sets its own resolution and refresh rate
  const OWN = {
    meta: 'On the PC: Meta Horizon Link › Devices › your headset › Graphics preferences: its refresh rate (up to 120 Hz on a Quest 3) and render resolution (up to 5408 × 2736). Frame interpolation (ASW) is in its Debug Tool.',
    steamvr: 'In SteamVR: Settings › Video: its render resolution, for every app or VPX’s alone, and its own headsets’ refresh rate.',
    'virtual-desktop': 'In the headset: Virtual Desktop’s Streaming tab: its graphics quality, frame rate, codec and bitrate; frame interpolation (SSW) off for pinball.',
    pimax: 'In Pimax Play: its render quality and refresh rate.',
    varjo: 'In Varjo Base: its resolution and refresh rate.',
    other: 'In its own settings.',
  };

  // the headset's resolution, kept for each kind of VR program (`[VPI]` `VrResFactor.<kind>`) and
  // VPX's own (`[PlayerVR]` `ResFactor`) set to the one VPX plays with; none, the program's own
  const KINDS = ['meta', 'steamvr', 'virtual-desktop', 'pimax', 'varjo', 'other'];
  const FACTORS = [null, 1.2, 1.4, 1.6, 2];
  const resKey = (kind) => `VPI/VrResFactor.${kind}`;
  // a factor as VPX takes one: past 0.1 and up to 10, anything else the recommended size
  const factorValue = (v) => {
    const n = v == null || String(v).trim() === '' ? NaN : Number(v);
    return n > 0.1 && n <= 10 ? n : null;
  };
  // the VR program VPX plays with: the one chosen, where it's on this PC, else Windows' own
  const inUse = (get) => vr?.programs.find((p) => p.manifest === (get('VPI/VrProgram') || '')) || vr?.programs.find((p) => p.active) || null;
  // no program's kept yet: VPX's own stands for the one in use
  const fresh = (get) => KINDS.every((k) => factorValue(get(resKey(k))) === null);
  const factorOf = (get) => {
    const p = inUse(get);
    return factorValue(!p || fresh(get) ? get('PlayerVR/ResFactor') : get(resKey(p.kind)));
  };
  // another VR program chosen: the resolution VPX had kept for the one before, where none was kept
  // yet, and VPX's own set to the new one's
  function pick(get, set, manifest) {
    const before = inUse(get);
    if (before && fresh(get) && factorValue(get('PlayerVR/ResFactor')) !== null) set(resKey(before.kind), get('PlayerVR/ResFactor'));
    set('VPI/VrProgram', manifest);
    const now = inUse(get);
    if (now && !fresh(get)) set('PlayerVR/ResFactor', factorValue(get(resKey(now.kind))) === null ? null : get(resKey(now.kind)));
  }

  const parts = [
    {
      name: 'Your VR program and headset',
      for: 'vpx',
      note: 'VPX shows a table in the headset through a VR program on this PC, which talks to the headset. Windows keeps one for every VR app; the one chosen here is VPX’s alone, Windows’ choice left as it is.',
      rows: [
        { label: 'VPX plays VR with', kind: 'custom', name: 'program', keys: ['VPI/VrProgram', 'PlayerVR/ResFactor', ...KINDS.map(resKey)] },
        { label: 'The headset', kind: 'custom', name: 'headset' },
      ],
    },
    {
      name: 'Playing in VR',
      for: 'vpx',
      rows: [
        {
          label: 'VR',
          kind: 'choice',
          options: [
            { label: 'Off', set: { 'PlayerVR/AskToTurnOn': [null, '2'] } },
            { label: 'When the headset is on', set: { 'PlayerVR/AskToTurnOn': '1' } },
            { label: 'Always', set: { 'PlayerVR/AskToTurnOn': '0' } },
          ],
          what: 'When the headset is on: the app asks for it as each game starts, and the game plays in it when it answers — VPX’s own menu there — or on your screens with the app’s pause menu when it doesn’t. Always: every table in the headset, the app checking first that it answers. Future Pinball’s tables follow this too, in BAM’s VR. Play it in VR, on a table’s page, plays one game in VR whatever this says.',
          vpx: '[PlayerVR] AskToTurnOn',
        },
        {
          label: 'The headset’s refresh rate',
          kind: 'choice',
          options: [
            { label: 'The VR program’s', set: { 'PlayerVR/DisplayRefreshRate': [null, '0'] } },
            { label: '72 Hz', set: { 'PlayerVR/DisplayRefreshRate': '1' } },
            { label: '80 Hz', set: { 'PlayerVR/DisplayRefreshRate': '2' } },
            { label: '90 Hz', set: { 'PlayerVR/DisplayRefreshRate': '3' } },
            { label: '120 Hz', set: { 'PlayerVR/DisplayRefreshRate': '4' } },
          ],
          what: 'Asked of the headset as a game starts; 120 Hz is the most VPX asks for. A lower rate gives VPX more time to draw each picture, for a smoother game on a table that’s heavy to draw; the VR program’s own keeps the rate set there.',
          vpx: '[PlayerVR] DisplayRefreshRate',
        },
        {
          label: 'Resolution in the headset',
          kind: 'choice',
          keys: ['PlayerVR/ResFactor', ...KINDS.map(resKey)],
          options: FACTORS.map((f) => ({ label: f === null ? 'The VR program’s' : `× ${f}` })),
          read: (get) => FACTORS.indexOf(factorOf(get)),
          write: (i, set, get) => {
            const v = FACTORS[i] === null ? null : String(FACTORS[i]);
            const p = inUse(get);
            if (p) set(resKey(p.kind), v);
            set('PlayerVR/ResFactor', v);
          },
          what: (get) => {
            const p = inUse(get);
            const last = p && vr?.lastFor?.[p.kind];
            const size = (s) => s.replace('x', ' × ');
            return [
              p ? `Kept for each VR program, as each recommends its own size: this is ${p.name}’s.` : '',
              'VPX draws each eye at the size the VR program recommends, times this: sharper, and heavier to draw. Past the most the headset takes, VPX keeps the recommended size; the VR program’s own resolution sets the size it recommends.',
              last?.selected ? `Its last game: each eye ${size(last.selected)}${last.recommended ? `, recommended ${size(last.recommended)}` : ''}${last.maximum ? `, at most ${size(last.maximum)}` : ''}.` : '',
            ].filter(Boolean).join(' ');
          },
          vpx: '[PlayerVR] ResFactor',
        },
        {
          label: 'Your screens meanwhile',
          kind: 'choice',
          options: [
            { label: 'The playfield and displays', set: { 'PlayerVR/DesktopDisplay': [null, '1'] } },
            { label: 'The playfield', set: { 'PlayerVR/DesktopDisplay': '2' } },
            { label: 'What you see', set: { 'PlayerVR/DesktopDisplay': '5' } },
            { label: 'Nothing', set: { 'PlayerVR/DesktopDisplay': '0' } },
          ],
          what: 'What VPX shows on the PC’s screen while you play in the headset: What you see is the headset’s view, both eyes. Nothing draws less, if a game stutters.',
          vpx: '[PlayerVR] DesktopDisplay',
        },
        { label: 'The headset’s view fitted to the screen', kind: 'toggle', key: 'PlayerVR/ShrinkDesktopDisplay', def: false, what: 'The headset’s view on the PC’s screen shrunk to fit it, rather than cut.', vpx: '[PlayerVR] ShrinkDesktopDisplay', when: (get) => ['3', '4', '5'].includes(String(get('PlayerVR/DesktopDisplay') ?? '')) },
      ],
    },
    {
      name: 'In the headset',
      for: 'vpx',
      rows: [
        { label: 'A backglass for tables without their own', kind: 'toggle', key: 'PlayerVR/AddBackglass', def: false, what: 'A table made for screens shows no backglass in VR: its B2S or PinUP backglass stood above the playfield.', vpx: '[PlayerVR] AddBackglass' },
        { label: 'The backglass’s sound from where it stands', kind: 'toggle', key: 'PlayerVR/SpatialAudioBackglass', def: true, what: 'Music and callouts heard from the backglass, as you turn your head.', vpx: '[PlayerVR] SpatialAudioBackglass' },
        { label: 'The playfield’s sounds from where they are', kind: 'toggle', key: 'PlayerVR/SpatialAudioPlayfield', def: true, what: 'Each sound of the playfield heard from its place on it, as you turn your head.', vpx: '[PlayerVR] SpatialAudioPlayfield' },
        { label: 'Your room around the table', kind: 'toggle', key: 'PlayerVR/UsePassthroughColor', def: false, what: 'Mixed reality: VPX’s room drawn plain black for the VR program to show your own room there, as Virtual Desktop does with its passthrough set to black.', vpx: '[PlayerVR] UsePassthroughColor' },
      ],
    },
    {
      name: 'The cabinet you stand at',
      for: 'vpx',
      note: 'The virtual cabinet sized to a real lockbar under your hands, or a cabinet’s own. Where the table stands in your room, and its turn, are set in VPX’s own menu in the headset, and the app keeps them from game to game.',
      rows: [
        { label: 'The lockbar’s width', kind: 'toggle', key: 'PlayerVR/OverrideLockbarWidth', def: true, what: 'The cabinet scaled to the width below; off, the table’s own size.', vpx: '[PlayerVR] OverrideLockbarWidth' },
        { label: 'Width', kind: 'slider', key: 'Player/LockbarWidth', min: 10, max: 150, step: 0.5, def: 70, fmt: (v) => `${Number(v)} cm`, what: 'Measured on the cabinet, or your table at home.', vpx: '[Player] LockbarWidth', when: (get) => get('PlayerVR/OverrideLockbarWidth') !== '0' },
        { label: 'The lockbar’s height', kind: 'toggle', key: 'PlayerVR/OverrideLockbarHeight', def: true, what: 'The cabinet stood at the height below; off, its own height.', vpx: '[PlayerVR] OverrideLockbarHeight' },
        { label: 'Height', kind: 'slider', key: 'Player/LockbarHeight', min: 0, max: 250, step: 0.5, def: 85, fmt: (v) => `${Number(v)} cm`, what: 'From the floor to the top of the lockbar.', vpx: '[Player] LockbarHeight', when: (get) => get('PlayerVR/OverrideLockbarHeight') !== '0' },
        { label: 'Tilted toward you', kind: 'slider', key: 'PlayerVR/SeatedTilt', min: -90, max: 90, step: 1, def: 0, fmt: (v) => `${Number(v)}°`, what: 'For playing seated or leaning back: the view tilted, the physics as they are.', vpx: '[PlayerVR] SeatedTilt' },
      ],
    },
  ];

  const kindOf = (manifest) => vr?.programs.find((p) => p.manifest === manifest)?.kind || 'other';
</script>

{#snippet program(get, set)}
  {@const now = get('VPI/VrProgram') || ''}
  {@const windows = vr?.programs.find((p) => p.active)}
  <span class="seg wrap" role="radiogroup" aria-label="VPX plays VR with">
    <button type="button" role="radio" aria-checked={!now} onclick={() => pick(get, set, null)}>{windows ? `Windows’ own: ${windows.name}` : 'Windows’ own'}</button>
    {#each (vr?.programs || []).filter((p) => !p.active) as p (p.manifest)}
      <button type="button" role="radio" aria-checked={now === p.manifest} title={p.manifest} onclick={() => pick(get, set, p.manifest)}>{p.name}</button>
    {/each}
  </span>
  {#if vr && !vr.programs.length}
    <span class="kept">No VR program is on this PC yet: Meta Horizon Link, SteamVR or Virtual Desktop’s Streamer, for your headset.</span>
  {:else}
    <span class="kept">{OWN[kindOf(now || windows?.manifest)]}</span>
  {/if}
{/snippet}

{#snippet headset()}
  <button type="button" class="btn small" disabled={checking} onclick={check}><Icon name="eye" size={14} />{checking ? 'Asking…' : 'Check'}</button>
  {#if checked}
    <span class="kept" class:good={checked.ready}>
      {#if checked.ready}{checked.headset || 'A headset'} answers{checked.runtime ? `, through ${checked.runtime}${checked.version ? ` ${checked.version}` : ''}` : ''}: ready to play.
      {:else}{checked.problem || 'No headset answered.'}{/if}
    </span>
  {:else if vr?.last?.runtime}
    <span class="kept">Last game in VR: {vr.last.runtime}{vr.last.selected ? `, each eye ${vr.last.selected.replace('x', ' × ')}` : ''}{vr.last.recommended ? ` (recommended ${vr.last.recommended.replace('x', ' × ')}` : ''}{vr.last.maximum ? `, at most ${vr.last.maximum.replace('x', ' × ')})` : vr.last.recommended ? ')' : ''}.</span>
  {:else}
    <span class="kept">Put the headset on and connect it, then Check: the app asks the VR program as VPX does, before a game.</span>
  {/if}
{/snippet}

<IniPage title="VR" tag="vpx" lead="VPX’s tables played in a VR headset, as VPX plays them: in its BGFX program, VPX 10.8.1 or newer, through a VR program on this PC. Sharpening is left off for games in VR, as VPX draws a line with it in the headset." {parts} custom={{ program, headset }} />

<style>
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg.wrap { flex-wrap: wrap; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .kept { color: var(--faint); font-size: 13px; }
  .kept.good { color: var(--good); }
</style>
