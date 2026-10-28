<script>
  // The library: its groups, smart playlists and "browse by" down the side; the tables as tiles (or
  // a list), with the picture they show, their size, a sort and "Has" filters above them. Needs
  // attention split by kind of problem, the PinUP packs named wrong renamed
  // all at once from there — the kinds on its own screen, each under a thin line, not down the side
  //; the playlists'
  // sections folded and unfolded, remembered.
  import { tick } from 'svelte';
  import Icon from './Icon.svelte';
  import Tile from './Tile.svelte';
  import ListView from './ListView.svelte';
  import {
    app, savePrefs, decade, needsAttention, problems, PROBLEM_KINDS, isVpw, isOriginal, hasTests, groupOf, openTable, renamePacks,
    tagPlaylists, newPlaylist, setHidden, inspect, play,
  } from './store.svelte.js';

  // the tables shown: all but those hidden, which only the Hidden smart playlist shows
  const all = $derived(app.tables);
  const T = $derived(all.filter((t) => !t.hiddenTable));
  const hiddenTables = $derived(all.filter((t) => t.hiddenTable));
  const by = $derived(app.prefs.groupBy);

  // a right-click on a table's tile or row: its menu; Remove asks first, in its own window
  let tableMenu = $state(null);
  function menuOn(e) {
    const id = e.target.closest?.('[data-table]')?.dataset.table;
    const t = id && all.find((x) => x.id === id);
    if (!t) return;
    e.preventDefault();
    tableMenu = { t, x: Math.min(e.clientX, window.innerWidth - 250), y: Math.min(e.clientY, window.innerHeight - 240) };
  }

  // the newest twenty, the newest first
  const recentlyAdded = $derived([...T].sort((a, b) => b.added - a.added).slice(0, 20));
  const played = $derived(T.filter((t) => t.played).sort((a, b) => b.played.last - a.played.last));
  // the tables rated, the most stars first
  const rated = $derived(T.filter((t) => t.rating).sort((a, b) => b.rating - a.rating || a.name.localeCompare(b.name)));

  const libGroups = $derived([
    { id: 'all', label: 'All tables', icon: 'book', list: T },
    { id: 'fav', label: 'Favourites', icon: 'heart', list: T.filter((t) => t.favourite) },
    { id: 'rated', label: 'Top rated', icon: 'star', list: rated },
    { id: 'played', label: 'Recently played', icon: 'clock', list: played },
    { id: 'added', label: 'Recently added', icon: 'inbox', list: recentlyAdded },
    { id: 'attention', label: 'Needs attention', icon: 'alert', list: T.filter(needsAttention) },
  ]);

  // Needs attention's own groups, those with tables
  const attentionAll = $derived(PROBLEM_KINDS.map(([kind, label]) => ({ id: `attention:${kind}`, label, list: T.filter((t) => problems(t).some((p) => p.kind === kind)) })));
  const attention = $derived(attentionAll.filter((g) => g.list.length));
  const misnamed = $derived(T.filter((t) => t.holds.pupMisnamed));
  let renaming = $state(false);

  // the smart playlists: made from what the tables are, and from VPS's tags,
  // those shown (Settings › Library › Playlists; the five biggest until the user chooses)
  const tags = $derived(tagPlaylists(T));
  const shownTags = $derived(app.prefs.tagLists ? tags.filter((g) => app.prefs.tagLists.includes(g.id)) : tags.slice(0, 5));
  const smart = $derived([
    { id: 'vpw', label: 'VPin Workshop’s', list: T.filter(isVpw) },
    { id: 'originals', label: 'Originals', list: T.filter(isOriginal) },
    { id: 'nineties', label: 'The 1990s', list: T.filter((t) => decade(t.year) === '1990s') },
    ...shownTags,
    // the tables hidden, once turned on in Settings › Library › Playlists
    ...(app.prefs.showHidden ? [{ id: 'hidden', label: 'Hidden', list: hiddenTables }] : []),
  ]);
  // the user's own, in their order
  const mine = $derived(app.playlists.map((p) => ({ id: `pl:${p.id}`, label: p.name, list: p.tables.map((k) => T.find((t) => t.key === k)).filter(Boolean) })));
  let naming = $state(false);
  let newName = $state('');

  // the side's sections folded away, by name, as the user left them
  const folded = $derived(app.prefs.folded || []);
  function fold(name) {
    setPref('folded', folded.includes(name) ? folded.filter((n) => n !== name) : [...folded, name]);
  }

  function makePlaylist() {
    if (newName.trim()) pick(`pl:${newPlaylist(newName)}`);
    naming = false;
    newName = '';
  }

  const groups = $derived.by(() => {
    const counts = new Map();
    for (const t of T) {
      const g = groupOf(t, by);
      counts.set(g, (counts.get(g) || 0) + 1);
    }
    const last = (g) => g === 'Not on VPS' || g.endsWith('unknown');
    return [...counts].sort((a, b) => (last(a[0]) - last(b[0])) || (by === 'decade' ? a[0].localeCompare(b[0]) : b[1] - a[1] || a[0].localeCompare(b[0])));
  });

  const mark = (label) => (label === 'Not on VPS' ? '?' : label.replace(/[^A-Za-z0-9 ]/g, '').split(' ').filter(Boolean).map((w) => w[0]).join('').slice(0, 2).toUpperCase());

  const chosen = $derived.by(() => {
    const p = app.pick;
    const lib = libGroups.find((g) => g.id === p);
    if (lib) return { title: lib.label, list: lib.list, ordered: p === 'played' || p === 'added' || p === 'rated' };
    const a = attentionAll.find((g) => g.id === p);
    if (a) return { title: a.label, list: a.list, ordered: false };
    const s = smart.find((g) => g.id === p) || tags.find((g) => g.id === p);
    if (s) return { title: s.label, list: s.list, ordered: false };
    const m = mine.find((g) => g.id === p);
    if (m) return { title: m.label, list: m.list, ordered: true };
    if (p.startsWith('g:')) {
      const g = p.slice(2);
      return { title: g, list: T.filter((t) => groupOf(t, by) === g), ordered: false };
    }
    return { title: 'All tables', list: T, ordered: false };
  });

  const empties = {
    fav: 'Tables you mark with a heart gather here.',
    rated: 'Tables you give stars to on their pages gather here, the most stars first.',
    played: 'The tables you play show up here, the latest first.',
    added: 'Tables you add show up here, the newest first.',
    attention: 'Nothing needs attention: every table has what it asks for.',
    'attention:match': 'Every table’s VPS match is confirmed.',
    'attention:missing': 'No table is missing a file it asks for.',
    'attention:wrong': 'Every file checked is as it should be.',
    'attention:test': 'No table failed its test.',
    'attention:read': 'Every table’s file reads.',
  };

  /** A list as the screen shows it: searched, filtered by "Has", sorted. */
  function arrange(list, ordered) {
    const q = app.query.trim().toLowerCase();
    if (q) {
      list = list.filter((t) => [t.name, t.file, t.maker, t.rom, ...t.authors, ...t.designers].some((s) => (s || '').toLowerCase().includes(q)));
    }
    for (const k of app.has) list = list.filter(hasTests[k]);
    if (ordered && app.prefs.sort === 'name') return list;
    const s = app.prefs.sort;
    return [...list].sort((a, b) =>
      s === 'year' ? (a.year || 9999) - (b.year || 9999) || a.name.localeCompare(b.name)
      : s === 'maker' ? (a.maker || '~').localeCompare(b.maker || '~') || a.name.localeCompare(b.name)
      : s === 'added' ? b.added - a.added
      : s === 'rating' ? (b.rating || 0) - (a.rating || 0) || a.name.localeCompare(b.name)
      : a.name.localeCompare(b.name) || a.file.localeCompare(b.file),
    );
  }

  const shown = $derived(arrange(chosen.list, chosen.ordered));

  // a table just added shown once the library has it — all tables shown if the group or a filter
  // leaves it out — scrolled to and lit for a moment
  $effect(() => {
    const r = app.reveal;
    if (!r) return;
    if (Date.now() > r.until) {
      app.reveal = null;
      return;
    }
    const id = r.ids.find((x) => T.some((t) => t.id === x));
    if (!id) return;
    if (!shown.some((t) => t.id === id)) {
      app.pick = 'all';
      app.query = '';
      app.has = [];
      return;
    }
    app.reveal = null;
    tick().then(() => {
      document.querySelector(`[data-table="${id}"]`)?.scrollIntoView({ block: 'center', behavior: 'smooth' });
      app.glow = id;
      setTimeout(() => app.glow === id && (app.glow = null), 4200);
    });
  });
  // Needs attention, by kind: each kind with tables, in its own part of the screen
  const attentionShown = $derived(app.pick === 'attention' ? attention.map((g) => ({ ...g, shown: arrange(g.list, false) })).filter((g) => g.shown.length) : []);

  const emptyText = $derived(
    shown.length ? '' : chosen.list.length ? 'No table here matches. Clear a filter, or search for something else.' : empties[app.pick] || (app.pick.startsWith('pl:') ? 'Nothing in it yet: add tables from a table’s page, its … menu, Add to a playlist.' : 'No tables here.'),
  );

  const chips = [
    { id: 'b2s', label: 'Backglass', icon: 'image' },
    { id: 'rom', label: 'ROM', icon: 'chip' },
    { id: 'pup', label: 'PinUP pack', icon: 'layers' },
    { id: 'alt', label: 'AltSound', icon: 'music' },
    { id: 'colour', label: 'Color DMD', icon: 'palette' },
  ];

  const pictures = [
    { id: 'playfield', label: 'Playfield' },
    { id: 'backglass', label: 'Backglass' },
    { id: 'wheel', label: 'Wheel' },
  ];

  function pick(id) {
    app.pick = id;
    app.query = '';
  }

  function toggleHas(id) {
    app.has = app.has.includes(id) ? app.has.filter((k) => k !== id) : [...app.has, id];
  }

  function setPref(name, value) {
    app.prefs[name] = value;
    savePrefs();
  }

  const status = $derived(app.status);
  const reading = $derived(status.phase === 'reading' || status.phase === 'vps' || status.phase === 'starting');
</script>

<div class="library">
  <aside aria-label="Library groups">
    <section>
      <h2 class="section-title">Library</h2>
      {#each libGroups as g (g.id)}
        <button type="button" class="row" aria-pressed={app.pick === g.id} onclick={() => pick(g.id)}>
          <Icon name={g.icon} size={17} />
          <span class="label">{g.label}</span>
          <span class="count">{g.list.length || ''}</span>
        </button>
      {/each}
    </section>
    <section>
      <h2 class="section-title"><button type="button" class="fold" aria-expanded={!folded.includes('smart')} onclick={() => fold('smart')}><Icon name={folded.includes('smart') ? 'chevR' : 'chevD'} size={13} />Smart playlists</button></h2>
      {#if !folded.includes('smart')}
      {#each smart as g (g.id)}
        <button type="button" class="row" aria-pressed={app.pick === g.id} onclick={() => pick(g.id)}>
          <Icon name={g.id.startsWith('tag:') ? 'star' : 'list'} size={17} />
          <span class="label">{g.label}</span>
          <span class="count">{g.list.length || ''}</span>
        </button>
      {/each}
      {/if}
    </section>
    <section>
      <h2 class="section-title"><button type="button" class="fold" aria-expanded={!folded.includes('mine')} onclick={() => fold('mine')}><Icon name={folded.includes('mine') ? 'chevR' : 'chevD'} size={13} />Your playlists</button></h2>
      {#if !folded.includes('mine')}
      {#each mine as g (g.id)}
        <button type="button" class="row" aria-pressed={app.pick === g.id} onclick={() => pick(g.id)}>
          <Icon name="heart" size={17} />
          <span class="label">{g.label}</span>
          <span class="count">{g.list.length || ''}</span>
        </button>
      {/each}
      {#if naming}
        <form class="newpl" onsubmit={(e) => { e.preventDefault(); makePlaylist(); }}>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="select" bind:value={newName} placeholder="Its name" aria-label="The new playlist’s name" autofocus onkeydown={(e) => e.key === 'Escape' && (naming = false)} />
          <button type="submit" class="btn small">Make</button>
        </form>
      {:else}
        <button type="button" class="row soft" onclick={() => (naming = true)}>
          <Icon name="plus" size={17} />
          <span class="label">New playlist</span>
        </button>
      {/if}
      {/if}
    </section>
    <section>
      <div class="by">
        <h2 class="section-title">Browse by</h2>
        <select class="select" aria-label="Browse by" value={by} onchange={(e) => { setPref('groupBy', e.currentTarget.value); app.pick = 'all'; }}>
          <option value="maker">Maker</option>
          <option value="decade">Decade</option>
          <option value="kind">Type</option>
        </select>
      </div>
      {#each groups as [label, count] (label)}
        <button type="button" class="row" aria-pressed={app.pick === `g:${label}`} onclick={() => pick(`g:${label}`)}>
          <span class="mark" aria-hidden="true">{mark(label)}</span>
          <span class="label">{label}</span>
          <span class="count">{count}</span>
        </button>
      {/each}
    </section>
  </aside>

  <main oncontextmenu={menuOn}>
    {#if reading}
      <div class="reading" role="status">
        {#if status.phase === 'vps'}
          Fetching VPS’s list of tables, the first time only…
        {:else if status.phase === 'reading'}
          Reading your tables: {status.done + 1} of {status.of}{status.current ? ` · ${status.current}` : ''}
        {:else}
          Opening the library…
        {/if}
        {#if status.of}<span class="bar"><span style="width: {(100 * status.done) / status.of}%"></span></span>{/if}
      </div>
    {/if}
    {#if status.problem}
      <div class="problem" role="status">{status.problem}</div>
    {/if}

    <div class="head">
      <div class="titles">
        <h1 class="display">{chosen.title}</h1>
        <div class="sub">
          {#if app.loaded && shown.length}
            {shown.length === T.length ? (T.length === 1 ? '1 table' : `${T.length} tables`) : `${shown.length} of ${T.length} tables`}{app.query ? ` matching “${app.query}”` : ''}
          {/if}
        </div>
      </div>
      <div class="controls">
        <label class="labelled">Sort
          <select class="select" value={app.prefs.sort} onchange={(e) => setPref('sort', e.currentTarget.value)}>
            <option value="name">Name</option>
            <option value="year">Year</option>
            <option value="maker">Maker</option>
            <option value="added">Recently added</option>
            <option value="rating">Rating</option>
          </select>
        </label>
        {#if app.prefs.view === 'tiles'}
          <div class="switch" role="group" aria-label="Pictures">
            {#each pictures as p (p.id)}
              <button type="button" aria-pressed={app.prefs.picture === p.id} onclick={() => setPref('picture', p.id)}>{p.label}</button>
            {/each}
          </div>
          <label class="labelled size">Size
            <input type="range" min="150" max="380" step="10" value={app.prefs.tile} oninput={(e) => setPref('tile', Number(e.currentTarget.value))} aria-label="Tile size" />
          </label>
        {/if}
        <div class="switch icons" role="group" aria-label="View">
          <button type="button" aria-pressed={app.prefs.view === 'tiles'} aria-label="Tiles" title="Tiles" onclick={() => setPref('view', 'tiles')}><Icon name="grid" size={16} /></button>
          <button type="button" aria-pressed={app.prefs.view === 'list'} aria-label="List" title="List" onclick={() => setPref('view', 'list')}><Icon name="list" size={16} /></button>
        </div>
      </div>
    </div>

    {#snippet fixAll()}
      <div class="fixall" role="status">
        <Icon name="layers" size={18} />
        {#if renaming}
          <span>Rename {misnamed.length === 1 ? 'the PinUP pack' : `the ${misnamed.length} PinUP packs`} to the names their tables’ scripts ask for? Each stays where it is; PinUP Popper finds them by the same names.</span>
          <button type="button" class="btn small" onclick={() => { renaming = false; renamePacks(misnamed.map((t) => t.id)); }}>Rename {misnamed.length === 1 ? 'it' : 'them'}</button>
          <button type="button" class="btn small ghost" onclick={() => (renaming = false)}>Cancel</button>
        {:else}
          <span>{misnamed.length === 1 ? 'One table’s PinUP pack is' : `${misnamed.length} tables’ PinUP packs are`} there but named so the player won’t find {misnamed.length === 1 ? 'it' : 'them'}.</span>
          <button type="button" class="btn small" onclick={() => (renaming = true)}>Rename all</button>
        {/if}
      </div>
    {/snippet}
    {#if app.pick === 'attention:wrong' && misnamed.length}
      {@render fixAll()}
    {/if}

    <div class="chips">
      <span class="has">Has</span>
      {#each chips as c (c.id)}
        <button type="button" class="chip" aria-pressed={app.has.includes(c.id)} onclick={() => toggleHas(c.id)}><Icon name={c.icon} size={14} />{c.label}</button>
      {/each}
    </div>

    {#snippet tables(list)}
      {#if app.prefs.view === 'list'}
        <ListView tables={list} open={(t) => openTable(t.id, list.map((x) => x.id))} />
      {:else}
        <div class="grid {app.prefs.picture}" style="--tile: {app.prefs.tile}px">
          {#each list as t (t.id)}
            <Tile {t} picture={app.prefs.picture} onopen={() => openTable(t.id, list.map((x) => x.id))} />
          {/each}
        </div>
      {/if}
    {/snippet}
    {#if app.pick === 'attention'}
      {#each attentionShown as g (g.id)}
        <section class="kind" aria-labelledby="k-{g.id}">
          <h2 id="k-{g.id}" class="kindname">{g.label} <span class="n">{g.shown.length}</span></h2>
          {#if g.id === 'attention:wrong' && misnamed.length}{@render fixAll()}{/if}
          {@render tables(g.shown)}
        </section>
      {/each}
    {:else}
      {@render tables(shown)}
    {/if}
    {#if emptyText && app.loaded}
      <div class="empty">{emptyText}</div>
    {/if}
  </main>
</div>

<!-- a table's menu closed by a click anywhere, Escape or scrolling -->
<svelte:window onclick={() => (tableMenu = null)} onkeydown={(e) => e.key === 'Escape' && (tableMenu = null)} onwheel={() => (tableMenu = null)} />

{#if tableMenu}
  <div class="ctx" role="menu" style="left: {tableMenu.x}px; top: {tableMenu.y}px">
    <button type="button" role="menuitem" onclick={() => openTable(tableMenu.t.id, shown.map((x) => x.id))}>Open its page</button>
    <button type="button" role="menuitem" onclick={() => { const id = tableMenu.t.id; tableMenu = null; inspect(id); }}>Open in the table editor</button>
    <button type="button" role="menuitem" onclick={() => { const t = tableMenu.t; tableMenu = null; play(t); }}>Play</button>
    <button type="button" role="menuitem" title="As its author made it: your table settings, script edits and physics changes left out for this game" onclick={() => { const t = tableMenu.t; tableMenu = null; play(t, null, 'hardcore'); }}>Play in Hardcore</button>
    {#if tableMenu.t.hiddenTable}
      <button type="button" role="menuitem" onclick={() => setHidden(tableMenu.t, false)}>Unhide</button>
    {:else}
      <button type="button" role="menuitem" onclick={() => setHidden(tableMenu.t, true)}>Hide</button>
    {/if}
    <button type="button" role="menuitem" class="bad" onclick={() => (app.removing = tableMenu.t)}>Remove from library…</button>
  </div>
{/if}

<style>
  .library { flex: 1; display: flex; min-height: 0; }
  .ctx {
    position: fixed; z-index: 40; width: 240px; padding: 6px; border-radius: 12px; background: #151a24; border: 1px solid var(--control-line);
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55); display: flex; flex-direction: column; gap: 2px;
  }
  .ctx > button { padding: 9px 12px; border: 0; border-radius: 8px; background: transparent; text-align: left; color: var(--fg); }
  .ctx > button:hover { background: var(--selected); }
  .ctx > button.bad { color: #f0a8a8; }
  aside {
    flex: 0 0 248px; padding: 18px 12px 28px; background: var(--side); border-right: 1px solid var(--line);
    display: flex; flex-direction: column; gap: 20px; position: sticky; top: 61px; align-self: flex-start;
    height: calc(100vh - 61px); overflow-y: auto;
  }
  section { display: flex; flex-direction: column; gap: 2px; }
  section > .section-title { margin: 0 0 6px 10px; }
  .by { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 0 4px 6px 10px; }
  .by .select { height: 30px; }
  .row {
    display: flex; align-items: center; gap: 10px; width: 100%; padding: 8px 10px; border: 0; border-radius: 8px;
    background: transparent; color: var(--fg-soft); font-size: 14px; text-align: left;
  }
  .row:hover { background: var(--hover); }
  .row[aria-pressed='true'] { background: var(--selected); color: #fff; font-weight: 600; }
  .row.soft { color: var(--faint); }
  .fold { display: inline-flex; align-items: center; gap: 6px; margin-left: -4px; padding: 2px 4px; border: 0; border-radius: 6px; background: none; color: inherit; font: inherit; letter-spacing: inherit; text-transform: inherit; cursor: pointer; }
  .fold:hover { color: var(--fg-soft); background: var(--hover); }
  /* Needs attention's kinds, each under a thin line, as the side's own */
  .kind { display: flex; flex-direction: column; gap: 14px; padding-top: 16px; border-top: 1px solid var(--line); }
  .kind:first-of-type { padding-top: 4px; border-top: 0; }
  .kindname { margin: 0; font-size: 15px; font-weight: 700; color: var(--fg-soft); }
  .kindname .n { margin-left: 6px; font-size: 13px; font-weight: 500; color: var(--faint); }
  .newpl { display: flex; gap: 6px; padding: 4px 4px 4px 10px; }
  .newpl input { flex: 1; min-width: 0; height: 32px; }
  .fixall {
    display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 10px 14px; border-radius: 10px;
    background: #2a2414; border: 1px solid #5a4a20; color: #ffe2b0; font-size: 13px;
  }
  .fixall span { flex: 1 1 300px; }
  .row .label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row .count { font-size: 12px; color: var(--faint); font-weight: 400; }
  .mark {
    display: inline-flex; align-items: center; justify-content: center; flex: 0 0 22px; height: 22px; border-radius: 6px;
    background: var(--mark-bg); color: var(--mark-fg); font-size: 10px; font-weight: 700;
  }
  main { flex: 1; min-width: 0; padding: 22px 28px 44px; display: flex; flex-direction: column; gap: 16px; }
  .reading, .problem {
    display: flex; flex-wrap: wrap; align-items: center; gap: 12px; padding: 10px 14px; border-radius: 10px;
    background: #12233a; border: 1px solid #23476e; color: #d6e9ff; font-size: 13px;
  }
  .problem { background: #2a2214; border-color: #5a4422; color: #ffe2b0; }
  .bar { flex: 1 1 160px; height: 6px; border-radius: 999px; background: #1d3554; overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); transition: width 0.3s ease; }
  .head { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 12px; }
  .titles { flex: 1 1 300px; }
  h1 { font-size: 36px; line-height: 1.05; }
  .sub { margin-top: 4px; color: var(--muted); font-size: 13px; min-height: 19px; }
  .controls { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .labelled { display: flex; align-items: center; gap: 8px; color: var(--muted); font-size: 13px; }
  .size input { width: 110px; accent-color: var(--accent); }
  .switch { display: flex; padding: 3px; border-radius: 9px; background: var(--control); border: 1px solid var(--control-line); }
  .switch button { height: 28px; padding: 0 10px; border: 0; border-radius: 6px; background: transparent; color: var(--muted); font-size: 13px; }
  .switch.icons button { width: 36px; padding: 0; display: inline-flex; align-items: center; justify-content: center; }
  .switch button[aria-pressed='true'] { background: #263045; color: var(--fg); font-weight: 600; }
  .chips { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .has { color: var(--faint); font-size: 13px; margin-right: 2px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile), 1fr)); gap: 20px; }
  .grid.backglass { grid-template-columns: repeat(auto-fill, minmax(calc(var(--tile) * 1.3), 1fr)); }
  .grid.wheel { grid-template-columns: repeat(auto-fill, minmax(calc(var(--tile) * 0.8), 1fr)); }
  .empty { padding: 48px 24px; border: 1px dashed var(--control-line); border-radius: 14px; text-align: center; color: var(--muted); font-size: 15px; }
</style>
