<script>
  // Settings › Picture: how tables look, for every table —
  // a table's own settings always win — the score displays, and the display timing. VPX's own settings,
  // under VPX 5436's names; each row's VPX name beside what it does.
  import IniPage from './IniPage.svelte';

  // VPX keeps a colour as Windows does, red in the lowest byte
  const hex = (v) => '#' + [v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff].map((c) => c.toString(16).padStart(2, '0')).join('');
  // a PIN2DMD key being typed: written to the settings when used, never read back into the page
  let keyEntering = $state(false);
  let keyDraft = $state('');
  const colour = (h) => String(parseInt(h.slice(5, 7), 16) * 65536 + parseInt(h.slice(3, 5), 16) * 256 + parseInt(h.slice(1, 3), 16));
  // VPX's eight segment display looks with build 5436's defaults (`segments::PROFILES`)
  const SEGMENTS = [
    ['Neon plasma', 0x001e96ff],
    ['VFD blueish', 0x00ffef3f],
    ['VFD greenish', 0x00ecff48],
    ['Red LED', 0x001523ff],
    ['Green LED', 0x0023ff15],
    ['Yellow LED', 0x0023ffff],
    ['Generic plasma', 0x00ffffff],
    ['Generic LED', 0x00ffffff],
  ];
  const profileKeys = SEGMENTS.flatMap((_, i) => ['Color', 'Unlit', 'Brightness', 'DiffuseGlow'].map((k) => `Alpha/Profile${i + 1}${k}`));

  // the frames' choice: VPX's own synchronization (`[Player]` `SyncMode`), Frame Pacing its default
  const frames = {
    label: 'Frames',
    kind: 'choice',
    select: true,
    options: [
      { label: 'VPX’s own: each frame just before the screen shows it (Frame Pacing)', set: { 'Player/SyncMode': [null, '3'] } },
      { label: 'As fast as possible: the quickest, may tear', set: { 'Player/SyncMode': '0' } },
      { label: 'The screen’s own pace: no tearing, more delay (Vertical Sync)', set: { 'Player/SyncMode': '1' } },
      { label: 'The screen’s pace, but late frames shown at once (Adaptive Sync)', set: { 'Player/SyncMode': '2' } },
    ],
    what: 'The quickest response comes from drawing as fast as possible; tearing — a picture split across a moving line — shows on some screens, and waiting for the screen stops it at the cost of a little delay.',
    vpx: '[Player] SyncMode',
  };

  const parts = [
    {
      name: 'The look, for every table',
      note: 'Each table’s own unless you choose here; a table’s own settings, on its page, win over these.',
      rows: [
        {
          label: 'Tone mapper',
          kind: 'choice',
          options: [
            { label: 'Each table’s own', set: { 'TableOverride/ToneMapper': null } },
            { label: 'Reinhard', set: { 'TableOverride/ToneMapper': '0' } },
            { label: 'AgX', set: { 'TableOverride/ToneMapper': '1' } },
            { label: 'Filmic', set: { 'TableOverride/ToneMapper': '2' } },
            { label: 'Neutral', set: { 'TableOverride/ToneMapper': '3' } },
          ],
          what: 'How colors too bright for the screen are brought down: Reinhard softest, Filmic and AgX keep the color of bright lights.',
          vpx: '[TableOverride] ToneMapper',
        },
        { label: 'Exposure', kind: 'slider', key: 'TableOverride/Exposure', min: 0, max: 2, step: 0.05, def: 1, unset: 'Each table’s own', fmt: (v) => Number(v).toFixed(2), what: 'Brighter or darker overall.', vpx: '[TableOverride] Exposure' },
        {
          label: 'Light level',
          kind: 'choice',
          options: [
            { label: 'Each table’s own', set: { 'Player/OverrideTableEmissionScale': [null, '0'], 'Player/DynamicDayNight': [null, '0'] } },
            { label: 'Fixed', set: { 'Player/OverrideTableEmissionScale': '1', 'Player/DynamicDayNight': [null, '0'] } },
            { label: 'From the sun where you are', set: { 'Player/OverrideTableEmissionScale': '1', 'Player/DynamicDayNight': '1' } },
          ],
          what: 'Fixed: from daylight to night, below. From the sun: as it is now where you are (General › Where you are).',
          vpx: '[Player] OverrideTableEmissionScale, DynamicDayNight',
        },
        {
          label: 'Fixed light level',
          kind: 'slider',
          key: 'Player/EmissionScale',
          min: 0,
          max: 1,
          step: 0.05,
          def: 1,
          fmt: (v) => `${Math.round(Number(v) * 100)}%: ${Number(v) >= 0.7 ? 'daylight' : Number(v) >= 0.35 ? 'dusk' : 'night'}`,
          when: (get) => get('Player/OverrideTableEmissionScale') === '1' && get('Player/DynamicDayNight') !== '1',
          vpx: '[Player] EmissionScale',
        },
      ],
    },
    {
      name: 'On an HDR screen',
      note: 'For a screen with Windows’ HDR on.',
      rows: [
        {
          label: 'Bright light',
          kind: 'choice',
          options: [
            { label: 'VPX’s HDR curve', set: { 'Player/HDRDisableToneMapper': [null, '1'] } },
            { label: 'Each table’s own tone mapper', set: { 'Player/HDRDisableToneMapper': '0' } },
          ],
          what: 'VPX’s curve hands the screen the light past white, as VPX 5436 does.',
          vpx: '[Player] HDRDisableToneMapper',
        },
        { label: 'HDR brightness', kind: 'slider', key: 'Player/HDRGlobalExposure', min: 0, max: 5, step: 0.1, def: 1, fmt: (v) => Number(v).toFixed(1), vpx: '[Player] HDRGlobalExposure' },
      ],
    },
    {
      name: 'Quality',
      rows: [
        {
          label: 'Smoothing (multisampling)',
          kind: 'choice',
          options: [
            { label: 'Off', set: { 'Player/MSAASamples': [null, '0'] } },
            { label: '4 samples', set: { 'Player/MSAASamples': '1' } },
            { label: '8 samples', set: { 'Player/MSAASamples': ['3', '2'] } },
          ],
          what: 'Off is VPX’s own and the quickest. On switches off the ambient occlusion, the balls’ motion blur and the smoothing filters’ sparing of the background, which need what multisampling takes.',
          vpx: '[Player] MSAASamples',
        },
        { label: 'Supersampling', kind: 'slider', key: 'Player/AAFactor', min: 0.5, max: 2, step: 0.25, def: 1, fmt: (v) => (Number(v) === 1 ? '1× (off)' : `${Number(v)}×`), what: 'Drawn larger or smaller than the window, then fitted. At anything but 1×, VPX 10.8.1 and newer draw a backglass, score display or topper kept inside the table’s window at the wrong place and size.', vpx: '[Player] AAFactor' },
        {
          label: 'Smoothing filter',
          kind: 'choice',
          select: true,
          options: [
            { label: 'Off', set: { 'Player/FXAA': [null, '0'] } },
            { label: 'FXAA, fast', set: { 'Player/FXAA': '1' } },
            { label: 'FXAA', set: { 'Player/FXAA': '2' } },
            { label: 'FXAA, quality', set: { 'Player/FXAA': '3' } },
            { label: 'NFAA', set: { 'Player/FXAA': '4' } },
            { label: 'DLAA', set: { 'Player/FXAA': '5' } },
            { label: 'SMAA', set: { 'Player/FXAA': '6' } },
            { label: 'FAAA', set: { 'Player/FXAA': '7' } },
          ],
          vpx: '[Player] FXAA',
        },
        {
          label: 'Sharpening',
          kind: 'choice',
          options: [
            { label: 'Off', set: { 'Player/Sharpen': [null, '0'] } },
            { label: 'CAS', set: { 'Player/Sharpen': '1' } },
            { label: 'Bilateral CAS', set: { 'Player/Sharpen': '2' } },
          ],
          vpx: '[Player] Sharpen',
        },
        {
          label: 'Ambient occlusion',
          kind: 'choice',
          options: [
            { label: 'Off', set: { 'Player/DisableAO': '1' } },
            { label: 'Made once', set: { 'Player/DisableAO': [null, '0'], 'Player/DynamicAO': '0' } },
            { label: 'Every frame', set: { 'Player/DisableAO': [null, '0'], 'Player/DynamicAO': [null, '1'] } },
          ],
          what: 'The soft shadows where parts meet. Every frame is VPX’s own; made once, with the still parts, is quicker.',
          vpx: '[Player] DisableAO, DynamicAO',
        },
        {
          label: 'Reflections',
          kind: 'choice',
          select: true,
          options: [
            { label: 'None', set: { 'Player/PFReflection': '0' } },
            { label: 'The balls only', set: { 'Player/PFReflection': '1' } },
            { label: 'The still parts only', set: { 'Player/PFReflection': '2' } },
            { label: 'The still parts and the balls', set: { 'Player/PFReflection': '3' } },
            { label: 'Still and moving parts, unsynced', set: { 'Player/PFReflection': '4' } },
            { label: 'Everything', set: { 'Player/PFReflection': [null, '5'] } },
          ],
          what: 'What the playfield and the tables’ mirrors reflect.',
          vpx: '[Player] PFReflection',
        },
        { label: 'Bloom', kind: 'toggle', key: 'Player/ForceBloomOff', invert: true, def: true, what: 'The glow around bright lights, as strong as each table says.', vpx: '[Player] ForceBloomOff' },
        { label: 'The balls’ motion blur', kind: 'toggle', key: 'Player/ForceMotionBlurOff', invert: true, def: true, vpx: '[Player] ForceMotionBlurOff' },
        { label: 'Sharper pictures at an angle', kind: 'toggle', key: 'Player/ForceAnisotropicFiltering', def: true, what: 'Anisotropic filtering.', vpx: '[Player] ForceAnisotropicFiltering' },
        { label: 'Detail level', kind: 'slider', key: 'Player/AlphaRampAccuracy', min: 1, max: 10, step: 1, def: 10, what: 'Of rubbers, ramps and balls, drawn and in the physics, as VPX takes it.', vpx: '[Player] AlphaRampAccuracy' },
      ],
    },
    {
      name: 'The balls',
      rows: [
        { label: 'Unstretch the ball', kind: 'toggle', key: 'Player/BallAntiStretch', def: false, what: 'Squeezed back against the view’s stretching.', vpx: '[Player] BallAntiStretch' },
        { label: 'Ball trail', kind: 'toggle', key: 'Player/BallTrail', def: false, what: 'VPX’s older trail behind a moving ball. Its newer motion blur is under Quality.', vpx: '[Player] BallTrail' },
        { label: 'Its strength', kind: 'slider', key: 'Player/BallTrailStrength', min: 0, max: 5, step: 0.1, def: 0.5, fmt: (v) => Number(v).toFixed(1), when: (get) => get('Player/BallTrail') === '1', vpx: '[Player] BallTrailStrength' },
        { label: 'Lighting on balls', kind: 'toggle', key: 'Player/DisableLightingForBalls', invert: true, def: true, what: 'Off: balls without lighting or reflections.', vpx: '[Player] DisableLightingForBalls' },
        { label: 'Your own ball pictures', kind: 'toggle', key: 'Player/OverwriteBallImage', def: false, what: 'For every ball, in place of the tables’ own.', vpx: '[Player] OverwriteBallImage' },
        { label: 'Ball picture', kind: 'text', key: 'Player/BallImage', placeholder: 'A picture file’s full path', when: (get) => get('Player/OverwriteBallImage') === '1', vpx: '[Player] BallImage' },
        { label: 'Its decal', kind: 'text', key: 'Player/DecalImage', placeholder: 'A picture file’s full path, or none', when: (get) => get('Player/OverwriteBallImage') === '1', vpx: '[Player] DecalImage' },
      ],
    },
    {
      name: 'The score displays',
      note: 'The DMD, color files, older machines’ segment displays and a real DMD panel. With the classic VPinMAME and a display device, a game’s own color on the real DMD is in its table’s settings.',
      rows: [
        {
          label: 'The DMD made smoother',
          kind: 'choice',
          select: true,
          options: [
            { label: 'Off, as in VPX', set: { 'Plugin.UpscaleDMD/UpscaleMode': [null, '0'] } },
            { label: 'ScaleFX AA 1×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '1' } },
            { label: 'ScaleFX 3×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '2' } },
            { label: 'MMPX 2×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '3' } },
            { label: 'xBRZ 2×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '4' } },
            { label: 'xBRZ 3×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '5' } },
            { label: 'xBRZ 4×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '6' } },
            { label: 'xBRZ 5×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '7' } },
            { label: 'xBRZ 6×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '8' } },
            { label: 'Super-xBR 2×', set: { 'Plugin.UpscaleDMD/UpscaleMode': '9' } },
          ],
          what: 'VPX 5436’s DMD upscalers.',
          vpx: '[Plugin.UpscaleDMD] UpscaleMode',
        },
        { label: 'Score view layouts', kind: 'toggle', key: 'Plugin.ScoreView/Enable', def: true, what: 'VPX’s layouts for the score window.', vpx: '[Plugin.ScoreView] Enable' },
        { label: 'Serum colorings', kind: 'toggle', key: 'Plugin.Serum/Enable', def: true, vpx: '[Plugin.Serum] Enable' },
        { label: 'PIN2DMD colorings', kind: 'toggle', key: 'Plugin.VNI/Enable', def: true, vpx: '[Plugin.VNI] Enable' },
        { label: 'Your PIN2DMD key', kind: 'custom', name: 'key', keys: ['Plugin.VNI/PacKey'], what: 'Some colorings made for PIN2DMD panels come locked (a .pac file) and only play with the key of the PIN2DMD panel they were bought for. If you own a PIN2DMD panel, its key opens them here too; without one, those few colorings stay off and everything else plays. Kept in the settings file, never shown again.', vpx: '[Plugin.VNI] PacKey' },
        { label: 'Segment displays', kind: 'custom', name: 'profiles', keys: profileKeys, what: 'Each of VPX’s eight looks for older machines’ digits: its lit and unlit colors, brightness and the glow behind; a table chooses the look its displays use.', vpx: '[Alpha] Profile1…8 Color, Unlit, Brightness, DiffuseGlow' },
        { label: 'A real DMD panel, built in', kind: 'toggle', key: 'Plugin.DMDUtil/Enable', def: false, onLabel: 'On: shown on the panel', offLabel: 'Off', what: 'For a cabinet with a real LED DMD panel plugged in by USB (or on the network): ZeDMD, PIN2DMD or Pixelcade. On, the score display shows on the panel as well as on screen, driven by libdmdutil, the same part VPX uses; it finds the panel by itself. Nothing to install. Leave it off without a panel.', vpx: '[Plugin.DMDUtil] Enable' },
      ],
    },
    {
      name: 'Display timing',
      rows: [
        frames,
        {
          label: 'At most',
          kind: 'choice',
          select: true,
          options: [
            { label: 'VPX’s own: the screen’s refresh rate', set: { 'Player/MaxFramerate': [null, '-1'] } },
            { label: '60 a second', set: { 'Player/MaxFramerate': '60' } },
            { label: '120 a second', set: { 'Player/MaxFramerate': '120' } },
            { label: '144 a second', set: { 'Player/MaxFramerate': '144' } },
            { label: '165 a second', set: { 'Player/MaxFramerate': '165' } },
            { label: '240 a second', set: { 'Player/MaxFramerate': '240' } },
            { label: 'No limit: the graphics card works flat out', set: { 'Player/MaxFramerate': '0' } },
          ],
          vpx: '[Player] MaxFramerate',
        },
        {
          label: 'Frames queued',
          kind: 'choice',
          options: [
            { label: '1', set: { 'Player/MaxPrerenderedFrames': [null, '1'] } },
            { label: '2', set: { 'Player/MaxPrerenderedFrames': '2' } },
            { label: '3', set: { 'Player/MaxPrerenderedFrames': '3' } },
          ],
          what: 'How many frames the graphics card may hold before showing them: 1 is the quickest.',
          vpx: '[Player] MaxPrerenderedFrames',
        },
      ],
    },
  ];
</script>

{#snippet key(get, set)}
  <span class="keyrow">
    <span class="keyset">{get('Plugin.VNI/PacKey') ? 'A key is saved' : 'No key'}</span>
    {#if keyEntering}
      <input class="select keyin" type="password" autocomplete="off" spellcheck="false" placeholder="Your panel’s key" bind:value={keyDraft} aria-label="Your PIN2DMD panel’s key" />
      <button type="button" class="btn small" disabled={!keyDraft.trim()} onclick={() => { set('Plugin.VNI/PacKey', keyDraft.trim()); keyDraft = ''; keyEntering = false; }}>Use it</button>
      <button type="button" class="btn small ghost" onclick={() => { keyDraft = ''; keyEntering = false; }}>Cancel</button>
    {:else}
      <button type="button" class="btn small" onclick={() => (keyEntering = true)}>{get('Plugin.VNI/PacKey') ? 'Change it…' : 'Enter it…'}</button>
      {#if get('Plugin.VNI/PacKey')}<button type="button" class="btn small ghost" onclick={() => set('Plugin.VNI/PacKey', null)}>Remove it</button>{/if}
    {/if}
  </span>
{/snippet}

{#snippet profiles(get, set)}
  <div class="profiles">
    {#each SEGMENTS as [name, lit], i (name)}
      {@const k = (x) => `Alpha/Profile${i + 1}${x}`}
      {@const litV = get(k('Color')) !== null ? Number(get(k('Color'))) : lit}
      {@const unlitV = get(k('Unlit')) !== null ? Number(get(k('Unlit'))) : 0x00404040}
      <div class="profile">
        <b>{name}</b>
        <span class="digits" style="color: {hex(litV)}; text-shadow: 0 0 10px {hex(litV)}">88:88</span>
        <label>Lit <input type="color" value={hex(litV)} oninput={(e) => set(k('Color'), colour(e.currentTarget.value))} /></label>
        <label>Unlit <input type="color" value={hex(unlitV)} oninput={(e) => set(k('Unlit'), colour(e.currentTarget.value))} /></label>
        <label>Brightness <input class="select n" type="number" min="0.1" max="100" step="0.5" value={get(k('Brightness')) ?? 5} onchange={(e) => set(k('Brightness'), e.currentTarget.value)} /></label>
        <label>Glow <input class="select n" type="number" min="0" max="10" step="0.1" value={get(k('DiffuseGlow')) ?? 1} onchange={(e) => set(k('DiffuseGlow'), e.currentTarget.value)} /></label>
      </div>
    {/each}
  </div>
{/snippet}

<IniPage title="Picture" tag="vpx" lead="How tables look, for every table, their score displays, and how frames reach the screen: VPX’s own settings." {parts} custom={{ key, profiles }} />

<style>
  .keyset { font-size: 13px; color: var(--fg-soft); }
  .profiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 10px; width: 100%; }
  .profile { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); font-size: 13px; color: var(--fg-soft); }
  .profile b { color: var(--fg); font-size: 13px; }
  .digits { font: 700 26px ui-monospace, Consolas, monospace; letter-spacing: 0.08em; }
  .profile label { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .profile input[type='color'] { width: 44px; height: 26px; padding: 0; border: 1px solid var(--control-line); border-radius: 6px; background: none; }
  .profile .n { width: 80px; height: 28px; }
  .keyrow { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .keyin { width: 260px; }
</style>
