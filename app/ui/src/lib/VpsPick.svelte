<script>
  // A table's VPS entry chosen by hand (when it isn't matched, or the guess
  // isn't it, VPS searched and the table and its version picked): find the
  // machine, pick its release, then keep the match in VPI only, or write it for every program by
  // renaming its files to VPS's name for it (PinUP Popper's game list changing with it), each
  // rename shown first, all of it put back from its settings.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { vps, loadVps, notice, chooseVps, follow } from './store.svelte.js';

  let { t, onclose } = $props();

  let query = $state(t.name);
  let machine = $state(null);
  let files = $state(null);
  let picked = $state(undefined);
  let how = $state('vpi');
  let renames = $state(null);
  let busy = $state(false);

  $effect(() => {
    if (!vps.machines.length) loadVps();
  });

  const found = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (q.length < 2) return [];
    const words = q.split(/\s+/);
    // the machine named so first, then those whose names begin so, then those with a VPX table
    const rank = (m) => {
      const n = m.name.toLowerCase();
      return n === q ? 0 : n.startsWith(q) ? 1 : 2;
    };
    return vps.machines
      .filter((m) => words.every((w) => [m.name, m.maker, String(m.year || ''), ...m.roms].some((s) => (s || '').toLowerCase().includes(w))))
      .sort((a, b) => rank(a) - rank(b) || (b.formats.includes('VPX') - a.formats.includes('VPX')) || a.name.localeCompare(b.name))
      .slice(0, 24);
  });

  async function pickMachine(m) {
    machine = m;
    picked = undefined;
    files = null;
    try {
      const g = await invoke('vps_game', { game: m.id, file: null });
      files = (g?.kinds.find((k) => k.kind === 'tables')?.files || []).filter((f) => !f.format || f.format === 'VPX');
    } catch (e) {
      files = [];
      notice(String(e), { kind: 'problem' });
    }
  }

  async function pickFile(f) {
    picked = f;
    renames = null;
    try {
      renames = await invoke('vps_names', { id: t.id, game: machine.id });
    } catch (e) {
      renames = { name: '', files: [], popper: null, blocked: String(e) };
    }
  }

  async function use() {
    busy = true;
    try {
      if (how === 'all' && renames?.files.length) {
        // renamed for every program: its page follows it to its new name once the library's read again
        const [was, short, to] = [t.id, t.short, renames.name];
        const id = await invoke('overwrite_vps', { id: was, game: machine.id, file: picked?.id ?? null });
        follow(was, id);
        notice(`${short} is renamed ${to}, for every program. Its settings can put the names back.`, { kind: 'done' });
      } else {
        await invoke('set_vps', { id: t.id, game: machine.id, file: picked?.id ?? null });
        notice(`${t.short} is ${machine.name}${picked?.version ? ` ${picked.version}` : ''} on VPS now, in VPI.`, { kind: 'done' });
      }
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  async function notOnVps() {
    await chooseVps(t, false);
    onclose();
  }

  const date = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '');
  const name = (p) => p.split('\\').pop();
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="vp-title">
    <header>
      <h2 id="vp-title">Find {t.short} on VPS</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    <p class="file">{t.file}.vpx</p>

    {#if picked !== undefined}
      <div class="chosen">
        {#if machine.picture}<img src={machine.picture} alt="" />{/if}
        <div>
          <b>{machine.name}</b>
          <span>{[machine.maker, machine.year].filter(Boolean).join(' · ')}{picked ? ` · ${picked.version ? `version ${picked.version}` : 'a release'}${picked.authors.length ? ` by ${picked.authors.join(', ')}` : ''}` : ' · no release in particular'}</span>
        </div>
        <button type="button" class="btn small ghost" onclick={() => (picked = undefined)}>Another</button>
      </div>
      <div class="hows" role="radiogroup" aria-label="How it's kept">
        <label class="how" class:on={how === 'vpi'}>
          <input type="radio" name="how" value="vpi" bind:group={how} />
          <span><b>Associate in VPI only</b><span class="what">VPI shows it as this VPS entry, with VPS’s facts and pictures. Its files stay as they are; other programs keep matching it their own way. You can change it later from its page.</span></span>
        </label>
        <label class="how" class:on={how === 'all'} class:later={renames?.blocked || (renames && !renames.files.length)}>
          <input type="radio" name="how" value="all" bind:group={how} disabled={!renames || !!renames.blocked || !renames.files.length} />
          <span>
            <b>Overwrite for all players (VPX, PinMAME, VPin Studio)</b>
            <span class="what">Renames its files to VPS’s name for it, so VPin Studio and PinUP Popper match it the same way, PinUP Popper’s game list changed with them (the list kept first). Its settings can put the names back.</span>
            {#if renames === null}
              <span class="what">Looking at its files…</span>
            {:else if renames.blocked}
              <span class="what blocked">{renames.blocked}</span>
            {:else if !renames.files.length}
              <span class="what">Its files are named so already.</span>
            {:else}
              <span class="renames">
                {#each renames.files as [from, to] (from)}<span><s>{name(from)}</s> → {name(to)}</span>{/each}
                {#if renames.popper}
                  {@const [nameWas, nameNow, shownWas, shownNow] = renames.popper}
                  {#if nameWas !== nameNow}<span>PinUP Popper’s name for it: <s>{nameWas}</s> → {nameNow}</span>{/if}
                  {#if shownWas !== shownNow}<span>PinUP Popper shows it as: <s>{shownWas}</s> → {shownNow}</span>{/if}
                  <span>PinUP Popper’s game list points to its new file name</span>
                {/if}
              </span>
            {/if}
          </span>
        </label>
      </div>
      <footer>
        <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
        <button type="button" class="btn go" disabled={busy} onclick={use}><Icon name="check" size={16} />Use it</button>
      </footer>
    {:else if machine}
      <div class="chosen">
        {#if machine.picture}<img src={machine.picture} alt="" />{/if}
        <div><b>{machine.name}</b><span>{[machine.maker, machine.year].filter(Boolean).join(' · ')}</span></div>
        <button type="button" class="btn small ghost" onclick={() => (machine = null)}>Back to the search</button>
      </div>
      <h3 class="section-title">Which of its tables you have</h3>
      {#if files === null}
        <p class="soft">Looking…</p>
      {:else}
        <div class="list">
          {#each files as f (f.id)}
            <button type="button" class="pick" onclick={() => pickFile(f)}>
              {#if f.picture}<img src={f.picture} alt="" loading="lazy" />{:else}<span class="noimg"><Icon name="file" size={18} /></span>{/if}
              <span class="txt">
                <b>{f.version ? `Version ${f.version}` : 'A release'}</b>
                <span>{[f.authors.join(', '), f.added && `on VPS since ${date(f.added)}`].filter(Boolean).join(' · ')}</span>
                {#if f.features.length}<span class="tags">{#each f.features as x (x)}<span class="tag">{x}</span>{/each}</span>{/if}
              </span>
            </button>
          {:else}
            <p class="soft">VPS lists no VPX table of it.</p>
          {/each}
          <button type="button" class="pick plain" onclick={() => pickFile(null)}><span class="txt"><b>None of these</b><span>Matched to the machine alone, with no release in particular</span></span></button>
        </div>
      {/if}
    {:else}
      <label class="search">
        <Icon name="search" />
        <!-- svelte-ignore a11y_autofocus -->
        <input type="search" bind:value={query} placeholder="A machine’s name, maker, year or ROM" aria-label="Search VPS" autofocus />
      </label>
      <div class="list">
        {#if !vps.machines.length}
          <p class="soft">Reading VPS’s list…</p>
        {:else}
          {#each found as m (m.id)}
            <button type="button" class="pick" onclick={() => pickMachine(m)}>
              {#if m.picture}<img src={m.picture} alt="" loading="lazy" />{:else}<span class="noimg"><Icon name="image" size={18} /></span>{/if}
              <span class="txt">
                <b>{m.name}</b>
                <span>{[m.maker, m.year, m.kind].filter(Boolean).join(' · ')}{m.formats.includes('VPX') ? '' : ' · no VPX table'}{m.mine ? ' · in your library' : ''}</span>
              </span>
            </button>
          {:else}
            <p class="soft">{query.trim().length < 2 ? 'Type a name to search VPS.' : 'No machine matches: try fewer words, or its maker or ROM.'}</p>
          {/each}
        {/if}
      </div>
      <footer>
        <button type="button" class="btn ghost" onclick={notOnVps}>It isn’t on VPS</button>
        <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(680px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  h3 { margin: 4px 0 0; }
  .file { margin: -6px 0 0; color: var(--faint); font-size: 13px; overflow-wrap: anywhere; }
  .soft { margin: 6px 2px; color: var(--muted); font-size: 14px; }
  .search { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 42px; border-radius: 10px; background: var(--bg); border: 1px solid #2a3242; color: var(--faint); }
  .search input { flex: 1; min-width: 0; background: transparent; border: 0; outline: none; color: var(--fg); font-size: 15px; }
  .list { display: flex; flex-direction: column; gap: 6px; max-height: 52vh; overflow: auto; }
  .pick { display: flex; align-items: center; gap: 12px; padding: 8px; border-radius: 10px; border: 1px solid var(--card-line); background: var(--card); text-align: left; color: var(--fg); }
  .pick:hover { border-color: var(--accent); }
  .pick img, .noimg { flex: 0 0 auto; width: 46px; height: 62px; border-radius: 6px; object-fit: cover; background: var(--picture-bg); display: inline-flex; align-items: center; justify-content: center; color: var(--faint); }
  .pick.plain { border-style: dashed; }
  .txt { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .txt span { color: var(--muted); font-size: 13px; }
  .tags { display: flex; flex-wrap: wrap; gap: 4px; }
  .tag { padding: 1px 6px; border-radius: 5px; background: var(--tag-bg); color: var(--tag-fg) !important; font-size: 11px !important; font-weight: 600; }
  .chosen { display: flex; align-items: center; gap: 12px; padding: 10px; border-radius: 12px; background: var(--card); border: 1px solid var(--accent); }
  .chosen img { width: 52px; height: 70px; border-radius: 6px; object-fit: cover; }
  .chosen div { flex: 1; display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .chosen span { color: var(--muted); font-size: 13px; }
  .hows { display: flex; flex-direction: column; gap: 8px; }
  .how { display: flex; align-items: flex-start; gap: 10px; padding: 12px; border-radius: 12px; border: 1px solid var(--card-line); background: var(--card); cursor: pointer; }
  .how.on { border-color: var(--accent); background: var(--accent-bg); }
  .how.later { cursor: default; opacity: 0.75; }
  .how input { margin-top: 3px; }
  .how > span { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .what { color: var(--muted); font-size: 13px; }
  .what.blocked { color: var(--warn); }
  .renames { display: flex; flex-direction: column; gap: 2px; font-size: 12px; color: var(--fg-soft); overflow-wrap: anywhere; }
  .renames s { color: var(--faint); }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
