<script>
  // Color it…: a table's machine's display colored, in three steps — its display recorded, by
  // itself first (`colour_by_itself`: its attract mode, then a game it plays on its own, no window,
  // in the background) and then played (`colour_record`: PLAY), every new frame kept as coloring
  // programs read a frame dump (the recordings so far, `colour_recordings`, their folder shown in
  // Explorer, `colour_show`); colored in a coloring program of its own, neither ours —
  // ColorizingDMD, the one that works today, downloaded by the user and opened from where they
  // chose it (`colorizingdmd_open`, the place kept in `prefs.colorizingDmd`), the recordings written
  // out as it opens them (`colour_for_colorizingdmd`); or PPUC's, not released yet — and the
  // finished coloring put where the player reads it: dropped on the table's page or its row, or
  // chosen here (`colour_use`), then played. On a table's page and from Tools › Color DMD.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import { app, notice, openLink, play, colourPrograms, loadColourPrograms, openColourProgram } from './store.svelte.js';

  let { t, onclose } = $props();

  const COLORIZINGDMD = 'https://github.com/SerumColor/ColorizingDMD/releases';
  const PPUC = 'https://github.com/PPUC/PPUC-Colorizing-DMD';

  $effect(() => {
    loadColourPrograms();
  });
  const program = (key) => colourPrograms.list?.find((p) => p.key === key);
  const pc = $derived(program('colorizingdmd'));
  const pp = $derived(program('ppuc'));

  let recordings = $state(null);
  // this table being recorded by itself now
  let byItself = $state(false);
  async function load() {
    recordings = await invoke('colour_recordings', { id: t.id }).catch(() => []);
    byItself = (await invoke('colour_by_itself_now').catch(() => null)) === t.id;
  }
  $effect(() => {
    load();
    // a game played to record ends: the library's read again, and its recording with it
    const off = listen('library-changed', () => load());
    // a recording by itself done
    const done = listen('colour-recorded', (e) => {
      if (e.payload?.id !== t.id) return;
      if (e.payload.problem) notice(e.payload.problem, { kind: 'problem' });
      load();
    });
    return () => {
      off.then((f) => f());
      done.then((f) => f());
    };
  });

  const frames = $derived(recordings ? recordings.reduce((n, r) => n + (r.frames || 0), 0) : 0);
  const size = (b) => (b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

  async function record() {
    try {
      await invoke('colour_record', { id: t.id });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function recordByItself() {
    try {
      await invoke('colour_by_itself', { id: t.id });
      byItself = true;
      notice(`Recording ${t.name} by itself, about four minutes. Its recordings show here and in Tools › Color DMD when it’s done.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  const show = () => invoke('colour_show', { id: t.id }).catch((e) => notice(String(e), { kind: 'problem' }));

  async function forColorizingDmd() {
    try {
      await invoke('colour_for_colorizingdmd', { id: t.id });
      notice(`${t.rom}.txt is ready for ColorizingDMD: its folder is open. In ColorizingDMD, open that file.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }


  async function use() {
    try {
      if (await invoke('colour_use', { id: t.id })) notice(`${t.name}’s coloring is in place. Play it to see it.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ci-title">
    <header>
      <h2 id="ci-title">Color {t.name}’s display</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    <ol class="steps">
      <li>
        <span class="n">1</span>
        <div class="body">
          <b>Record the display</b>
          <span class="what">By itself first: its attract mode, then a game it plays on its own — about four minutes, no window, in the background; you can close this and carry on. Then play it yourself to add what only a player reaches: the modes, multiballs, jackpots. Every new frame its machine shows is kept; the pause menu records too, from any ROM table’s game.</span>
          <span class="what">
            {#if recordings === null}
              …
            {:else if recordings.length}
              Recorded so far: {frames.toLocaleString()} frames, in {recordings.length} {recordings.length === 1 ? 'recording' : 'recordings'} ({size(recordings.reduce((n, r) => n + r.bytes, 0))}). They’re kept in the app’s data folder, and listed in Tools › Color DMD.
            {:else}
              Nothing recorded yet.
            {/if}
          </span>
          <div class="row">
            <button type="button" class="btn small go" disabled={byItself} onclick={recordByItself}>{byItself ? 'Recording by itself…' : 'Record it by itself'}</button>
            <button type="button" class="btn small" onclick={record}>Play and record</button>
            {#if recordings?.length}<button type="button" class="btn small ghost" onclick={show}><Icon name="folder" size={14} />Show the recordings</button>{/if}
          </div>
        </div>
      </li>
      <li>
        <span class="n">2</span>
        <div class="body">
          <b>Color it in a coloring program</b>
          <span class="what">Coloring is done by hand, scene by scene, in a program made for table authors. Both below are separate programs by other people — not part of Visual Pinball Infinite: we don’t make them, and we don’t control what they do.</span>
          <div class="prog">
            <div class="ptitle"><b>ColorizingDMD</b><span class="tag good">Works today</span></div>
            <span class="what">The program most colorings were made with; free, for Windows. The first time, Install fetches it from its makers’ page and unpacks it into this app’s folder. It opens a recording as one plain text file named for the ROM ({t.rom}.txt): Save the recordings for it makes that file and shows where it is.</span>
            <div class="row">
              <button type="button" class="btn small go" disabled={!pc || !!colourPrograms.installing} title={pc?.installed ? 'Opens it: a program of its own' : 'Fetches it from its makers’ page on GitHub and unpacks it into this app’s folder, the first time'} onclick={() => openColourProgram(pc)}>{colourPrograms.installing === 'colorizingdmd' ? 'Installing…' : pc?.installed ? 'Open ColorizingDMD' : 'Install ColorizingDMD'}</button>
              <button type="button" class="btn small" disabled={!recordings?.length} onclick={forColorizingDmd}>Save the recordings for it</button>
              <button type="button" class="btn small ghost" onclick={() => openLink(COLORIZINGDMD)}>Its page<Icon name="ext" size={13} /></button>
            </div>
          </div>
          <div class="prog">
            <div class="ptitle"><b>PPUC’s Colorizing DMD</b><span class="tag warn">Program not released yet</span></div>
            <span class="what">The new coloring program from the people who make Serum, meant to replace ColorizingDMD. Only test versions exist so far, and they save a newer kind of file that no player can play yet.</span>
            <div class="row">
              {#if pp && (pp.released || pp.installed)}
                <button type="button" class="btn small" disabled={!!colourPrograms.installing} onclick={() => openColourProgram(pp)}>{colourPrograms.installing === 'ppuc' ? 'Installing…' : pp.installed ? 'Open PPUC' : 'Install PPUC'}</button>
              {/if}
              <button type="button" class="btn small ghost" onclick={() => openLink(PPUC)}>Its page<Icon name="ext" size={13} /></button>
            </div>
          </div>
        </div>
      </li>
      <li>
        <span class="n">3</span>
        <div class="body">
          <b>Put the finished coloring in place</b>
          <span class="what">The coloring program saves a file named for the ROM ({t.rom}.cRZ). Drop that file onto this table’s page, or its row in Tools › Color DMD, and it goes where the player finds it — or choose it here. A coloring there before goes to the Recycle Bin.</span>
          <div class="row">
            <button type="button" class="btn small go" onclick={use}>Choose the finished file…</button>
            <button type="button" class="btn small" onclick={() => play(t)}><Icon name="play" size={13} fill />Play it</button>
          </div>
        </div>
      </li>
    </ol>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(680px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .steps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .steps li { display: flex; gap: 14px; align-items: flex-start; padding: 12px 14px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); }
  .n { flex: 0 0 auto; width: 26px; height: 26px; border-radius: 50%; display: grid; place-items: center; background: var(--accent-bg); color: #fff; font-size: 13px; font-weight: 700; }
  .body { flex: 1; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .body b { color: var(--fg); }
  .what { color: var(--fg-soft); font-size: 13px; line-height: 1.5; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 2px; }
  .btn.go { background: var(--accent-bg); border-color: var(--accent); color: #fff; }
  .prog { display: flex; flex-direction: column; gap: 6px; margin-top: 4px; padding: 10px 12px; border-radius: 10px; background: var(--control); border: 1px solid var(--control-line); }
  .ptitle { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .tag { padding: 2px 8px; border-radius: 6px; font-size: 11px; font-weight: 700; letter-spacing: 0.03em; }
  .tag.good { background: color-mix(in srgb, var(--good) 18%, transparent); color: var(--good); }
  .tag.warn { background: color-mix(in srgb, var(--warn) 18%, transparent); color: var(--warn); }
</style>
