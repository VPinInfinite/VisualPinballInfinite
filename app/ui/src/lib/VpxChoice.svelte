<script>
  // VPX 10.8.1 or VPX 10.8.0: what changes with each, side by side, before choosing which plays the
  // tables (Settings › Library, first setup); opened to compare (`choosing` null) or to switch to
  // one (`choosing`, its line), the switch made only on its button (`vpx_play_with`).
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, notice } from './store.svelte.js';

  let { choosing = null, current = null, onclose, onchosen = null } = $props();

  let busy = $state(false);
  const switching = $derived(!!choosing && choosing !== current);

  async function choose() {
    busy = true;
    try {
      await invoke('vpx_play_with', { line: choosing });
      app.vpxOlder = choosing === '10.8.0';
      notice(`Tables play in VPX ${choosing} from now on.`, { kind: 'done' });
      onchosen?.(choosing);
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      busy = false;
    }
  }

  const NEWER = [
    ['star', 'VPX’s newest, which its team hasn’t released yet. Get VPX 10.8.1 brings the build this app is made for, one of VPX’s automatic builds.'],
    ['layers', 'Brings its own add-ons for ROMs, backglasses, PinUP packs, FlexDMD, music and toys — or uses the classic programs you have, part by part (Settings › Library › Its add-ons).'],
    ['trophy', 'Achievements as you play: each one shows the moment it’s earned, with its sound, and pausing shows where they stand. This needs VPX’s own ROM and backglass add-ons.'],
    ['image', 'Screenshots with a key, F9 unless you choose another, and, if you like, when a game makes a high score.'],
    ['sliders', 'The app’s Screens, Sound, Picture, Controls and Machines pages set it.'],
    ['eyeOff', 'Test it, the check of a new table, the script editor’s Check it and a machine’s first start run out of sight.'],
    ['folder', 'Plays tables kept in shared folders, or in a folder for each game.'],
  ];
  const OLDER = [
    ['star', 'VPX’s current full release.'],
    ['layers', 'Plays with the classic programs you have installed — VPinMAME, B2S Server, PinUP Player, FlexDMD, DOF — each with its own settings. The add-on switches don’t apply.'],
    ['trophy', 'Achievements are counted once the game is over, from its scores and what the machine and the table keep. Nothing shows while you play, and those that need more — a shot, a mode, a ball locked — can’t be earned.'],
    ['image', 'No screenshots from the app while you play.'],
    ['sliders', 'Keeps its own settings, apart from 10.8.1’s: the app’s Screens, Sound, Picture and Controls pages set 10.8.1’s. Set 10.8.0’s in its own Preferences, and the backglass and score display in B2S Server’s and VPinMAME’s. Screen presets and cabinet mode’s own screens are for 10.8.1.'],
    ['eyeOff', 'Test it and the other checks still run in VPX 10.8.1 when it’s there beside it; without it, they’re off.'],
    ['folder', 'Shared folders only: it can’t start ROM tables kept in a folder for each game.'],
  ];
  const BOTH = [
    'Your tables, ROMs, PinUP packs, AltSound and colorings stay where they are: nothing is moved, copied or changed when you switch, and you can switch back any time.',
    'PLAY, Play Hardcore and Play Softcore start the table the same way, from the library and from cabinet mode. Play Hardcore plays it as its author made it in both.',
    'A machine’s high scores and settings are the same in both. A table without a ROM keeps its own scores in a different place in each, so each shows its own.',
  ];
  const installed = (line) => app.vpxInstalled?.some((f) => f.line === line);
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="vc-title">
    <header>
      <h2 id="vc-title">{switching ? `Play your tables in VPX ${choosing}?` : 'VPX 10.8.1 or VPX 10.8.0'}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    <p>Both play your tables from the same folders. What changes is what the app can do around VPX while it plays.</p>
    <div class="cols">
      {#each [['10.8.1', NEWER], ['10.8.0', OLDER]] as [line, points] (line)}
        <section class="col" class:picked={(choosing || current) === line} aria-labelledby={`vc-${line}`}>
          <h3 id={`vc-${line}`}>
            VPX {line}
            {#if current === line}<span class="tag now">Plays your tables now</span>{:else if installed(line) === false}<span class="tag">Not on this PC</span>{/if}
          </h3>
          <ul>
            {#each points as [icon, text] (text)}<li><Icon name={icon} size={16} /><span>{text}</span></li>{/each}
          </ul>
        </section>
      {/each}
    </div>
    <section class="both" aria-label="Either way">
      <h3>Either way</h3>
      <ul>
        {#each BOTH as text (text)}<li><Icon name="check" size={16} /><span>{text}</span></li>{/each}
      </ul>
    </section>
    <footer>
      {#if switching}
        <button type="button" class="btn ghost" onclick={onclose}>Keep VPX {current}</button>
        <button type="button" class="btn go" onclick={choose} disabled={busy}>Play with VPX {choosing}</button>
      {:else}
        <button type="button" class="btn" onclick={onclose}>Close</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 80; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(980px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  h3 { margin: 0 0 8px; font-size: 16px; display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  p { margin: 0; color: var(--fg-soft); line-height: 1.5; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 12px; }
  .col { padding: 14px 16px; border-radius: 12px; border: 1px solid var(--control-line); background: rgba(255, 255, 255, 0.02); min-width: 0; }
  .col.picked { border-color: var(--accent); background: var(--accent-bg); }
  .both { padding: 12px 16px; border-radius: 12px; background: rgba(255, 255, 255, 0.03); }
  ul { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 9px; font-size: 14px; color: var(--fg-soft); line-height: 1.45; }
  li { display: flex; align-items: flex-start; gap: 10px; }
  li :global(svg) { flex: none; margin-top: 2px; color: var(--fg); }
  .tag { font-size: 12px; font-weight: 500; padding: 2px 8px; border-radius: 999px; border: 1px solid var(--control-line); color: var(--fg-soft); }
  .tag.now { border-color: var(--accent); color: var(--fg); }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
