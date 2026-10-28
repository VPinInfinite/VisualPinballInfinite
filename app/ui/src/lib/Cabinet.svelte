<script>
  // Settings › Cabinet: the cabinet's playfield screen and where the
  // player stands at it — VPX 5436's window mode, which the player reads today — and the boards,
  // sensors and toys, still being built here. The toys work already, from the DOF Config Tool's
  // files where VPX keeps them or in our settings folder: where they're found is said, and a set
  // can be brought in — the Config Tool's download, a file of a set, or DOF's own folder where
  // DOF is installed — for a PC without VPX set up.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import IniPage from './IniPage.svelte';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  const parts = [
    {
      name: 'The playfield screen and you',
      note: 'For the cabinet view, VPX’s window mode: the table drawn at its real size behind the glass.',
      rows: [
        { label: 'The screen’s width', kind: 'number', key: 'Player/ScreenWidth', min: 10, max: 300, step: 0.01, placeholder: 'The screen’s own', unit: 'cm, its picture’s width', what: 'Unset: the screen’s own, as it reports it.', vpx: '[Player] ScreenWidth' },
        { label: 'Its tilt', kind: 'number', key: 'Player/ScreenInclination', min: -90, max: 90, step: 0.5, placeholder: '0', unit: '° up from flat', vpx: '[Player] ScreenInclination' },
        { label: 'Where you stand: across', kind: 'number', key: 'Player/ScreenPlayerX', min: -200, max: 200, step: 0.5, placeholder: '0', unit: 'cm from the screen’s middle', vpx: '[Player] ScreenPlayerX' },
        { label: 'Back', kind: 'number', key: 'Player/ScreenPlayerY', min: -200, max: 200, step: 0.5, placeholder: '-10', unit: 'cm from its bottom edge', vpx: '[Player] ScreenPlayerY' },
        { label: 'Your eyes, up', kind: 'number', key: 'Player/ScreenPlayerZ', min: 0, max: 300, step: 0.5, placeholder: '70', unit: 'cm above it', vpx: '[Player] ScreenPlayerZ' },
      ],
    },
    { name: 'Boards and sensors', note: 'Still being built.', rows: [] },
    {
      name: 'Toys (DOF)',
      note: 'Still being built here, but the toys work already: your DOF Config Tool files are read where VPX keeps them (the directoutputconfig folder in its settings folder), so a cabinet set up for VPX keeps working, or from ours. No VPX set up? Import your configuration.',
      rows: [{ label: 'Its configuration', kind: 'custom', name: 'dof' }],
    },
  ];

  // where the toys' configuration is found, and DOF's own folder
  let dof = $state(null);
  const look = () => invoke('dof_found').then((w) => (dof = w)).catch(() => (dof = { found: null, dofOwn: null }));
  onMount(look);

  let busy = $state(false);
  async function bringIn(from = null) {
    busy = true;
    try {
      const n = await invoke('import_dof', { from });
      if (n !== null) {
        notice(`The DOF configuration is in (${n} ${n === 1 ? 'file' : 'files'}): the toys use it from the next game.`, { kind: 'done' });
        await look();
      }
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }
</script>

{#snippet dofRow()}
  <span class="dof">
    <span class="where">
      {#if !dof}Looking…
      {:else if dof.found}{dof.found.whose === 'ours' ? 'Ours' : 'VPX’s'}, {dof.found.files} {dof.found.files === 1 ? 'file' : 'files'}: {dof.found.folder}
      {:else}None found yet: the toys stay off until there is one.{/if}
    </span>
    <span class="buttons">
      <button type="button" class="btn small" disabled={busy} onclick={() => bringIn()}><Icon name="download" size={14} />{busy ? 'Bringing it in…' : 'Import a DOF configuration…'}</button>
      {#if dof?.dofOwn && !dof.found}
        <button type="button" class="btn small ghost" disabled={busy} onclick={() => bringIn(dof.dofOwn)}>Bring in DOF’s own, from {dof.dofOwn}</button>
      {/if}
    </span>
    <span class="hint">The DOF Config Tool’s download (its zip), or any file of a configuration you’ve unzipped. It’s copied into ours; a file it replaces goes to the Recycle Bin.</span>
  </span>
{/snippet}

<IniPage title="Cabinet" lead="The real cabinet: its playfield screen and where you stand at it, its boards and sensors, and its toys. A real DMD panel is on the Picture page." {parts} custom={{ dof: dofRow }} />

<style>
  .dof { display: flex; flex-direction: column; gap: 8px; }
  .where { color: var(--fg-soft); font-size: 13px; word-break: break-all; }
  .buttons { display: flex; flex-wrap: wrap; gap: 8px; }
  .hint { color: var(--faint); font-size: 12px; }
</style>
