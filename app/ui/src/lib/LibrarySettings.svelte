<script>
  // Settings › Library: the library's own choices, kept as they change, as the
  // library's controls keep them — the same preferences, so either place changes both. The order
  // and the groups are changed in the library itself only. One page that
  // scrolls, its parts each under a thin line and their names: the library; the user's playlists —
  // named, ordered and deleted here — and which of the playlists made from VPS's tags show; and the
  // folders, what's
  // done with a file that's there already when files are added, and where each kind of file lives.
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import FolderLayout from './FolderLayout.svelte';
  import ForTag from './ForTag.svelte';
  import WebPanel from './WebPanel.svelte';
  import { app, savePrefs, savePlaylists, newPlaylist, tagPlaylists, notice, setHidden, answerRatings, openWeb, closeWeb, vps as browser } from './store.svelte.js';

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
  const BUILDS = [
    ['combined', 'One tile for each table', 'A table shows once, however many table files you have for it. Its page has a tab for each — its builds — and follows the one you pick: its files, what PLAY plays, your plays and its achievements.'],
    ['each', 'Each table file on its own', 'Every table file has its own tile and page.'],
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
  const tags = $derived(tagPlaylists(app.tables.filter((t) => !t.hiddenTable)).filter((g) => g.kind !== 'format'));
  const hiddenTables = $derived(app.tables.filter((t) => t.hiddenTable));
  const shown = $derived(app.prefs.tagLists ?? tags.slice(0, 5).map((g) => g.id));
  // listed by name, A to Z
  const byName = $derived([...tags].sort((a, b) => a.label.localeCompare(b.label, undefined, { sensitivity: 'base' })));

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

  // the VPX the library plays its tables in, its folder chosen, VPX 10.8.1 got, the one before put back
  let vpx = $state(null);
  let addons = $state([]);
  let toBring = $state(0);
  const readVpx = async () => {
    vpx = await invoke('vpx_status').catch(() => ({}));
    app.vpxOlder = !!vpx?.found && !vpx.found.addons;
    app.vpxInstalled = vpx?.installed || [];
    app.vpxMadeForLine = vpx?.madeForLine || app.vpxMadeForLine;
    addons = await invoke('addons').catch(() => []);
    toBring = await invoke('vpx_settings_to_bring').catch(() => 0);
  };
  // the lines found, the one that plays, and whether a newer VPX (10.8.1 or later) is among them
  const lines = $derived(vpx?.installed || []);
  const hasNewer = $derived(lines.some((f) => f.addons));
  // the newer line: the one here, else the one Get VPX brings
  const newerLine = $derived(lines.find((f) => f.addons)?.line ?? vpx?.madeForLine ?? app.vpxMadeForLine);
  const compare = (choosing = null) => (app.vpxChoice = { choosing, current: vpx?.found?.line ?? null, onchosen: () => readVpx() });
  const bringSettings = () =>
    invoke('vpx_bring_settings')
      .then((n) => {
        notice(`${n === 1 ? '1 setting' : `${n} settings`} brought to VPX. VPX’s file before is kept beside it.`, { kind: 'done' });
        readVpx();
      })
      .catch((e) => notice(String(e), { kind: 'problem' }));
  async function setAddon(a, own) {
    try {
      await invoke('addon_set', { id: a.id, own });
      addons = await invoke('addons');
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  onMount(() => {
    readVpx();
    const off = listen('vpx-changed', (e) => {
      if (typeof e.payload === 'string') notice(e.payload, { kind: 'problem' });
      readVpx();
    });
    return () => off.then((u) => u());
  });
  async function chooseVpx() {
    const folder = await invoke('pick_folder', { title: 'VPX’s folder', start: vpx?.found?.program ?? null }).catch((e) => notice(String(e), { kind: 'problem' }));
    if (!folder) return;
    try {
      await invoke('vpx_set_folder', { folder });
      notice('VPX’s folder kept.', { kind: 'done' });
      readVpx();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  const getVpx = () => invoke('vpx_get').then(() => notice(`Getting VPX ${vpx?.madeFor ?? app.vpxMadeForLine}: the tasks bar shows how it’s going.`, { kind: 'info' })).catch((e) => notice(String(e), { kind: 'problem' }));
  const putBackVpx = () => invoke('vpx_put_back').then(() => { notice('The VPX before is back.', { kind: 'done' }); readVpx(); }).catch((e) => notice(String(e), { kind: 'problem' }));

  // Future Pinball as the app finds it — beside VPX's folder, where its installer put it, or where
  // the settings name it — and where it isn't found, its makers' download pages in the app's
  // browser (BAM's has the installer of both), looked for again once it's installed; Windows'
  // fullscreen optimizations off for its games where chosen (its guides' advice against stutters)
  const FP_SITE = 'https://futurepinball.com/';
  const BAM_DOWNLOADS = 'https://www.ravarcade.pl/download';
  let fp = $state(null);
  let fpProblem = $state('');
  async function readFp() {
    try {
      fp = await invoke('fp_state');
      app.fp = fp;
      fpProblem = '';
    } catch (e) {
      fpProblem = String(e);
    }
  }
  onMount(readFp);
  onDestroy(() => {
    if (browser.web?.from === 'fp') closeWeb();
  });
  async function setOptimizations(off) {
    try {
      await invoke('fp_set_no_optimizations', { on: off });
      fp = { ...fp, noOptimizations: off };
      notice(off ? 'Windows’ fullscreen optimizations are off for Future Pinball’s games from the next one.' : 'Windows’ fullscreen optimizations stay on for Future Pinball’s games.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
  let innerWidth = $state(1600);
  const drawer = $derived(Math.round(Math.min(760, Math.max(420, innerWidth * 0.45))));
</script>

<div class="page">
  <header>
    <h1 class="display">Library<ForTag which="both" /></h1>
    <p>How the library shows your tables, your playlists, and where your tables’ files go. The library’s own controls change the same choices, and each is kept as soon as it changes.</p>
  </header>

  <section class="part" aria-labelledby="lib-vpx">
    <h2 id="lib-vpx" class="partname">VPX<ForTag which="vpx" /></h2>
    <div class="block">
      <h3 class="section-title">What plays your tables</h3>
      {#if vpx === null}
        <p class="what" data-loading>Looking…</p>
      {:else if lines.length > 1}
        <!-- both lines beside each other: which plays, switched only once the window comparing them says what changes -->
        <div class="row">
          <div class="seg" role="radiogroup" aria-label="Which VPX plays your tables">
            {#each lines as f (f.line)}
              <button type="button" role="radio" aria-checked={vpx.found?.line === f.line} title={f.program} onclick={() => vpx.found?.line !== f.line && compare(f.line)}>VPX {f.version}</button>
            {/each}
          </div>
          <button type="button" class="btn small ghost" onclick={() => compare()}><Icon name="help" size={14} />How they differ</button>
        </div>
        <p class="what">{vpx.found?.addons ? `VPX ${vpx.found.line} plays your tables, with its own add-ons and achievements as you play.` : 'VPX 10.8.0 plays your tables, with the classic programs; achievements are counted after each game, and it keeps its own settings.'} Choosing the other shows what changes first.</p>
      {:else if vpx.found}
        <div class="folder">
          <span class="flabel">VPX {vpx.found.version}</span>
          <span class="fpath" title={vpx.found.program}>{vpx.found.program}</span>
          {#if vpx.found.plays}<span class="ok">Plays your tables</span>{:else}<span class="none">Tables play in VPX 10.8.0 or newer</span>{/if}
        </div>
      {:else}
        <p class="what">No VPX was found beside your tables. Choose its folder, or get VPX {app.vpxMadeForLine}.</p>
      {/if}
      {#if vpx}
        <div class="row">
          <button type="button" class="btn small" onclick={chooseVpx}><Icon name="folder" size={14} />Choose VPX’s folder…</button>
          {#if !hasNewer || vpx.older}<button type="button" class="btn small" onclick={getVpx}>{hasNewer ? `Get VPX ${vpx.madeFor}` : `Get VPX ${vpx.madeForLine ?? app.vpxMadeForLine}`}</button>{/if}
          {#if lines.length <= 1}<button type="button" class="btn small ghost" onclick={() => compare()}><Icon name="help" size={14} />VPX {newerLine} or 10.8.0?</button>{/if}
          {#if vpx.putBack}<button type="button" class="btn small ghost" title={vpx.putBack === 'none' ? 'Takes out what Get VPX brought, to the Recycle Bin: the folder as it was.' : `Takes out what Get VPX brought, to the Recycle Bin, and puts back the files of VPX ${vpx.putBack} it replaced.`} onclick={putBackVpx}>Undo Get VPX</button>{/if}
        </div>
        {#if (!hasNewer || vpx.older) && vpx.getInto}
          <p class="what">{vpx.older ? `Your VPX ${newerLine} is older than the build this app is made for, VPX ${vpx.madeFor}. ` : ''}Get VPX downloads VPX {vpx.madeFor}, the build this app is made for: one of VPX’s automatic builds of its newest code, as VPX’s team hasn’t released it yet. It checks it against the checksum GitHub keeps of it and puts it in {vpx.getInto}. Each file it replaces is kept, so the VPX before can be put back{vpx.found?.plays && !hasNewer ? '; VPX 10.8.0 stays, and you choose which plays your tables' : ''}.</p>
        {/if}
      {/if}
    </div>
    {#if vpx?.found?.addons && toBring}
      <div class="block">
        <h3 class="section-title">Your settings</h3>
        <p class="what">{toBring === 1 ? '1 setting' : `${toBring} settings`} you made in this app — your screens, sound, controls, picture — {toBring === 1 ? 'isn’t' : 'aren’t'} in VPX’s settings yet. Bring them over once, and VPX plays with them. VPX’s file before is kept beside it.</p>
        <div class="row"><button type="button" class="btn small" onclick={bringSettings}>Bring them to VPX</button></div>
      </div>
    {/if}
    {#if vpx?.found?.plays && !vpx.found.addons}
      <div class="block">
        <h3 class="section-title">Its add-ons</h3>
        <p class="what">VPX 10.8.0 takes no add-ons: it plays with the classic programs installed — VPinMAME, B2S Server, PinUP Player, FlexDMD, DOF — each set in its own settings. Achievements are counted once a game is over.</p>
      </div>
    {/if}
    {#if vpx?.found?.addons && addons.length}
      <div class="block">
        <h3 class="section-title">Its add-ons</h3>
        <p class="what">VPX {vpx.found.line} carries its own add-ons. Each part uses VPX’s own, or the classic program installed for it.</p>
        {#each addons as a (a.id)}
          <div class="row">
            <span class="name">{a.name}</span>
            <div class="seg" role="radiogroup" aria-label={a.name}>
              <button type="button" role="radio" aria-checked={a.own} onclick={() => setAddon(a, true)}>VPX’s own</button>
              {#if a.classic}
                <button type="button" role="radio" aria-checked={!a.own} disabled={!a.classicInstalled && a.own} title={a.classicInstalled ? '' : `${a.classic} isn’t installed`} onclick={() => setAddon(a, false)}>{a.classic}</button>
              {:else}
                <button type="button" role="radio" aria-checked={!a.own} onclick={() => setAddon(a, false)}>Off</button>
              {/if}
            </div>
            {#if a.classic && !a.classicInstalled}<span class="what">{a.classic} isn’t installed</span>{/if}
          </div>
        {/each}
        <p class="what">Achievements are followed as you play by an add-on of VPI’s own, put in VPX’s plugins folder (vpi-achievements) the first time a table with achievements plays: it hears the game from VPX’s own PinMAME and B2S add-ons, and VPX shows each one earned. It does nothing unless VPI starts the game. With the classic programs, only scores and games count, once the game is over.</p>
      </div>
    {/if}
  </section>

  <section class="part" aria-labelledby="lib-fp">
    <h2 id="lib-fp" class="partname">Future Pinball<ForTag which="fp" /></h2>
    <div class="block">
      <h3 class="section-title">Where it is</h3>
      {#if fpProblem}
        <p class="what">{fpProblem}</p>
      {:else if !fp}
        <p class="what" data-loading>Looking…</p>
      {:else if fp.folder}
        <div class="folder">
          <span class="flabel">Future Pinball</span>
          <span class="fpath" title={fp.folder}>{fp.folder}</span>
          {#if fp.bam}<span class="ok">With BAM</span>{:else}<span class="none">Without BAM</span>{/if}
        </div>
        <p class="what">{fp.tables === 1 ? 'Its table is' : `Its ${fp.tables} tables are`} in your library beside VPX’s, tagged FP, each played in Future Pinball{fp.bam ? ' through BAM, as other front ends play them' : ''}. The pages tagged VPX + FP set its games too: Screens, Sound’s outputs, Controls. BAM’s own options are in its menu during a game: press Q or ~.</p>
      {:else}
        <p class="what">Future Pinball isn’t found on this PC: the app looks beside VPX’s folder (as the Baller Installer puts it), where its installer puts it, and where the settings name it. Its tables play in Future Pinball itself, the app their front end; it isn’t part of the app.</p>
        <div class="row">
          <button type="button" class="btn small" onclick={() => openWeb(BAM_DOWNLOADS, null, 'fp')}><Icon name="download" size={14} />Get Future Pinball with BAM</button>
          <button type="button" class="btn small ghost" onclick={() => openWeb(FP_SITE, null, 'fp')}>Future Pinball’s site</button>
          <button type="button" class="btn small ghost" onclick={readFp}><Icon name="refresh" size={14} />Look again</button>
        </div>
        <p class="what">Download its installer (Future Pinball with BAM): it lands in Downloads › Visual Pinball Infinite. Run it from there, then Look again. Future Pinball and BAM are their makers’ own, free, installed by you.</p>
      {/if}
    </div>
    {#if fp?.folder}
      <div class="block">
        <h3 class="section-title">Its games</h3>
        <label class="fpcheck">
          <input type="checkbox" checked={!!fp.noOptimizations} onchange={(e) => setOptimizations(e.currentTarget.checked)} />
          <span><b>Turn off fullscreen optimizations</b><span class="what">Windows’ own compatibility setting, for Future Pinball and BAM’s loader, during its games. Future Pinball’s guides set it against stutters in its full screen; leave it off unless you see them.</span></span>
        </label>
      </div>
    {/if}
  </section>

  <section class="part" aria-labelledby="lib-library">
    <h2 id="lib-library" class="partname">The library<ForTag which="both" /></h2>
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
      <h3 class="section-title">Tables with more than one table file</h3>
      <p class="what">Most tables have one table file. For those with more — another release, a mod, a copy — how the library shows them. Table files are one table’s where VPS matches them to the same machine.</p>
      <div class="choices" role="radiogroup" aria-label="Tables with more than one table file">
        {#each BUILDS as [v, l, what] (v)}
          <label class="choice" class:on={(app.prefs.tableBuilds || 'each') === v}>
            <input type="radio" name="builds" value={v} checked={(app.prefs.tableBuilds || 'each') === v} onchange={() => set('tableBuilds', v)} />
            <span><b>{l}</b><span class="what">{what}</span></span>
          </label>
        {/each}
      </div>
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
      <p class="what">Made by themselves from what VPS says of each table: a release’s features (SSF, VR, FastFlips …) and a machine’s themes, A to Z. Ticked ones show in the library.</p>
      <div class="tagrid">
        {#each byName as g (g.id)}
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

<svelte:window bind:innerWidth />
{#if browser.web?.from === 'fp'}
  <aside class="webdrawer" style="width: {drawer}px" aria-label="Future Pinball’s download">
    <WebPanel back="Close" onback={() => { closeWeb(); readFp(); }} edge={drawer} />
  </aside>
{/if}

<style>
  .webdrawer {
    position: fixed; right: 0; top: 61px; bottom: 0; z-index: 30; display: flex;
    background: var(--side); border-left: 1px solid var(--line); box-shadow: -12px 0 30px rgba(0, 0, 0, 0.45);
  }
  .fpcheck { display: flex; align-items: flex-start; gap: 10px; font-size: 14px; color: var(--fg-soft); max-width: 760px; cursor: pointer; }
  .fpcheck input { margin-top: 3px; }
  .fpcheck > span { display: flex; flex-direction: column; gap: 3px; }
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
  .ok { color: var(--good); font-size: 12px; }
</style>
