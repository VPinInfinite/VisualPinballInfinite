<script>
  // Cabinet mode: the app full screen on the cabinet's playfield screen, the tables chosen with the
  // cabinet's buttons — a wheel of their logos over the table's playfield video, or a row of their
  // playfield pictures (Settings › Cabinet) — the backglass, score display and topper on screens of
  // their own (`cabinet.rs`). The flippers move a table, the magna-saves a letter, Start plays, Exit
  // (Esc, Pause or a gamepad's Back) opens its menu. Idle a while, it moves through the tables by
  // itself; after a game it's back on the same table. The buttons are read as the player reads
  // them: a controller's through the player's input test, the keyboard by the user's own mapping;
  // the mouse works too — a wheel clicked, the mouse's wheel turned, the menu's rows clicked. The
  // wheels sit on an arch, as on a wheel turning; a table rested on plays one sound, at one volume for
  // every table (the menu's Volume, Settings › Cabinet): the first of its backglass's, topper's and
  // playfield's videos with sound, in the order Settings › Cabinet sets, on a screen cabinet mode has,
  // else its music; Start shows "Starting…" until the player's window is up.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { app, play, notice, savePrefs, turnOf, cabinetVolume, cabinetSoundOrder, loudnessOf, volumeAt } from './store.svelte.js';
  import { CODES } from './keycodes.js';

  // what the buttons do here, by VPX's action names
  const ACTIONS = {
    LeftFlipper: 'prev',
    RightFlipper: 'next',
    UILeft: 'prev',
    UIRight: 'next',
    LeftMagna: 'prevLetter',
    RightMagna: 'nextLetter',
    UIUp: 'prevLetter',
    UIDown: 'nextLetter',
    Start: 'play',
    Pause: 'menu',
    InGameUI: 'menu',
  };
  // the keyboard's own, where the user's mapping says nothing of a key
  const KEYS = { ArrowLeft: 'prev', ArrowRight: 'next', ArrowUp: 'prevLetter', ArrowDown: 'nextLetter', Enter: 'play', Escape: 'menu' };

  const visible = $derived(app.tables.filter((t) => !t.hiddenTable));
  let collection = $state('all');
  const byName = (a, b) => a.name.localeCompare(b.name);
  const list = $derived.by(() => {
    if (collection === 'fav') return visible.filter((t) => t.favourite).sort(byName);
    if (collection === 'played') return visible.filter((t) => t.played).sort((a, b) => b.played.last - a.played.last);
    if (collection.startsWith('pl:')) {
      const p = app.playlists.find((x) => `pl:${x.id}` === collection);
      return p ? p.tables.map((k) => visible.find((t) => t.key === k)).filter(Boolean) : [];
    }
    return [...visible].sort(byName);
  });
  let at = $state(0);
  const wrap = (i) => (list.length ? ((i % list.length) + list.length) % list.length : 0);
  const current = $derived(list.length ? list[wrap(at)] : null);

  let playing = $state(false);
  // the player's window up: cabinet mode's own screens put away for it then, not before
  let showing = $state(false);
  // the other screens cabinet mode opened, by the player's names (none in the test mode)
  let others = $state([]);
  // what it plays, at one volume for every table, whatever a table's own settings or its script's
  const volume = $derived(cabinetVolume(app.prefs));
  const gain = $derived(loudnessOf(volume));
  const heard = $derived(gain > 0);
  // the menu's Volume row taken by the buttons: the flippers turn it down or up, Start or Exit is done
  let adjusting = $state(false);
  let menuOpen = $state(false);
  let mi = $state(0);
  let confirmShutdown = $state(false);
  let keyActions = {};
  let lastInput = Date.now();

  const look = $derived(app.prefs.cabinetLook === 'row' ? 'row' : 'wheel');
  const turn = $derived([90, 180, 270].includes(app.prefs.cabinetTurn) ? app.prefs.cabinetTurn : 0);

  // a kind of a table's media: its video, else its picture
  const media = (t, kind) => t?.media.find((m) => m.kind === kind);
  const shown = (t, kind) => media(t, `${kind}-video`) || media(t, kind) || (t?.picture?.[kind] ? { url: t.picture[kind], video: false } : null);
  const wheel = (t) => media(t, 'wheel')?.url || t?.picture?.wheel;
  const picture = (t) => media(t, 'playfield')?.url || t?.picture?.playfield;

  const letter = (t) => {
    const c = (t?.name || '').trim().charAt(0).toUpperCase();
    return /[A-Z]/.test(c) ? c : '#';
  };

  function step(by) {
    at = wrap(at + by);
  }

  // to the first table of the next letter, or of the one before
  function jump(dir) {
    if (!list.length) return;
    const here = letter(list[wrap(at)]);
    let i = wrap(at);
    for (let n = 0; n < list.length && letter(list[i]) === here; n++) i = wrap(i + dir);
    if (dir < 0) {
      const l = letter(list[i]);
      for (let n = 0; n < list.length && letter(list[wrap(i - 1)]) === l; n++) i = wrap(i - 1);
    }
    at = i;
  }

  const menuRows = $derived([
    { id: 'fav', label: 'Favourites' },
    { id: 'played', label: 'Recently played' },
    ...app.playlists.map((p) => ({ id: `pl:${p.id}`, label: p.name })),
    { id: 'all', label: 'All tables' },
    { id: 'volume', label: 'Volume' },
    { id: 'desktop', label: 'Back to the desktop app' },
    { id: 'quit', label: 'Close the app' },
    { id: 'shutdown', label: confirmShutdown ? 'Shut down the PC? Press Start again' : 'Shut down the PC' },
  ]);

  function setVolume(v) {
    app.prefs.cabinetVolume = Math.max(0, Math.min(100, Math.round(v)));
    savePrefs();
  }

  function choose(row) {
    if (row.id === 'volume') adjusting = !adjusting;
    else if (row.id === 'desktop') app.cabinet = false;
    else if (row.id === 'quit') invoke('quit');
    else if (row.id === 'shutdown') {
      if (confirmShutdown) invoke('shut_down').catch((e) => notice(String(e), { kind: 'problem' }));
      else confirmShutdown = true;
    } else {
      collection = row.id;
      at = 0;
      menuOpen = false;
    }
  }

  async function start() {
    const t = current;
    if (!t || playing) return;
    playing = true;
    showing = false;
    await invoke('controls_stop').catch(() => {});
    await play(t, app.prefs.cabinetPreset || null);
  }

  // the keyboard's focus in this page, so the keys work without a click (Windows focuses the window)
  let cab;
  function takeKeys() {
    window.focus();
    cab?.focus({ preventScroll: true });
  }

  function command(c) {
    lastInput = Date.now();
    if (playing) return;
    if (menuOpen && adjusting) {
      if (c === 'prev' || c === 'prevLetter') setVolume(volume - 5);
      else if (c === 'next' || c === 'nextLetter') setVolume(volume + 5);
      else adjusting = false;
      return;
    }
    if (menuOpen) {
      if (c === 'prev' || c === 'prevLetter') mi = (mi - 1 + menuRows.length) % menuRows.length;
      else if (c === 'next' || c === 'nextLetter') mi = (mi + 1) % menuRows.length;
      else if (c === 'play') return choose(menuRows[mi]);
      else if (c === 'menu') menuOpen = false;
      confirmShutdown = false;
      return;
    }
    if (c === 'prev') step(-1);
    else if (c === 'next') step(1);
    else if (c === 'prevLetter') jump(-1);
    else if (c === 'nextLetter') jump(1);
    else if (c === 'play') start();
    else if (c === 'menu') {
      menuOpen = true;
      adjusting = false;
      mi = 0;
    }
  }

  function onInput(m) {
    if (m.t === 'actions') {
      // the keys the user's mapping gives the actions, each alone (not held with another)
      const map = {};
      for (const a of m.list) {
        for (const g of (a.mapping || '').split('|')) {
          const b = g.split('&').map((x) => x.trim());
          if (b.length === 1 && b[0].startsWith('Key;')) map[Number(b[0].split(';')[1])] = a.id;
        }
      }
      keyActions = map;
    } else if (m.t === 'action' && m.on && ACTIONS[m.id]) command(ACTIONS[m.id]);
  }

  // the mouse: a wheel or card clicked chooses it, the chosen one plays; the mouse's wheel browses
  function clickTable(d) {
    lastInput = Date.now();
    if (playing || menuOpen) return;
    if (d === 0) start();
    else step(d);
  }

  let wheelAt = 0;
  function onWheel(e) {
    if (menuOpen || playing || Math.abs(e.deltaY) < 1) return;
    e.preventDefault();
    // one table a notch, however fast the wheel turns
    if (Date.now() - wheelAt < 90) return;
    wheelAt = Date.now();
    command(e.deltaY > 0 ? 'next' : 'prev');
  }

  function clickRow(k) {
    lastInput = Date.now();
    mi = k;
    const row = menuRows[k];
    if (row.id !== 'shutdown') confirmShutdown = false;
    choose(row);
  }

  function key(e) {
    const action = keyActions[CODES[e.code]];
    const c = (action && ACTIONS[action]) || KEYS[e.key];
    if (!c) return;
    e.preventDefault();
    if (e.repeat && c !== 'prev' && c !== 'next') return;
    command(c);
  }

  // what the other screens show: the table chosen's backglass, score display and topper
  $effect(() => {
    const t = current;
    // which screen's video plays its sound, at the volume, when it's one of the other screens
    const other = soundFrom === 'backglass' || soundFrom === 'topper';
    if (t) invoke('cabinet_show', { what: { name: t.name, backglass: shown(t, 'backglass'), scoreView: shown(t, 'dmd'), topper: shown(t, 'topper'), sound: other && heard && !playing ? soundFrom : null, volume: gain } });
  });

  onMount(() => {
    // opened from a table's page: on that table
    if (app.page === 'table') {
      const here = list.findIndex((t) => t.id === app.tableId);
      if (here >= 0) at = here;
    }
    app.page = 'library';
    // the library's page under it, its scrolling put away meanwhile
    const overflow = document.documentElement.style.overflow;
    document.documentElement.style.overflow = 'hidden';
    // the test mode shows it in its own window alone, no screens taken or buttons read
    if (!app.testing) {
      invoke('cabinet_open', { preset: app.prefs.cabinetPreset || null })
        .then((opened) => {
          others = opened || [];
          takeKeys();
        })
        .catch((e) => notice(String(e), { kind: 'problem' }));
      invoke('controls_start').catch(() => {});
    }
    const offInput = listen('input-test', (e) => onInput(e.payload));
    setTimeout(takeKeys, 300);
    // the player's window up: its own screens take over from ours
    const offShowing = listen('player-showing', async () => {
      if (!playing) return;
      showing = true;
      await invoke('cabinet_screens', { shown: false });
    });
    // a game over: back on the same table
    const offEnded = listen('player-ended', async () => {
      if (!playing) return;
      playing = false;
      showing = false;
      lastInput = Date.now();
      await invoke('cabinet_screens', { shown: true });
      invoke('controls_start').catch(() => {});
      takeKeys();
    });
    // nobody playing a while: through the tables by themselves, until a button's pressed
    const attract = setInterval(() => {
      const minutes = app.prefs.cabinetAttract ?? 3;
      if (minutes > 0 && !playing && !menuOpen && list.length > 1 && Date.now() - lastInput > minutes * 60000) step(1);
    }, 8000);
    return () => {
      document.documentElement.style.overflow = overflow;
      clearInterval(attract);
      offInput.then((f) => f());
      offShowing.then((f) => f());
      offEnded.then((f) => f());
      if (!app.testing) {
        invoke('controls_stop').catch(() => {});
        // the window as it was, and the app told whether that's full screen
        invoke('cabinet_close').then((full) => (app.fullscreen = !!full)).catch(() => {});
      }
    };
  });

  // its playfield: its video, with its sound (Settings › Cabinet: videos, as by default), else its picture
  const videos = $derived(app.prefs.cabinetVideos ?? true);
  const still = (t) => media(t, 'playfield') || (t?.picture?.playfield ? { url: t.picture.playfield, video: false } : null);
  const background = $derived(videos ? shown(current, 'playfield') : still(current));
  // one sound at a time: the first screen in the order set whose video has sound — the backglass's
  // or the topper's only where cabinet mode has that screen — else the table's music
  const soundFrom = $derived(
    cabinetSoundOrder(app.prefs).find((s) => {
      const m = s === 'playfield' ? background : others.includes(s) ? shown(current, s) : null;
      return !!(m?.video && m?.sound);
    }) || null,
  );
  const videoSound = $derived(soundFrom === 'playfield');

  // the music of the table rested on, a moment after the wheel stops; none while a table plays
  let tune = $state(null);
  $effect(() => {
    const url = heard && !playing && !soundFrom ? current?.audio : null;
    tune = null;
    if (!url) return;
    const wait = setTimeout(() => (tune = url), 700);
    return () => clearTimeout(wait);
  });
</script>

<svelte:window onkeydown={key} />

<div class="cab turn{turn} turn-frame" role="application" aria-label="Cabinet mode" tabindex="-1" bind:this={cab} onwheel={onWheel}>
  {#if background?.video}
    <!-- a picture of it blurred behind, where the video leaves room; else the video itself -->
    {#if picture(current)}<img class="bg behind" src={picture(current)} alt="" />
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video class="bg behind" src={background.url} autoplay loop playsinline muted></video>
    {/if}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video class="bg" class:turned={background.turn === 90 || background.turn === 270} class:ccw={background.turn === 270} class:upside={background.turn === 180} src={background.url} autoplay loop playsinline muted={!heard || !videoSound || playing} use:volumeAt={gain}></video>
  {:else if background}
    <img class="bg behind" src={background.url} alt="" />
    <img class="bg" class:turned={background.turn === 90 || background.turn === 270} class:ccw={background.turn === 270} class:upside={background.turn === 180} src={background.url} alt="" />
  {/if}
  <div class="shade"></div>

  {#if !app.loaded && !app.tables.length}
    <p class="empty">Reading your tables…</p>
  {:else if !list.length}
    <p class="empty">No tables here. Exit opens the menu.</p>
  {:else if look === 'wheel'}
    <div class="wheels">
      {#each [-2, -1, 0, 1, 2] as d (d)}
        {@const t = list[wrap(at + d)]}
        <button type="button" class="wheel" class:on={d === 0} style="--d: {d}" aria-label={d === 0 ? `Play ${t.name}` : t.name} onclick={() => clickTable(d)}>
          {#if wheel(t)}<img src={wheel(t)} alt="" />{:else}<span>{t.name}</span>{/if}
        </button>
      {/each}
    </div>
  {:else}
    <div class="row">
      {#each [-2, -1, 0, 1, 2] as d (d)}
        {@const t = list[wrap(at + d)]}
        {@const side = turnOf(t, picture(t))}
        <button type="button" class="card" class:on={d === 0} class:turn-frame={side} aria-label={d === 0 ? `Play ${t.name}` : t.name} onclick={() => clickTable(d)}>
          {#if picture(t)}<img src={picture(t)} alt="" class:turned={side === 90 || side === 270} class:ccw={side === 270} class:upside={side === 180} />{:else}<span>{t.name}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}

  {#if current}
    <div class="info">
      <h1 class="display">{current.name}</h1>
      <p>{[current.maker, current.year].filter(Boolean).join(' · ')}{current.played ? ` · played ${current.played.count} ${current.played.count === 1 ? 'time' : 'times'}` : ''}</p>
    </div>
  {/if}
  <p class="hints">Flippers or ← →: browse · Magna-saves or ↑ ↓: next letter · Start or Enter: play · Exit or Esc: menu</p>
  <button type="button" class="menu-button" onclick={() => command('menu')}>Menu</button>

  {#if tune}<audio src={tune} autoplay loop use:volumeAt={gain}></audio>{/if}

  {#if playing && !showing}
    <div class="starting" role="status">
      {#if current && wheel(current)}<img src={wheel(current)} alt="" />{/if}
      <p>Starting {current?.name}…</p>
    </div>
  {/if}

  {#if menuOpen}
    <!-- a click beside the menu closes it -->
    <div class="menu-shade" role="presentation" onclick={() => command('menu')}></div>
    <div class="menu" role="menu">
      {#each menuRows as r, k (r.id)}
        {#if r.id === 'volume'}
          <!-- the buttons: Start takes the row, the flippers turn it; the mouse: its slider -->
          <div class="mi vol" class:on={k === mi} class:adjusting role="menuitem" tabindex="-1" onmouseenter={() => (mi = k)}>
            <span>{r.label}</span>
            <input type="range" min="0" max="100" step="5" value={volume} aria-label="Volume" oninput={(e) => setVolume(Number(e.currentTarget.value))} />
            <span class="pct">{volume ? `${volume}%` : 'Off'}</span>
          </div>
        {:else}
          <button type="button" class="mi" class:on={k === mi} class:here={r.id === collection} role="menuitem" onmouseenter={() => { mi = k; adjusting = false; }} onclick={() => clickRow(k)}>{r.label}</button>
        {/if}
      {/each}
      {#if menuRows[mi]?.id === 'volume'}
        <p class="menu-hint">{adjusting ? 'Flippers: quieter or louder · Start: done' : 'Start: change the volume, the same for every table'}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .cab { position: fixed; inset: 0; z-index: 80; background: #000; color: #e9edf4; overflow: hidden; }
  /* the picture turned for a playfield screen lying another way than Windows has it */
  .cab.turn90 { inset: auto; top: 0; left: 100vw; width: 100vh; height: 100vw; transform-origin: 0 0; transform: rotate(90deg); }
  .cab.turn180 { transform: rotate(180deg); }
  .cab.turn270 { inset: auto; top: 100vh; left: 0; width: 100vh; height: 100vw; transform-origin: 0 0; transform: rotate(-90deg); }
  /* the playfield whole, over a blurred copy of it filling the screen */
  .bg { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
  .bg.behind { inset: -40px; width: calc(100% + 80px); height: calc(100% + 80px); object-fit: cover; filter: blur(30px) brightness(0.6) saturate(1.3); }
  .shade { position: absolute; inset: 0; background: linear-gradient(to bottom, rgba(0, 0, 0, 0.1) 40%, rgba(0, 0, 0, 0.8) 100%); }
  /* five wheels on an arch, as the top of a wheel turning: all one size, evenly spaced and turned
     along it, the chosen one at its top, ringed; the outer ones inside the screen's width (100vmin
     on an upright screen) and clear of the table's name below */
  .wheels { position: absolute; left: 0; right: 0; bottom: 34vmin; height: 0; --r: 95vmin; }
  .wheel {
    --a: calc(var(--d) * 11deg);
    position: absolute; left: 50%; top: 0; width: 17vmin; height: 17vmin; display: flex; align-items: center; justify-content: center; padding: 0; border: 0; border-radius: 50%; background: none; color: inherit; cursor: pointer;
    transform: translate(-50%, -50%) translate(calc(sin(var(--a)) * var(--r)), calc((1 - cos(var(--a))) * var(--r))) rotate(var(--a));
    transition: transform 0.2s;
  }
  .wheel.on { box-shadow: 0 0 0 0.5vmin #4cc2ff, 0 0 3vmin rgba(76, 194, 255, 0.6); }
  .wheel img { max-width: 100%; max-height: 100%; object-fit: contain; filter: drop-shadow(0 6px 18px rgba(0, 0, 0, 0.7)); }
  .wheel span { width: 100%; height: 100%; border-radius: 50%; display: flex; align-items: center; justify-content: center; text-align: center; padding: 1vmin; background: rgba(20, 26, 38, 0.85); border: 2px solid #4cc2ff; font-size: 1.8vmin; font-weight: 700; }
  .row { position: absolute; left: 0; right: 0; bottom: 20%; display: flex; justify-content: center; align-items: center; gap: 2vmin; }
  .card { width: 14vmin; height: 26vmin; padding: 0; border: 0; border-radius: 1vmin; overflow: hidden; background: #111; color: inherit; opacity: 0.6; display: flex; align-items: center; justify-content: center; cursor: pointer; }
  .card.on { width: 19vmin; height: 34vmin; opacity: 1; outline: 0.4vmin solid #4cc2ff; }
  .card img { width: 100%; height: 100%; object-fit: cover; }
  .card span { padding: 1vmin; text-align: center; font-size: 1.6vmin; }
  .info { position: absolute; left: 0; right: 0; bottom: 9vmin; padding: 0 4vmin; text-align: center; }
  .info h1 { margin: 0; font-size: 4.6vmin; line-height: 1; text-transform: uppercase; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-shadow: 0 3px 16px rgba(0, 0, 0, 0.8); }
  .info p { margin: 1vmin 0 0; font-size: 2.2vmin; color: #c3cad6; }
  .hints { position: absolute; left: 0; right: 0; bottom: 3vmin; margin: 0; text-align: center; font-size: 1.7vmin; color: #8b94a6; }
  .empty { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: 3vmin; }
  .menu-button { position: absolute; top: 2vmin; right: 2vmin; padding: 0.8vmin 1.8vmin; border: 1px solid #2c3445; border-radius: 1vmin; background: rgba(13, 16, 22, 0.7); color: #c3cad6; font-size: 1.8vmin; cursor: pointer; }
  .menu-button:hover { color: #fff; border-color: #4cc2ff; }
  .menu-shade { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.35); }
  .starting { position: absolute; inset: 0; z-index: 2; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 3vmin; background: rgba(0, 0, 0, 0.82); }
  .starting img { max-width: 30vmin; max-height: 30vmin; object-fit: contain; }
  .starting p { margin: 0; font-size: 3.4vmin; color: #e9edf4; }
  .cab:focus { outline: none; }
  .menu { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); min-width: 40vmin; padding: 2vmin; border-radius: 1.6vmin; background: rgba(13, 16, 22, 0.94); border: 1px solid #2c3445; display: flex; flex-direction: column; }
  .mi { padding: 1.2vmin 2vmin; border: 0; border-radius: 1vmin; background: none; color: inherit; font-size: 2.6vmin; text-align: left; cursor: pointer; }
  .mi.on { background: #1f2a3c; color: #8bd8ff; }
  .mi.here::after { content: ' ·'; color: #4cc2ff; }
  .mi.vol { display: flex; align-items: center; gap: 2vmin; cursor: default; }
  .mi.vol input { flex: 1; min-width: 16vmin; accent-color: #4cc2ff; cursor: pointer; }
  .mi.vol.adjusting { outline: 0.3vmin solid #4cc2ff; }
  .mi .pct { min-width: 7vmin; text-align: right; font-variant-numeric: tabular-nums; }
  .menu-hint { margin: 1.2vmin 0 0; padding: 0 2vmin; font-size: 1.8vmin; color: #8b94a6; }
</style>
