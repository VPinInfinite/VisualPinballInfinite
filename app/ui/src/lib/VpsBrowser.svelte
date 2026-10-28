<script>
  // The VPS browser: every machine the Virtual Pinball Spreadsheet lists, with its picture, the
  // catalog's filters — maker, era, machine type, table format, a sort, only machines not in the
  // library — and "VPS has" chips, the cards' and the side panel's sizes adjustable; the machine chosen
  // beside the cards with every file VPS lists for it, so there's no need to visit VPS's site. Its
  // search is the top bar's while it shows, apart from the library's.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import VpsFiles from './VpsFiles.svelte';
  import { app, vps, loadVps, savePrefs, openTable, openLink, vpidbPacks, loadVpidbPacks } from './store.svelte.js';

  // read again as the library changes: which machines are in it
  $effect(() => {
    void app.tables;
    void app.loaded;
    loadVps();
  });

  const tile = $derived(app.prefs.vpsTile || 190);
  const panelWidth = $derived(app.prefs.vpsPanel || 460);

  const ERAS = [
    ['all', 'All eras'],
    ['pre70', 'Before 1970'],
    ['1970s', '1970s'],
    ['1980s', '1980s'],
    ['1990s', '1990s'],
    ['2000s', '2000s and later'],
  ];
  const TYPES = [
    ['all', 'All types'],
    ['SS', 'Solid state (SS)'],
    ['EM', 'Electro-mechanical (EM)'],
    ['PM', 'Pure mechanical (PM)'],
    ['Original', 'Original'],
  ];
  const CHIPS = [
    ['backglasses', 'Backglass', 'image', 'B2S'],
    ['roms', 'ROM', 'chip', 'ROM'],
    ['pupPacks', 'PinUP pack', 'layers', 'PUP'],
    ['colour', 'Color DMD', 'palette', 'Color'],
    ['altSound', 'AltSound', 'music', 'AltSound'],
    ['sounds', 'Sound pack', 'speaker', 'Sound'],
    ['wheels', 'Wheel', 'disc', 'Wheel'],
    ['toppers', 'Topper', 'topper', 'Topper'],
    ['pov', 'POV', 'eye', 'POV'],
    ['mediaPacks', 'Media pack', 'film', 'Media'],
    ['rules', 'Rule card', 'rules', 'Rules'],
  ];

  const era = (year) => (!year ? null : year < 1970 ? 'pre70' : year < 1980 ? '1970s' : year < 1990 ? '1980s' : year < 2000 ? '1990s' : '2000s');

  // the makers, the most machines first
  const makers = $derived.by(() => {
    const counts = new Map();
    for (const m of vps.machines) if (m.maker) counts.set(m.maker, (counts.get(m.maker) || 0) + 1);
    return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  });

  const shown = $derived.by(() => {
    const q = vps.query.trim().toLowerCase();
    const has = (s) => (s || '').toLowerCase().includes(q);
    let list = vps.machines.filter(
      (m) =>
        (vps.format === 'any' ? true : m.formats.includes('VPX')) &&
        (vps.maker === 'all' || m.maker === vps.maker) &&
        (vps.era === 'all' || era(m.year) === vps.era) &&
        (vps.type === 'all' || m.badge === vps.type) &&
        (!vps.notMine || !m.mine) &&
        vps.has.every((k) => (k === 'achievements' ? packsOf(m) > 0 : m.has[k] > 0)) &&
        (!q || [m.name, m.maker, ...m.designers, ...m.authors, ...m.roms, ...m.theme].some(has)),
    );
    const s = vps.sort;
    const releases = (m) => (vps.format === 'any' ? m.releases : m.has.tables || 0);
    return s === 'name' ? [...list].sort((a, b) => a.name.localeCompare(b.name))
      : s === 'year' ? [...list].sort((a, b) => (a.year || 9999) - (b.year || 9999) || a.name.localeCompare(b.name))
      : s === 'releases' ? [...list].sort((a, b) => releases(b) - releases(a) || a.name.localeCompare(b.name))
      : list;
  });

  // a few hundred at a time, more as the page scrolls down to them
  let limit = $state(150);
  let sentinel = $state();
  $effect(() => {
    void shown;
    limit = 150;
  });
  $effect(() => {
    if (!sentinel) return;
    const seen = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) limit += 150;
    }, { rootMargin: '600px' });
    seen.observe(sentinel);
    return () => seen.disconnect();
  });

  // the machine beside the cards: the one chosen, else the first shown
  const selId = $derived(vps.sel && vps.machines.some((m) => m.id === vps.sel) ? vps.sel : shown[0]?.id);
  const sel = $derived(vps.machines.find((m) => m.id === selId));
  let game = $state(null);
  let openKinds = $state({ tables: true });
  $effect(() => {
    const id = selId;
    game = null;
    if (!id) return;
    invoke('vps_game', { game: id, file: null }).then((g) => {
      if (selId === id) game = g;
    });
  });

  // VPIDB's achievement packs for each machine, by its VPS ID
  $effect(() => {
    loadVpidbPacks();
  });
  const packsOf = (m) => vpidbPacks.byGame?.[m.id]?.length ?? 0;

  let failed = $state({});
  const tags = (m) => [...(packsOf(m) ? ['Achievements'] : []), ...CHIPS.filter(([k]) => m.has[k] > 0).map(([, , , short]) => short)];
  const sub = (m) => [m.maker, m.year].filter(Boolean).join(' · ');
  const date = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' }) : '');

  function setPref(name, value) {
    app.prefs[name] = value;
    savePrefs();
  }

  function toggleHas(k) {
    vps.has = vps.has.includes(k) ? vps.has.filter((x) => x !== k) : [...vps.has, k];
  }

  function clearAll() {
    Object.assign(vps, { query: '', maker: 'all', era: 'all', type: 'all', format: 'VPX', notMine: false, has: [] });
  }

  // the panel's width, dragged at its edge
  let dragging = $state(null);
  function startDrag(e) {
    dragging = { x: e.clientX, w: panelWidth };
    e.currentTarget.setPointerCapture(e.pointerId);
  }
  function drag(e) {
    if (!dragging) return;
    app.prefs.vpsPanel = Math.round(Math.min(760, Math.max(340, dragging.w + dragging.x - e.clientX)));
  }
  function endDrag() {
    if (dragging) savePrefs();
    dragging = null;
  }
</script>

<div class="browser">
  <main>
    <div class="head">
      <div class="titles">
        <h1 class="display">VPS browser</h1>
        <div class="sub">
          {#if vps.machines.length}
            {shown.length === vps.machines.length ? `${vps.machines.length.toLocaleString()} machines` : `${shown.length.toLocaleString()} of ${vps.machines.length.toLocaleString()} machines`} on the Virtual Pinball Spreadsheet{vps.query ? ` matching “${vps.query}”` : ''}{vps.refreshed ? ` · its list fetched ${date(vps.refreshed)}` : ''}
          {:else if !app.loaded}
            Reading VPS’s list…
          {:else}
            VPS’s list couldn’t be read: it’s fetched once a day, when the app opens.
          {/if}
        </div>
      </div>
      <div class="controls">
        <label class="labelled size">Size
          <input type="range" min="140" max="320" step="10" value={tile} oninput={(e) => setPref('vpsTile', Number(e.currentTarget.value))} aria-label="Card size" />
        </label>
      </div>
    </div>

    <div class="filters">
      <label class="toggle"><input type="checkbox" bind:checked={vps.notMine} />Only machines I don’t have</label>
      <label class="labelled">Maker
        <select class="select" bind:value={vps.maker}>
          <option value="all">All makers</option>
          {#each makers as [m, n] (m)}<option value={m}>{m} ({n})</option>{/each}
        </select>
      </label>
      <label class="labelled">Era
        <select class="select" bind:value={vps.era}>
          {#each ERAS as [v, l] (v)}<option value={v}>{l}</option>{/each}
        </select>
      </label>
      <label class="labelled">Machine type
        <select class="select" bind:value={vps.type}>
          {#each TYPES as [v, l] (v)}<option value={v}>{l}</option>{/each}
        </select>
      </label>
      <label class="labelled">Table format
        <select class="select" bind:value={vps.format}>
          <option value="VPX">Visual Pinball X</option>
          <option value="any">Any format</option>
        </select>
      </label>
      <label class="labelled">Sort by
        <select class="select" bind:value={vps.sort}>
          <option value="newest">Newest release</option>
          <option value="name">Name</option>
          <option value="year">Year</option>
          <option value="releases">Most releases</option>
        </select>
      </label>
    </div>

    <div class="chips">
      <span class="has">VPS has</span>
      {#each CHIPS as [k, label, icon] (k)}
        <button type="button" class="chip" aria-pressed={vps.has.includes(k)} onclick={() => toggleHas(k)}><Icon name={icon} size={14} />{label}</button>
      {/each}
      <button type="button" class="chip" aria-pressed={vps.has.includes('achievements')} title="Achievement packs on VPIDB" onclick={() => toggleHas('achievements')}><Icon name="trophy" size={14} />Achievements</button>
    </div>

    <div class="grid" style="--tile: {tile}px">
      {#each shown.slice(0, limit) as m (m.id)}
        <button type="button" class="card" class:on={m.id === selId} onclick={() => (vps.sel = m.id)} aria-label="{m.name}, {sub(m)}" aria-pressed={m.id === selId}>
          <div class="picture">
            {#if m.picture && !failed[m.picture]}
              <img src={m.picture} alt="" loading="lazy" decoding="async" onerror={() => (failed = { ...failed, [m.picture]: true })} />
            {:else}
              <div class="named"><span class="display">{m.name}</span></div>
            {/if}
            {#if m.badge}<span class="badge">{m.badge}</span>{/if}
            {#if m.mine}<span class="mine">In your library</span>{/if}
          </div>
          <div class="text">
            <div class="name">{m.name}</div>
            <div class="msub">{sub(m) || 'Maker unknown'}</div>
            {#if tags(m).length}
              <div class="tags">{#each tags(m).slice(0, 5) as g (g)}<span class="tag">{g}</span>{/each}</div>
            {/if}
          </div>
        </button>
      {/each}
    </div>
    {#if shown.length > limit}<div class="more" bind:this={sentinel}>Showing {limit} of {shown.length.toLocaleString()}…</div>{/if}
    {#if vps.machines.length && !shown.length}
      <div class="empty">No machine here matches. <button type="button" class="btn small" onclick={clearAll}>Clear the filters</button></div>
    {/if}
  </main>

  {#if sel}
    <aside class="panel" style="width: {panelWidth}px" aria-label={sel.name}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="grip" title="Drag to size the panel" onpointerdown={startDrag} onpointermove={drag} onpointerup={endDrag} onpointercancel={endDrag}></div>
      <div class="pbody">
        <div class="ppicture">
          {#if game?.picture && !failed[game.picture]}
            <img src={game.picture} alt="" onerror={() => (failed = { ...failed, [game.picture]: true })} />
          {:else if sel.picture && !failed[sel.picture]}
            <img src={sel.picture} alt="" />
          {:else}
            <div class="named"><span class="display">{sel.name}</span></div>
          {/if}
        </div>
        <h2 class="display">{sel.name}</h2>
        <div class="facts">
          {[sel.maker, sel.year, sel.kind !== sel.maker ? sel.kind : null, sel.players ? `${sel.players} players` : null].filter(Boolean).join(' · ')}
          {#if sel.designers.length}<br />Designed by {sel.designers.join(', ')}{/if}
          {#if sel.theme.length}<br />{sel.theme.join(', ')}{/if}
        </div>
        <div class="pbuttons">
          {#if sel.mine}
            <button type="button" class="btn small" onclick={() => openTable(sel.mine, [sel.mine])}><Icon name="book" size={14} />Open in my library</button>
          {/if}
          {#if game?.link}<button type="button" class="btn small ghost" onclick={() => openLink(game.link)}><Icon name="ext" size={14} />On VPS’s site</button>{/if}
        </div>
        {#if game}
          <VpsFiles kinds={game.kinds} bind:open={openKinds} game={selId} />
        {:else}
          <p class="quiet">Looking…</p>
        {/if}
      </div>
    </aside>
  {/if}
</div>

<style>
  .browser { flex: 1; display: flex; min-height: 0; }
  main { flex: 1; min-width: 0; padding: 22px 28px 44px; display: flex; flex-direction: column; gap: 14px; }
  .head { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 12px; }
  .titles { flex: 1 1 300px; }
  h1 { font-size: 36px; line-height: 1.05; }
  .sub { margin-top: 4px; color: var(--muted); font-size: 13px; min-height: 19px; }
  .controls { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .labelled { display: flex; align-items: center; gap: 8px; color: var(--muted); font-size: 13px; }
  .size input { width: 110px; accent-color: var(--accent); }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 18px; }
  .filters .select { max-width: 220px; }
  .toggle { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .chips { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .has { color: var(--faint); font-size: 13px; margin-right: 2px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile), 1fr)); gap: 18px; }
  .card {
    display: flex; flex-direction: column; padding: 0; text-align: left; color: var(--fg);
    background: var(--card); border: 1px solid var(--card-line); border-radius: 12px; overflow: hidden;
    transition: transform 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
  }
  .card:hover, .card:focus-visible { transform: translateY(-3px); border-color: var(--accent); box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5); }
  .card.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .picture { position: relative; aspect-ratio: 9 / 16; background: var(--picture-bg); }
  .picture img, .ppicture img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .named {
    position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; padding: 14px; text-align: center;
    background: radial-gradient(circle at 50% 35%, #1b2a44 0%, #0f141d 70%);
  }
  .named span { font-size: 24px; line-height: 1.02; text-transform: uppercase; color: #dbe7ff; text-shadow: 0 2px 12px rgba(76, 194, 255, 0.35); }
  .badge, .mine {
    position: absolute; top: 8px; padding: 2px 7px; border-radius: 6px; background: rgba(8, 10, 15, 0.82);
    font-size: 11px; font-weight: 700; letter-spacing: 0.04em;
  }
  .badge { left: 8px; }
  .mine { right: 8px; color: var(--good); }
  .text { padding: 11px 13px 13px; display: flex; flex-direction: column; gap: 4px; }
  .name { font-weight: 600; font-size: 14px; line-height: 1.3; }
  .msub { font-size: 12px; color: var(--muted); }
  .tags { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 4px; }
  .tag { padding: 2px 6px; border-radius: 5px; background: var(--tag-bg); color: var(--tag-fg); font-size: 10.5px; font-weight: 600; }
  .more { padding: 18px; text-align: center; color: var(--faint); font-size: 13px; }
  .empty { display: flex; align-items: center; justify-content: center; gap: 12px; padding: 48px 24px; border: 1px dashed var(--control-line); border-radius: 14px; color: var(--muted); font-size: 15px; }
  .panel {
    position: sticky; top: 61px; align-self: flex-start; height: calc(100vh - 61px); flex: 0 0 auto;
    display: flex; background: var(--side); border-left: 1px solid var(--line);
  }
  .grip { flex: 0 0 6px; cursor: ew-resize; touch-action: none; }
  .grip:hover { background: var(--hover); }
  .pbody { flex: 1; min-width: 0; overflow-y: auto; padding: 18px 20px 28px 14px; display: flex; flex-direction: column; gap: 12px; }
  /* each at its own height, the panel scrolling past them */
  .pbody > * { flex-shrink: 0; }
  .ppicture { position: relative; aspect-ratio: 4 / 3; border-radius: 12px; overflow: hidden; background: var(--picture-bg); }
  h2 { margin: 0; font-size: 28px; line-height: 1.05; }
  .facts { color: var(--fg-soft); font-size: 13px; line-height: 1.6; }
  .pbuttons { display: flex; flex-wrap: wrap; gap: 8px; }
  .quiet { color: var(--muted); margin: 0; }
</style>
