<script>
  // Settings › Library: the library's own choices, kept as they change, as the
  // library's controls keep them — the same preferences, so either place changes both. The order
  // and the groups are changed in the library itself only. One page that
  // scrolls, its parts each under a thin line and their names: the library; the user's playlists —
  // named, ordered and deleted here — and which of the playlists made from VPS's tags show; and the
  // folders, what's
  // done with a file that's there already when files are added, and where each kind of file lives.
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import FolderLayout from './FolderLayout.svelte';
  import { app, savePrefs, savePlaylists, newPlaylist, tagPlaylists, notice, setHidden, answerRatings } from './store.svelte.js';

  function set(name, value) {
    app.prefs[name] = value;
    savePrefs();
  }

  // ratings put in the front ends' lists too, or kept here: switched on, every rating given here
  // goes in them now
  const toFronts = (on) => answerRatings(on);

  const PICTURES = [
    ['playfield', 'Playfield'],
    ['backglass', 'Backglass'],
    ['wheel', 'Wheel'],
  ];
  const VIEWS = [
    ['tiles', 'Tiles'],
    ['list', 'A list'],
  ];
  // what's done with a file that's there already as files are added: asked each time unless chosen
  const ALREADY = [
    ['ask', 'Ask each time', 'Each file there already waits for your choice in the drop window'],
    ['replace', 'Replace it', 'The old one to the Recycle Bin; a pack replaced whole'],
    ['keepBoth', 'Keep both', 'The new one beside it, numbered; a pack’s new files added to it'],
    ['append', 'Append', 'A pack’s new files added to it, one of the same name replaced; a single file replaced'],
    ['skip', 'Leave it', 'Nothing of it copied'],
  ];

  // the playlists made from VPS's tags; those shown, the five biggest until the user chooses
  const tags = $derived(tagPlaylists(app.tables.filter((t) => !t.hiddenTable)));
  const hiddenTables = $derived(app.tables.filter((t) => t.hiddenTable));
  const shown = $derived(app.prefs.tagLists ?? tags.slice(0, 5).map((g) => g.id));

  function showTag(id, on) {
    set('tagLists', on ? [...shown, id] : shown.filter((x) => x !== id));
  }

  let newName = $state('');
  let deleting = $state(null);

  function move(i, by) {
    const list = [...app.playlists];
    const [p] = list.splice(i, 1);
    list.splice(i + by, 0, p);
    app.playlists = list;
    savePlaylists();
  }

  function rename(p, name) {
    p.name = name.trim() || p.name;
    savePlaylists();
  }

  function remove(p) {
    app.playlists = app.playlists.filter((x) => x.id !== p.id);
    if (app.pick === `pl:${p.id}`) app.pick = 'all';
    deleting = null;
    savePlaylists();
  }

  // where each kind of file lives
  let folders = $state(null);
  onMount(async () => {
    folders = await invoke('folders').catch(() => []);
    // opened at a part (Settings › Library › Playlists, from elsewhere in the app)
    if (app.settingsTab) {
      await tick();
      document.getElementById(`lib-${app.settingsTab}`)?.scrollIntoView({ block: 'start' });
      app.settingsTab = null;
    }
  });

  const showFolder = (key) => invoke('show_folder', { key }).catch((e) => notice(String(e), { kind: 'problem' }));
</script>

<div class="page">
  <header>
    <h1 class="display">Library</h1>
    <p>How the library shows your tables, your playlists, and where your tables’ files go. The library’s own controls change the same choices, and each is kept as soon as it changes.</p>
  </header>

  <section class="part" aria-labelledby="lib-library">
    <h2 id="lib-library" class="partname">The library</h2>
    <div class="block">
      <h3 class="section-title">It opens as</h3>
      <div class="row">
        <div class="seg" role="radiogroup" aria-label="It opens as">
          {#each VIEWS as [v, l] (v)}<button type="button" role="radio" aria-checked={app.prefs.view === v} onclick={() => set('view', v)}>{l}</button>{/each}
        </div>
      </div>
    </div>
    <div class="block">
      <h3 class="section-title">Tiles</h3>
      <div class="row">
        <span class="name">What they show</span>
        <div class="seg" role="radiogroup" aria-label="What tiles show">
          {#each PICTURES as [v, l] (v)}<button type="button" role="radio" aria-checked={app.prefs.picture === v} onclick={() => set('picture', v)}>{l}</button>{/each}
        </div>
      </div>
      <div class="row">
        <span class="name">Their size</span>
        <input type="range" min="150" max="380" step="10" value={app.prefs.tile} oninput={(e) => set('tile', Number(e.currentTarget.value))} aria-label="Tile size" />
        <span class="what">{app.prefs.tile} pixels wide, at least; the window fits as many as it can</span>
      </div>
      <div class="preview" style="--tile: {app.prefs.tile}px">
        {#each app.tables.filter((t) => t.picture[app.prefs.picture]).slice(0, 3) as t (t.id)}
          <div class="tile {app.prefs.picture}"><img src={t.picture[app.prefs.picture]} alt="" /><span>{t.name}</span></div>
        {/each}
      </div>
      <label class="check"><input type="checkbox" checked={app.prefs.dropOnTiles !== false} onchange={(e) => set('dropOnTiles', e.currentTarget.checked)} />Files dropped onto a table’s tile are added to that table</label>
      <p class="what">Off, files dropped anywhere in the library open the drop window, which works out which table each is for.</p>
    </div>
    <div class="block">
      <h3 class="section-title">Ratings</h3>
      <p class="what">Give a table one to five stars on its page. Ratings you gave in {app.ratingFronts.length ? app.ratingFronts.join(' and ') : 'PinUP Popper, PinballX or PinballY'} show here until you rate the table here yourself.</p>
      {#if app.ratingFronts.length}
        <label class="check"><input type="checkbox" checked={!!app.prefs.ratingsToFronts} onchange={(e) => toFronts(e.currentTarget.checked)} />Put my ratings in {app.ratingFronts.join(' and ')}’s list too</label>
        <p class="what">On, the stars you give here show there too, and a rating you change there shows here. Off, they stay in this app.</p>
      {/if}
    </div>
  </section>

  <section class="part" aria-labelledby="lib-playlists">
    <h2 id="lib-playlists" class="partname">Playlists</h2>
    <div class="block">
      <h3 class="section-title">Your playlists</h3>
      {#each app.playlists as p, i (p.id)}
        <div class="pl">
          <span class="arrows">
            <button type="button" class="btn small ghost" aria-label="Move {p.name} up" disabled={i === 0} onclick={() => move(i, -1)}><Icon name="chevD" size={14} /></button>
            <button type="button" class="btn small ghost" aria-label="Move {p.name} down" disabled={i === app.playlists.length - 1} onclick={() => move(i, 1)}><Icon name="chevD" size={14} /></button>
          </span>
          <input class="select" value={p.name} aria-label="{p.name}’s name" onchange={(e) => rename(p, e.currentTarget.value)} />
          <span class="what">{p.tables.length} {p.tables.length === 1 ? 'table' : 'tables'}</span>
          {#if deleting === p.id}
            <span class="ask">Delete it? Its tables stay in the library.</span>
            <button type="button" class="btn small danger" onclick={() => remove(p)}>Delete</button>
            <button type="button" class="btn small ghost" onclick={() => (deleting = null)}>Cancel</button>
          {:else}
            <button type="button" class="btn small ghost" onclick={() => (deleting = p.id)}><Icon name="x" size={14} />Delete</button>
          {/if}
        </div>
      {:else}
        <p class="what">None yet. Make one here, or from a table’s page: its … menu, Add to a playlist.</p>
      {/each}
      <form class="row" onsubmit={(e) => { e.preventDefault(); if (newName.trim()) { newPlaylist(newName); newName = ''; } }}>
        <input class="select" bind:value={newName} placeholder="A new playlist’s name" aria-label="A new playlist’s name" />
        <button type="submit" class="btn small"><Icon name="plus" size={14} />New playlist</button>
      </form>
    </div>
    <div class="block">
      <h3 class="section-title">Smart playlists from VPS’s tags</h3>
      <p class="what">Made by themselves from what VPS says of each table: a release’s features (SSF, VR, FastFlips …) and a machine’s themes. Ticked ones show in the library.</p>
      <div class="tagrid">
        {#each tags as g (g.id)}
          <label class="check"><input type="checkbox" checked={shown.includes(g.id)} onchange={(e) => showTag(g.id, e.currentTarget.checked)} />{g.label}<span class="n">{g.list.length}</span><span class="kind">{g.kind === 'feature' ? 'feature' : 'theme'}</span></label>
        {/each}
      </div>
    </div>
    <div class="block">
      <h3 class="section-title">Hidden tables</h3>
      <p class="what">Tables you hide with a right-click on their tile. They show only in the Hidden playlist.</p>
      <label class="check"><input type="checkbox" checked={!!app.prefs.showHidden} onchange={(e) => set('showHidden', e.currentTarget.checked)} />Show the Hidden playlist in the library</label>
      {#each hiddenTables as t (t.id)}
        <div class="pl">
          <span class="hname">{t.name}</span>
          <span class="what">{t.file}</span>
          <button type="button" class="btn small ghost" onclick={() => setHidden(t, false)}>Unhide</button>
        </div>
      {:else}
        <p class="what">None hidden.</p>
      {/each}
    </div>
  </section>

  <section class="part" aria-labelledby="lib-folders">
    <h2 id="lib-folders" class="partname">Folders</h2>
    <div class="block">
      <h3 class="section-title">When a file is there already</h3>
      <p class="what">What the drop window picks when a file you add is already there. You can still pick something else for any file in the drop window, before anything is copied.</p>
      <div class="choices" role="radiogroup" aria-label="When a file is there already">
        {#each ALREADY as [v, l, what] (v)}
          <label class="choice" class:on={(app.prefs.alreadyDefault || 'ask') === v}>
            <input type="radio" name="already" value={v} checked={(app.prefs.alreadyDefault || 'ask') === v} onchange={() => set('alreadyDefault', v)} />
            <span><b>{l}</b><span class="what">{what}</span></span>
          </label>
        {/each}
      </div>
    </div>
    <FolderLayout />
    <div class="block">
      <h3 class="section-title">Where each kind of file lives</h3>
      {#if folders === null}
        <p class="what" data-loading>Looking…</p>
      {:else}
        <div class="folders">
          {#each folders as f (f.key)}
            <div class="folder">
              <span class="flabel">{f.label}</span>
              <span class="fpath" title={f.path}>{f.path}</span>
              {#if f.there}
                <button type="button" class="btn small ghost" onclick={() => showFolder(f.key)}><Icon name="folder" size={14} />Open</button>
              {:else}
                <span class="none">Not made yet</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </section>
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 26px; max-width: 1240px; }
  header h1 { font-size: 38px; line-height: 1; }
  header p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  /* each part under a thin line, with its name (as the side panel's divider) */
  .part { display: flex; flex-direction: column; gap: 20px; padding-top: 22px; border-top: 1px solid var(--line); }
  .partname { margin: 0; font-size: 20px; font-weight: 700; color: var(--fg); }
  .block { display: flex; flex-direction: column; gap: 10px; }
  .block h3 { margin: 0; display: flex; align-items: center; gap: 10px; }
  .row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .name { min-width: 190px; color: var(--fg-soft); font-size: 14px; }
  .what { color: var(--faint); font-size: 13px; margin: 0; }
  input[type='range'] { width: 280px; accent-color: var(--accent); }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 34px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .preview { display: flex; gap: 16px; align-items: flex-start; overflow: hidden; }
  .tile { flex: 0 0 var(--tile); border-radius: 12px; overflow: hidden; background: var(--card); border: 1px solid var(--card-line); }
  .tile img { display: block; width: 100%; object-fit: cover; aspect-ratio: 9 / 16; }
  .tile.backglass { flex-basis: calc(var(--tile) * 1.3); }
  .tile.backglass img { aspect-ratio: 4 / 3; }
  .tile.wheel { flex-basis: calc(var(--tile) * 0.8); }
  .tile.wheel img { aspect-ratio: 1 / 1; object-fit: contain; padding: 10%; }
  .tile span { display: block; padding: 10px 12px; font-size: 14px; font-weight: 600; }
  .pl { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; padding: 6px 8px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .pl input { flex: 0 1 320px; height: 34px; }
  .pl .hname { font-weight: 600; }
  .pl .hname + .what { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .arrows { display: inline-flex; }
  .arrows .btn { width: 30px; padding: 0; }
  .arrows .btn:first-child :global(svg) { transform: rotate(180deg); }
  .ask { color: var(--fg-soft); font-size: 13px; }
  .danger { background: #4a1d26; border-color: #7a2c3a; }
  .tagrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 6px 16px; }
  .check { display: flex; align-items: center; gap: 8px; font-size: 14px; color: var(--fg-soft); padding: 4px 0; }
  .check .n { color: var(--faint); font-size: 12px; }
  .check .kind { margin-left: auto; color: var(--faint); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; }
  .choices { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 8px; }
  .choice { display: flex; align-items: flex-start; gap: 10px; padding: 10px 12px; border-radius: 10px; border: 1px solid var(--card-line); background: var(--card); cursor: pointer; }
  .choice.on { border-color: var(--accent); background: var(--accent-bg); }
  .choice input { margin-top: 3px; }
  .choice > span { display: flex; flex-direction: column; gap: 3px; font-size: 14px; }
  .tobuild { padding: 2px 8px; border-radius: 999px; background: #2a2414; color: #ffd27a; font-size: 11px; font-weight: 600; letter-spacing: 0; text-transform: none; }
  .folders { display: flex; flex-direction: column; }
  .folder { display: grid; grid-template-columns: minmax(200px, 300px) minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 8px 2px; border-top: 1px solid var(--line-soft); font-size: 13px; }
  .flabel { color: var(--fg-soft); }
  .fpath { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
  .none { color: var(--faint); font-size: 12px; }
</style>
