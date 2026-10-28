<script>
  // The drop window: each download
  // dropped, the table it brings — matched to VPS — or the table its files are for, every file with
  // where it goes and what's there already, ticked or not; what nothing could place grouped by
  // folder, with a place to choose; its read-me. Nothing is copied until Add.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import VpsFiles from './VpsFiles.svelte';
  import { app, drop, addDrop, closeDrop, chooseDropTable, removeFromDrop, openTable, savePrefs, openLink, tableById, isPack } from './store.svelte.js';

  const tables = $derived([...app.tables].sort((a, b) => a.name.localeCompare(b.name)));
  // the tables being added in the drop, not in the library yet: files can be for them too
  const adding = $derived(
    drop.downloads
      .filter((d) => d.table && !d.installedId)
      .map((d) => ({ id: `new:${d.table.stem}`, name: d.table.sure ? d.table.game || d.table.stem : d.table.ownName || d.table.stem, year: d.table.sure ? d.table.year : null }))
      .filter((t, i, all) => all.findIndex((o) => o.id.toLowerCase() === t.id.toLowerCase()) === i),
  );

  // how a download's table was found, as its tag says it
  const howFound = (d) => (!d.forTableId ? d.forNote : /^(by|named) /.test(d.forNote) ? `Found ${d.forNote}` : d.forNote[0].toUpperCase() + d.forNote.slice(1));

  const stateText = (r) =>
    ({
      new: 'New',
      same: 'There already, the same',
      some: 'Some there already',
      replace: 'A different one there',
      unsorted: 'Yours to place',
    })[r.state] ||
    r.note ||
    'Not copied';

  const size = (bytes) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`);

  const ticked = (i, r) => drop.choices[i]?.include[r];

  // two ticked files going to the same place: only one can be there
  const clash = $derived.by(() => {
    const first = new Map();
    const out = new Set();
    drop.downloads.forEach((d, i) =>
      d.rows.forEach((r, k) => {
        if (!r.to || !ticked(i, k)) return;
        const place = r.to.toLowerCase();
        if (first.has(place)) out.add(first.get(place)).add(`${i}/${k}`);
        else first.set(place, `${i}/${k}`);
      }),
    );
    return out;
  });
  // one left unticked as it goes where another one does
  const samePlace = (i, k, r) => !!r.to && drop.downloads.some((d, j) => d.rows.some((o, m) => (j !== i || m !== k) && o.to && o.to.toLowerCase() === r.to.toLowerCase() && ticked(j, m)));

  /** Whether what's ticked is there already, the same, so there's nothing to copy. */
  const allSame = $derived(drop.downloads.some((d, i) => d.rows.some((r, k) => ticked(i, k) && r.state === 'same')));
  /** The files the Add button copies: every ticked row's, but those there already. */
  const count = $derived(
    drop.downloads.reduce((n, d, i) => n + d.rows.reduce((m, r, k) => m + (ticked(i, k) && r.state !== 'same' ? r.files : 0), 0), 0),
  );
  // what's there already, row by row (asked each time, with Append as well)
  const there = (r) => r.state === 'replace' || r.state === 'some';
  const conflicts = $derived(drop.downloads.flatMap((d, i) => d.rows.map((r, k) => [i, k, r]).filter(([, k, r]) => there(r) && ticked(i, k))));
  const unchosen = $derived(conflicts.filter(([i, k]) => !drop.choices[i]?.already?.[k]).length);
  const ALREADY = {
    replace: ['Replace it', 'The old one to the Recycle Bin'],
    keepBoth: ['Keep both', 'The new one beside it, numbered'],
    append: ['Append', 'Its new files added to the folder there; one of the same name replaced'],
    skip: ['Leave it', 'Nothing of it copied'],
  };
  const options = (r) => (isPack(r) ? ['replace', 'append', 'skip'] : ['replace', 'keepBoth', 'skip']);

  function choose(i, k, v) {
    const c = drop.choices[i];
    const next = { ...(c.already || {}) };
    if (v) next[k] = v;
    else delete next[k];
    c.already = next;
  }

  // every row there already set at once: a pack's Keep both is its Append
  function chooseAll(v) {
    for (const [i, k, r] of conflicts) choose(i, k, v && (isPack(r) ? (v === 'keepBoth' ? 'append' : v) : v === 'append' ? 'replace' : v));
  }

  function put(i, k, key) {
    const c = drop.choices[i];
    if (key) {
      c.put[k] = key;
      c.include[k] = true;
    } else {
      delete c.put[k];
      c.include[k] = false;
    }
  }

  function placeLabel(d, key) {
    return d.places.find((p) => p[0] === key)?.[1] || '';
  }

  // "See them": a second panel beside the downloads, inside the window, with the VPS table a download
  // is — its own, or the library's table it's for — and every file VPS lists for it, with links
  let seeing = $state(null);
  let seeOpen = $state({ tables: true });

  function vpsOf(d) {
    if (d.table?.gameId && d.table.sure) return { game: d.table.gameId, file: d.table.fileId || null };
    const t = d.forTableId ? tableById(d.forTableId) : null;
    return t?.onVps?.game && !t.guess ? { game: t.onVps.game, file: t.onVps.tableFile || null } : null;
  }

  async function see(d) {
    const v = vpsOf(d);
    if (!v) return;
    if (seeing?.source === d.source) {
      seeing = null;
      return;
    }
    // the release marked: the one dropped, or the user's own the files are for
    const mine = d.table ? 'Dropped' : 'Yours';
    seeing = { source: d.source, view: null, mine };
    seeOpen = { tables: true };
    const view = await invoke('vps_game', v).catch(() => null);
    if (seeing?.source === d.source) seeing = { source: d.source, view, failed: !view, mine };
  }

  const pct = $derived(drop.progress && drop.progress.total ? Math.round((drop.progress.done / drop.progress.total) * 100) : 0);

  function close() {
    if (!drop.busy) closeDrop();
  }

  function key(e) {
    if (e.key === 'Escape') close();
  }

  const title = $derived(
    drop.results ? 'Added' : drop.downloads.length === 0 ? 'Add tables' : drop.downloads.length === 1 ? 'Adding a download' : `Adding ${drop.downloads.length} downloads`,
  );
</script>

<svelte:window onkeydown={key} />

<div class="scrim">
  <div class="win" class:seeing={!!seeing && !drop.results} role="dialog" aria-modal="true" aria-labelledby="drop-title">
    <header>
      <div>
        <h1 id="drop-title" class="display">{title}</h1>
        <p>
          {#if drop.results}What was copied, download by download.{:else if drop.downloads.length}Dropped on the window. Nothing is copied until you say so; more files dropped here join them.{:else}Drop table downloads — files, folders or archives (zip, 7-Zip, RAR) — anywhere on this window.{/if}
        </p>
      </div>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={close} disabled={drop.busy}><Icon name="x" /></button>
    </header>

    <div class="columns">
    <div class="body">
      {#if drop.error}<p class="problem">{drop.error}</p>{/if}

      {#if drop.results}
        {#each drop.results as res (res.name)}
          <article class="dl">
            <div class="dl-head"><b>{res.name}</b></div>
            {#if res.problem}
              <p class="problem">{res.problem}</p>
            {:else}
              {@const a = res.added}
              <ul class="done">
                <li>{a.copied === 0 ? 'Nothing new to copy' : a.copied === 1 ? '1 file copied' : `${a.copied} files copied`}{a.bytes ? `, ${size(a.bytes)}` : ''}</li>
                {#if a.replaced}<li>{a.replaced} replaced — the old ones are in the Recycle Bin</li>{/if}
                {#if a.keptBoth}<li>{a.keptBoth} kept beside the old ones, numbered</li>{/if}
                {#if a.same}<li>{a.same} there already, left as they were</li>{/if}
                {#if a.skipped}<li>{a.skipped} not copied, as you chose</li>{/if}
                {#if a.removed}<li>{a.removed} old {a.removed === 1 ? 'file' : 'files'} of a replaced pack that the new one hasn’t — in the Recycle Bin</li>{/if}
                {#each a.problems as p (p)}<li class="bad">{p}</li>{/each}
              </ul>
              {#each res.tables.filter((t) => !t.sure) as t (t.id)}
                <p class="check warn">
                  <Icon name="alert" size={14} width={2.4} />{t.name} isn’t surely found on VPS: confirm its match on its page.
                  {#if tableById(t.id)}<button type="button" class="btn small" onclick={() => { closeDrop(); openTable(t.id, app.tables.map((x) => x.id)); }}>Open its page</button>{/if}
                </p>
              {/each}
              {#each res.tested as [table, look, text] (table)}
                <p class="check {look}"><Icon name={look === 'good' ? 'check' : 'alert'} size={14} width={2.4} />Tested {table}: {text}</p>
              {/each}
            {/if}
          </article>
        {/each}
      {:else}
        {#if drop.busy && !drop.progress}<p class="soft">Looking inside…</p>{/if}
        {#each drop.downloads as d, i (d.source)}
          <article class="dl">
            <div class="dl-head">
              <b>{d.name}</b><span>{d.what}</span>
              {#if !d.error && vpsOf(d)}
                <button type="button" class="btn small ghost see" aria-pressed={seeing?.source === d.source} onclick={() => see(d)}><Icon name="search" size={14} />{seeing?.source === d.source ? 'Hide VPS' : 'See them on VPS'}</button>
              {/if}
              <!-- out of this drop only: the file itself stays where it is -->
              <button type="button" class="btn icon ghost small out" aria-label="Take {d.name} out of this drop" title="Take it out of this drop (the file stays where it is)" disabled={drop.busy} onclick={() => { if (seeing?.source === d.source) seeing = null; removeFromDrop(i); }}><Icon name="x" size={16} /></button>
            </div>
            {#if d.error}
              <p class="problem">{d.error}</p>
            {:else}
              {#if d.table}
                {@const t = d.table}
                <div class="match">
                  <div class="who">
                    {#if t.sure}
                      <b>{t.game || t.stem}</b>
                      <span>{[t.maker, t.year].filter(Boolean).join(' ')}{t.version ? ` · release ${t.version}` : ''}{t.authors.length ? ` by ${t.authors.slice(0, 3).join(', ')}${t.authors.length > 3 ? ` +${t.authors.length - 3}` : ''}` : ''}</span>
                    {:else}
                      <!-- only a guess on VPS: named as its own file names it -->
                      <b>{t.ownName || t.stem}</b>
                      <span>{[t.ownVersion ? `release ${t.ownVersion}` : '', t.ownAuthors ? `by ${t.ownAuthors}` : ''].filter(Boolean).join(' ') || t.file}</span>
                    {/if}
                  </div>
                  <div class="tags">
                    <span class="tag" class:guess={!t.sure}>{t.sure ? `Found on VPS ${t.how}` : t.gameId ? `Our guess on VPS: ${t.game}${t.year ? ` (${[t.maker, t.year].filter(Boolean).join(' ')})` : ''}, to confirm on its page once added` : 'Not on VPS'}</span>
                    {#if t.gameName && t.pinmame}<span class="tag">Its machine: {t.gameName}</span>{/if}
                    {#if d.installedId}<button type="button" class="tag link" onclick={() => { closeDrop(); openTable(d.installedId, app.tables.map((x) => x.id)); }}>In your library already</button>{/if}
                  </div>
                </div>
              {:else}
                <div class="match">
                  <label class="for">
                    <span>For</span>
                    <select class="select" value={d.forTableId || ''} onchange={(e) => e.currentTarget.value && chooseDropTable(i, e.currentTarget.value)} disabled={drop.busy}>
                      {#if !d.forTableId}<option value="">Choose its table…</option>{/if}
                      {#if adding.length}
                        <optgroup label="Being added">
                          {#each adding as t (t.id)}<option value={t.id}>{t.name}{t.year ? ` (${t.year})` : ''}</option>{/each}
                        </optgroup>
                        <optgroup label="In your library">
                          {#each tables as t (t.id)}<option value={t.id}>{t.name}{t.year ? ` (${t.year})` : ''}</option>{/each}
                        </optgroup>
                      {:else}
                        {#each tables as t (t.id)}<option value={t.id}>{t.name}{t.year ? ` (${t.year})` : ''}</option>{/each}
                      {/if}
                    </select>
                  </label>
                  {#if d.forNote}<span class="tag" class:guess={!d.forTableId || d.forNote === 'being added with it'}>{howFound(d)}</span>{/if}
                </div>
              {/if}
              {#each d.notes as n (n)}<p class="note">{n}</p>{/each}
              {#if d.sameMachine?.length}
                <p class="note">Your library has {d.sameMachine.length === 1 ? 'another release' : 'other releases'} of this machine: {d.sameMachine.join(', ')}. Add this one too, or leave it if it’s the same.</p>
              {/if}

              <table class="files">
                <thead><tr><th class="tick"><span class="visually-hidden">Add</span></th><th>What</th><th>File</th><th>Goes to</th><th>Now</th></tr></thead>
                <tbody>
                  {#each d.rows as r, k (k)}
                    <tr class={r.state}>
                      <td class="tick">
                        {#if r.state !== 'skip' && r.state !== 'unsorted'}
                          <input type="checkbox" aria-label="Add {r.name}" checked={ticked(i, k)} onchange={(e) => (drop.choices[i].include[k] = e.currentTarget.checked)} />
                        {/if}
                      </td>
                      <td class="what">{r.label}</td>
                      <td class="name" title={r.name}>{r.name}{#if r.files > 1}<span class="many"> · {r.files} files, {size(r.size)}</span>{/if}</td>
                      <td class="to">
                        {#if r.state === 'unsorted'}
                          <select class="select small" aria-label="Where {r.name} goes" value={drop.choices[i].put[k] || ''} onchange={(e) => put(i, k, e.currentTarget.value)}>
                            <option value="">Leave it out</option>
                            {#each d.places as [pk, label] (pk)}<option value={pk}>{label}</option>{/each}
                          </select>
                        {:else if r.to}
                          <span title={r.to}>{r.folder}</span>
                        {:else}
                          <span class="soft">—</span>
                        {/if}
                      </td>
                      <td class="now">
                        {#if there(r) && ticked(i, k)}
                          {@const v = drop.choices[i].already?.[k] || ''}
                          <select class="select small" class:ask={!v} aria-label="{r.name} is there already: what to do" value={v} title={v ? ALREADY[v][1] : ''} onchange={(e) => choose(i, k, e.currentTarget.value)}>
                            <option value="">{r.state === 'some' ? 'Some there: choose…' : 'There already: choose…'}</option>
                            {#each options(r) as o (o)}<option value={o}>{ALREADY[o][0]}{isPack(r) && o === 'replace' ? ' — the whole pack' : ''}</option>{/each}
                          </select>
                        {:else}
                          {r.state === 'unsorted' && drop.choices[i].put[k] ? `To ${placeLabel(d, drop.choices[i].put[k]).toLowerCase()}` : stateText(r)}
                        {/if}
                      </td>
                    </tr>
                    {#if r.check || (r.note && r.state !== 'skip' && r.state !== 'unsorted') || (r.to && samePlace(i, k, r))}
                      <tr class="sub">
                        <td></td>
                        <td colspan="4">
                          <div class="subtext">
                            {#if r.to && samePlace(i, k, r)}<span class="check {clash.has(`${i}/${k}`) ? 'bad' : 'warn'}"><Icon name="alert" size={14} width={2.4} />Another file in this drop goes to the same place, {r.to.split(/[\\/]/).pop()}: only one can be there{clash.has(`${i}/${k}`) ? ' — untick one' : ''}</span>{/if}
                            {#if r.note && r.state !== 'skip' && r.state !== 'unsorted'}<span class="rnote">{r.note[0].toUpperCase() + r.note.slice(1)}</span>{/if}
                            {#if r.check}<span class="check {r.check.look}"><Icon name={r.check.look === 'good' ? 'check' : 'alert'} size={14} width={2.4} />{r.check.text}</span>{/if}
                          </div>
                        </td>
                      </tr>
                    {/if}
                  {/each}
                </tbody>
              </table>
              {#if d.readme}
                <details class="readme"><summary>Its read-me</summary><pre>{d.readme}</pre></details>
              {/if}
            {/if}
          </article>
        {/each}
      {/if}
    </div>
    {#if seeing && !drop.results}
      <aside class="see-panel" aria-label="On VPS">
        {#if !seeing.view}
          <p class="soft">{seeing.failed ? 'VPS’s list isn’t at hand just now.' : 'Looking…'}</p>
        {:else}
          {@const g = seeing.view}
          <div class="see-head">
            {#if g.picture}<img src={g.picture} alt="" />{/if}
            <div class="see-who">
              <b>{g.name}</b>
              <span>{[g.maker, g.year].filter(Boolean).join(' · ')}</span>
              <button type="button" class="btn small" onclick={() => openLink(g.link)}><Icon name="ext" size={14} />On VPS’s site</button>
            </div>
            <button type="button" class="btn icon ghost" aria-label="Hide VPS" onclick={() => (seeing = null)}><Icon name="x" size={16} /></button>
          </div>
          <VpsFiles kinds={g.kinds} bind:open={seeOpen} mine={seeing.mine} />
        {/if}
      </aside>
    {/if}
    </div>

    <footer>
      {#if drop.progress}
        <div class="progress" role="status">
          {#if drop.progress.testing}
            <span>Testing {drop.progress.testing}: running it with no window for about 15 seconds…</span>
          {:else}
            <span>Adding {drop.progress.name || '…'}{drop.progress.of > 1 ? ` (${drop.progress.download + 1} of ${drop.progress.of})` : ''}</span>
            <div class="bar"><div style="width: {pct}%"></div></div>
          {/if}
        </div>
      {:else if drop.results}
        <span class="where">The library is read again to show them.</span>
        <button type="button" class="btn save" onclick={closeDrop}><Icon name="check" />Done</button>
      {:else}
        {#if conflicts.length}
          <div class="already" role="group" aria-label="What is there already">
            <span>{conflicts.length === 1 ? 'One is there already' : `${conflicts.length} are there already`}{unchosen ? `, ${unchosen} still to choose for` : ''}. All of them:</span>
            <div class="seg">
              <button type="button" onclick={() => chooseAll('replace')}>Replace</button>
              <button type="button" onclick={() => chooseAll('keepBoth')} title="A pack's new files are appended to it">Keep both / Append</button>
              <button type="button" onclick={() => chooseAll('skip')}>Leave</button>
              <button type="button" onclick={() => chooseAll(null)}>Ask each</button>
            </div>
            <span class="soft">Replaced files go to the Recycle Bin</span>
          </div>
        {/if}
        <span class="where">Into the folders shared with VPX, PinUP Popper and VPin Studio, under C:\vPinball.</span>
        <label class="test-new" title="Each table added runs for about 15 seconds with no window: its physics, its script, and its machine for a ROM table">
          <input type="checkbox" checked={!!app.prefs.testNewTables} onchange={(e) => { app.prefs.testNewTables = e.currentTarget.checked; savePrefs(); }} />Test new tables after adding them
        </label>
        <button type="button" class="btn ghost" onclick={close} disabled={drop.busy}>Cancel</button>
        <button type="button" class="btn save" onclick={addDrop} disabled={drop.busy || count === 0 || unchosen > 0 || clash.size > 0} title={unchosen ? 'Choose first what to do with each that is there already' : clash.size ? 'Two ticked files go to the same place: untick one' : count === 0 && allSame ? 'What’s ticked is there already, the same files' : undefined}><Icon name="download" />{clash.size ? 'Two go to the same place' : unchosen ? `Choose for ${unchosen} there already` : count === 0 && allSame ? 'Nothing new to add' : count === 1 ? 'Add 1 file' : `Add ${count} files`}</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 40; background: rgba(5, 7, 11, 0.72);
    display: flex; align-items: center; justify-content: center; padding: 28px;
  }
  .win {
    width: min(1120px, 100%); max-height: 100%; display: flex; flex-direction: column;
    background: var(--panel); border: 1px solid var(--panel-line); border-radius: 18px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6);
  }
  .win.seeing { width: min(1560px, 100%); }
  .columns { flex: 1; min-height: 0; display: flex; }
  .columns > .body { flex: 1; min-width: 0; }
  .see-panel {
    flex: 0 0 420px; overflow: auto; padding: 16px 20px; border-left: 1px solid var(--line-soft);
    display: flex; flex-direction: column; gap: 12px;
  }
  .see-head { display: flex; gap: 12px; align-items: flex-start; }
  .see-head img { width: 120px; height: 90px; border-radius: 8px; object-fit: cover; background: var(--picture-bg); flex: 0 0 auto; }
  .see-who { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; align-items: flex-start; }
  .see-who b { font-size: 15px; }
  .see-who span { color: var(--faint); font-size: 13px; }
  .dl-head .see { margin-left: auto; }
  .dl-head .out { align-self: center; color: var(--muted); }
  .dl-head .see + .out { margin-left: 0; }
  .dl-head .out:first-of-type { margin-left: auto; }
  .dl-head .out:hover:not(:disabled) { color: #f0a8a8; }
  header { display: flex; justify-content: space-between; gap: 16px; padding: 22px 24px 14px; border-bottom: 1px solid var(--line-soft); }
  h1 { font-size: 30px; line-height: 1.05; }
  header p { margin: 6px 0 0; color: var(--muted); }
  .body { overflow: auto; padding: 16px 24px; display: flex; flex-direction: column; gap: 14px; min-height: 120px; }
  .soft { color: var(--faint); }
  .problem { margin: 0; padding: 10px 14px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .dl { border: 1px solid var(--card-line); background: var(--card); border-radius: 14px; padding: 14px 16px; display: flex; flex-direction: column; gap: 10px; }
  .dl-head { display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; }
  .dl-head b { font-size: 15px; }
  .dl-head span { color: var(--faint); font-size: 13px; }
  .match { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; justify-content: space-between; }
  .who { display: flex; flex-direction: column; }
  .who b { font-size: 17px; }
  .who span { color: var(--muted); font-size: 13px; }
  .tags { display: flex; gap: 8px; flex-wrap: wrap; }
  .tag { font-size: 12px; padding: 4px 10px; border-radius: 999px; background: #143024; color: var(--good); border: 0; }
  .tag.guess { background: #2a2414; color: var(--warn); }
  .tag.link { cursor: pointer; background: var(--accent-bg); color: #e9f6ff; }
  .for { display: flex; gap: 10px; align-items: center; }
  .for span { color: var(--fg-soft); }
  .note { margin: 0; font-size: 13px; color: var(--warn); }
  .files { width: 100%; border-collapse: collapse; font-size: 13px; table-layout: fixed; }
  .files th { text-align: left; font-weight: 600; color: var(--faint); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; padding: 4px 6px; }
  .files td { padding: 6px; border-top: 1px solid var(--line-soft); vertical-align: middle; }
  .tick { width: 34px; }
  .what { width: 170px; color: var(--fg-soft); }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .many { color: var(--faint); }
  .to { width: 210px; }
  .now { width: 190px; color: var(--muted); }
  tr.new .now { color: var(--good); }
  tr.replace .now { color: var(--warn); }
  tr.skip td { color: var(--faint); }
  tr.unsorted .now { color: var(--warn); }
  tr.sub td { border-top: 0; padding-top: 0; }
  .subtext { display: flex; flex-direction: column; gap: 3px; font-size: 12px; }
  .rnote { color: var(--faint); }
  .check { display: inline-flex; align-items: flex-start; gap: 6px; line-height: 1.4; }
  .check.good { color: var(--good); }
  .check.warn { color: var(--warn); }
  .check.bad { color: var(--bad); }
  p.check { margin: 6px 0 0; font-size: 13px; }
  .test-new { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .select.small { height: 30px; font-size: 12px; max-width: 200px; }
  .select.small.ask { border-color: var(--warn); color: var(--warn); }
  .readme summary { cursor: pointer; color: var(--accent); font-size: 13px; }
  .readme pre { margin: 8px 0 0; white-space: pre-wrap; font: inherit; font-size: 13px; color: var(--fg-soft); background: var(--bg); padding: 10px 12px; border-radius: 8px; }
  .done { margin: 0; padding-left: 18px; color: var(--fg-soft); display: flex; flex-direction: column; gap: 2px; }
  .done .bad { color: var(--bad); }
  footer { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; justify-content: flex-end; padding: 14px 24px 18px; border-top: 1px solid var(--line-soft); }
  .where { flex: 1 1 320px; color: var(--faint); font-size: 12px; }
  .save { background: var(--accent-bg); border-color: var(--accent); }
  .already { flex: 1 1 100%; display: flex; gap: 10px; align-items: center; flex-wrap: wrap; font-size: 13px; color: var(--fg-soft); }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 30px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-pressed='true'] { background: var(--accent-bg); color: #fff; }
  .progress { flex: 1; display: flex; flex-direction: column; gap: 8px; }
  .bar { height: 8px; border-radius: 999px; background: var(--control); overflow: hidden; }
  .bar div { height: 100%; background: var(--accent); transition: width 0.2s; }
</style>
