<script>
  // The app's pause menu over VPX, in a window of its own over the playfield while the game is
  // paused (VPX pauses by itself while this window has the focus), the table seen darkened around
  // it: a panel as our own player's pause menu was, sized to the window. Resume; the volume (VPX's
  // own, changed as it's pressed); the table's rules (the front ends' instruction cards and flyers,
  // and the rules kept in its file with the pictures they name), the guide VPS links for its game
  // (Pinball Primer's own page as the site draws it, in a view of its own over this page that the
  // menu's keys and a controller scroll, its address a link to the user's browser),
  // its achievements, the keys and
  // buttons it's played with, its settings (VPX reads them as a table starts: they apply from the
  // next game); VPX's own menu, for what only it changes at once; and Quit. VPX's own menu keys move
  // through it as they move through VPX's — the magna-saves up and down, the flippers left and right,
  // the launch button or Start to choose — and the arrows and Enter too; Esc goes back, or plays on,
  // F12 plays on; a controller and the mouse work too (the right button goes back).
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import TableSettings from './TableSettings.svelte';
  import Reader from './Reader.svelte';
  import { app } from './store.svelte.js';
  import { startPad } from './gamepad.js';
  import { CODES } from './keycodes.js';

  /** The app's test mode: the table shown as if paused, by part of its name. */
  let { test = null } = $props();

  let state = $state(null);
  let page = $state('main');
  let at = $state(0);
  let volume = $state(null);
  // each part of the sound's level, as the app gives them: changed as they're pressed
  let parts = $state([]);
  let busy = $state(false);
  let size = $state({ w: window.innerWidth, h: window.innerHeight });
  let list = $state();
  let scroller = $state();
  // the guide's pages once they've been found for the Guide page (their titles and addresses), or
  // why they couldn't be; the one shown; the view's place on the page, whether its page has loaded,
  // and why it couldn't be shown
  let guide = $state(null);
  let shown = $state(0);
  let viewBox = $state();
  let viewLoaded = $state(false);
  let viewProblem = $state('');

  async function load() {
    try {
      state = await invoke('pause_menu_state', { test });
    } catch {
      state = null;
    }
    volume = typeof state?.volume === 'number' ? state.volume : null;
    parts = state?.levels || [];
    page = 'main';
    at = 0;
    guide = null;
    hideGuide();
  }

  onMount(() => {
    load();
    const stop = startPad();
    const unlisten = listen('pause-menu-open', load);
    // the guide's view shows its page once it has loaded
    const loaded = listen('pause-guide-loaded', (e) => {
      if (page === 'guide' && e.payload === guide?.pages?.[shown]?.name) viewLoaded = true;
    });
    const resize = () => (size = { w: window.innerWidth, h: window.innerHeight });
    window.addEventListener('resize', resize);
    return () => {
      stop();
      unlisten.then((f) => f());
      loaded.then((f) => f());
      window.removeEventListener('resize', resize);
      hideGuide();
    };
  });

  // sized as our own player's menu was, a fifth smaller: by the window's height, held back by its
  // width (a portrait playfield's)
  const s = $derived(Math.max(0.45, Math.min(Math.min(Math.max(size.h / 1080, 0.6), 2.4) * 0.8, (size.w - 32) / 720)));

  const t = $derived(state?.table);
  const media = $derived(t?.media || []);
  const cards = $derived(media.filter((m) => m.kind === 'gamehelp' && !m.video));
  const flyers = $derived(media.filter((m) => m.kind === 'gameinfo' && !m.video));
  const ruleBlocks = $derived(blocks(state?.rules));
  // the table's own pictures that show its rules, its manuals and read-mes
  const tableCards = $derived(state?.cards || []);
  const docs = $derived(t?.docs || []);
  const nothing = $derived(!cards.length && !flyers.length && !ruleBlocks.length && !tableCards.length && !docs.length);
  const vpsRules = $derived(t?.vpsHas?.rules || 0);
  // a manual or read-me being read
  let reader = $state(null);
  // the test mode's picture: the table behind it, as it would be paused
  const behind = $derived(state?.testing ? (media.find((m) => m.kind === 'playfield' && m.chosen && !m.video) || media.find((m) => m.kind === 'playfield' && !m.video))?.url : null);

  const rows = $derived.by(() => {
    const table = [
      { id: 'rules', label: 'Rules', opens: true, note: 'Its instruction cards, flyers and rules' },
      state?.guide && { id: 'guide', label: 'Guide', opens: true, note: 'How to play this game, from Pinball Primer' },
      state?.summary && { id: 'achievements', label: 'Achievements', opens: true, note: state.summary },
      { id: 'controls', label: 'Controls', opens: true, note: 'The keys and buttons it’s played with' },
      t && { id: 'settings', label: 'Its settings', opens: true, note: 'VPX reads them as a table starts: they apply from your next game' },
    ]
      .filter(Boolean)
      .map((r, i) => (i === 0 ? { ...r, section: 'Table' } : r));
    return [
      { id: 'resume', label: 'Resume', note: 'Play on' },
      { id: 'volume', label: 'Volume', section: 'Sound', level: { value: volume, max: 100 }, note: 'VPX’s volume: the playfield’s and the backglass’s together, kept for your next games' },
      ...parts.map((p) => ({ id: `part:${p.id}`, label: p.label, part: p, level: { value: Math.round(p.value * 100), max: 100 }, note: '100% is as it came, less turns it down: kept for this table' })),
      ...(parts.some((p) => Math.abs(p.value - 1) > 0.001) ? [{ id: 'reset', label: 'Put the parts back as they came', note: 'Every part of the sound back to as the table came' }] : []),
      ...table,
      { id: 'vpx', label: 'VPX’s own menu', opens: true, section: '', note: 'For what only VPX changes at once: the view, the table’s options, the playfield’s and backglass’s volumes apart' },
      { id: 'quit', label: 'Quit the game', note: 'Back to the app' },
    ];
  });

  const heading = $derived({ main: 'Paused', rules: 'Rules', guide: 'Guide', controls: 'Controls', achievements: 'Achievements' }[page] || 'Paused');
  const byline = $derived(t ? [t.maker, t.year].filter(Boolean).join(' · ') : '');
  const note = $derived(page === 'main' ? rows[at]?.note || '' : page === 'controls' ? 'Change them in the app: Settings › Controls' : page === 'achievements' ? state?.summary || '' : '');
  const keysHint = $derived(
    page === 'main' ? 'Magna-saves or ↑ ↓ move · flippers or ← → change · Start or Enter chooses · F12 plays on' : 'Magna-saves or ↑ ↓ scroll · a flipper or Esc goes back · F12 plays on',
  );

  /** The table's own rules, VPX's Markdown, as blocks: headings, paragraphs, a list's items, and the
   * table's own pictures they name (never a picture from elsewhere). */
  function blocks(md) {
    const out = [];
    let para = [];
    const flush = () => {
      if (para.length) out.push({ kind: 'p', text: para.join(' ') });
      para = [];
    };
    for (const raw of (md || '').split(/\r?\n/)) {
      let line = raw.trim();
      const re = /!\[([^\]]*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g;
      let m;
      let last = 0;
      while ((m = re.exec(line))) {
        const before = line.slice(last, m.index).trim();
        if (before) para.push(plain(before));
        flush();
        if (!/^[a-z]+:/i.test(m[2])) out.push({ kind: 'img', alt: m[1], name: m[2] });
        last = re.lastIndex;
      }
      line = line.slice(last).trim();
      if (!line) {
        flush();
        continue;
      }
      const h = line.match(/^#{1,6}\s+(.*)$/);
      const li = line.match(/^(?:[-*+]|\d+[.)])\s+(.*)$/);
      if (h) {
        flush();
        out.push({ kind: 'h', text: plain(h[1]) });
      } else if (li) {
        flush();
        out.push({ kind: 'li', text: plain(li[1]) });
      } else para.push(plain(line));
    }
    flush();
    return out;
  }

  // emphasis and links' marks taken off, their words kept
  const plain = (s) => s.replace(/\*\*|__/g, '').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').replace(/(^|\s)[*_](?=\S)/g, '$1').replace(/(\S)[*_](?=\s|$)/g, '$1');

  async function open(id) {
    page = id;
    await tick();
    scroller?.scrollTo({ top: 0 });
    // a controller's right stick scrolls what has the focus
    (scroller || document.querySelector('.panel .back'))?.focus({ preventScroll: true });
  }

  // the Guide page opened at once, the guide's pages found on the site, the first shown in the view
  async function openGuide() {
    guide = null;
    shown = 0;
    viewLoaded = false;
    viewProblem = '';
    await open('guide');
    try {
      guide = { pages: await invoke('pause_menu_guide', { pages: state.guide.pages, year: state.guide.year ?? null }) };
    } catch (e) {
      guide = { problem: String(e) };
    }
  }

  // the view put over its place on the page, showing the page chosen: again as the window's size changes
  $effect(() => {
    const p = guide?.pages?.[shown];
    if (page !== 'guide' || !viewBox || !p || !size) return;
    const r = viewBox.getBoundingClientRect();
    invoke('pause_menu_guide_show', { page: p.name, x: r.left, y: r.top, width: r.width, height: r.height }).catch((e) => (viewProblem = String(e)));
  });

  function showPage(i) {
    if (i === shown) return;
    viewLoaded = false;
    shown = i;
  }

  function hideGuide() {
    invoke('pause_menu_guide_hide').catch(() => {});
  }

  // the page shown opened in the user's own browser
  async function browse() {
    const p = guide?.pages?.[shown];
    if (!p) return;
    try {
      await invoke('pause_menu_guide_browse', { page: p.name });
    } catch (e) {
      app.notices.push({ id: Date.now(), text: String(e), kind: 'problem' });
    }
  }

  // the guide's page scrolled by the menu's keys, a step a press
  function scrollGuide(down) {
    if (!viewLoaded) return;
    invoke('pause_menu_guide_scroll', { by: (down ? 1 : -1) * (viewBox?.clientHeight || 400) * 0.3, smooth: true }).catch(() => {});
  }

  // a controller's sticks and pad scroll the guide, whose view can't take the focus to scroll itself
  $effect(() => {
    if (page !== 'guide') return;
    let frame = 0;
    let last = 0;
    const poll = (now) => {
      frame = requestAnimationFrame(poll);
      if (!document.hasFocus() || !viewLoaded) return;
      const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter(Boolean) : [];
      let y = 0;
      for (const p of pads) {
        for (const a of [p.axes[1] ?? 0, p.axes[3] ?? 0]) if (Math.abs(a) > Math.abs(y)) y = a;
        if (p.buttons[12]?.pressed) y = -1;
        if (p.buttons[13]?.pressed) y = 1;
      }
      if (Math.abs(y) > 0.2 && now - last > 32) {
        last = now;
        invoke('pause_menu_guide_scroll', { by: y * 36, smooth: false }).catch(() => {});
      }
    };
    frame = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(frame);
  });

  function back() {
    const from = page;
    if (from === 'guide') hideGuide();
    page = 'main';
    at = Math.max(0, rows.findIndex((r) => r.id === from));
  }

  async function act(id) {
    if (busy) return;
    if (id === 'guide') return openGuide();
    if (['rules', 'achievements', 'controls', 'settings'].includes(id)) return open(id);
    if (id === 'reset') {
      for (const p of parts) setPart(p, 1);
      return;
    }
    if (id === 'volume' || id.startsWith('part:') || state?.testing) return;
    busy = true;
    try {
      await invoke('pause_menu_do', { what: id === 'vpx' ? 'vpx-menu' : id });
    } catch (e) {
      app.notices.push({ id: Date.now(), text: String(e), kind: 'problem' });
    } finally {
      busy = false;
    }
  }

  async function changeVolume(by) {
    if (volume === null || !by) return;
    const want = Math.max(0, Math.min(100, volume + by));
    if (want === volume) return;
    volume = want;
    if (state?.testing) return;
    try {
      const now = await invoke('pause_menu_do', { what: 'volume', steps: by });
      if (typeof now === 'number') volume = now;
    } catch (e) {
      app.notices.push({ id: Date.now(), text: String(e), kind: 'problem' });
    }
  }

  // a part of the sound set, told to the game at once
  async function setPart(p, value) {
    p.value = value;
    if (state?.testing) return;
    try {
      await invoke('pause_menu_do', { what: 'level', id: p.id, value });
    } catch (e) {
      app.notices.push({ id: Date.now(), text: String(e), kind: 'problem' });
    }
  }

  // a level row moved by a number of its percents
  function changeLevel(r, by) {
    if (r.id === 'volume') return changeVolume(by);
    if (!r.part) return;
    const want = Math.max(0, Math.min(r.level.max, r.level.value + by));
    if (want !== r.level.value) setPart(r.part, want / 100);
  }

  function key(e) {
    // the settings window and the reader have their own keys; Esc closes them back to the menu
    if (page === 'settings' || reader) return;
    const nav = state?.nav || {};
    const code = CODES[e.code];
    const is = (list) => !!list && list.includes(code);
    const k = e.key;
    const up = k === 'ArrowUp' || is(nav.up);
    const down = k === 'ArrowDown' || is(nav.down);
    const left = k === 'ArrowLeft' || is(nav.left);
    const right = k === 'ArrowRight' || is(nav.right);
    const choose = k === 'Enter' || k === ' ' || is(nav.choose);
    if (k === 'F12') {
      e.preventDefault();
      act('resume');
      return;
    }
    if (k === 'Escape' || k === 'Backspace') {
      e.preventDefault();
      if (page === 'main') act('resume');
      else back();
      return;
    }
    if (page !== 'main') {
      if (up || down) {
        e.preventDefault();
        if (page === 'guide') scrollGuide(down);
        else scroller?.scrollBy({ top: (down ? 1 : -1) * scroller.clientHeight * 0.3, behavior: 'smooth' });
      } else if (left || right || choose) {
        e.preventDefault();
        back();
      }
      return;
    }
    if (up || down) {
      e.preventDefault();
      at = (at + (up ? rows.length - 1 : 1)) % rows.length;
    } else if (left || right) {
      e.preventDefault();
      const r = rows[at];
      if (r?.level) changeLevel(r, right ? 5 : -5);
      else if (right && r?.opens) act(r.id);
    } else if (choose) {
      e.preventDefault();
      act(rows[at].id);
    }
  }

  // the row pointed at keeps the keyboard's focus, so a controller, the mouse and the keys agree
  $effect(() => {
    if (page === 'main') list?.querySelectorAll('[data-row]')[at]?.focus({ preventScroll: true });
  });
</script>

<svelte:window onkeydown={key} />

<main
  class="pause"
  style="--s: {s}"
  oncontextmenu={(e) => {
    e.preventDefault();
    if (page !== 'main') back();
  }}
>
  {#if behind}<img class="behind" src={behind} alt="" />{/if}
  <div class="dim"></div>

  {#if page === 'settings' && t}
    <TableSettings {t} onclose={() => back()} onfindvps={() => {}} onscreens={() => {}} />
  {:else}
    <section class="panel" class:wide={page === 'rules' || page === 'guide'} class:tall={page === 'guide'} aria-label="Pause menu">
      <header>
        <span class="heading">{heading}</span>
        <h1>{t?.name ?? 'Visual Pinball Infinite'}</h1>
        {#if byline}<p class="byline">{byline}</p>{/if}
      </header>

      {#if page === 'main'}
        <div class="rows" bind:this={list} role="menu" aria-label="Pause menu">
          {#each rows as r, i (r.id)}
            {#if r.section !== undefined}<div class="section" class:gap={r.section === ''}>{r.section}</div>{/if}
            {#if r.level}
              <div class="row level" class:on={i === at} role="presentation" onmouseenter={() => (at = i)}>
                <span class="label">{r.label}</span>
                <input
                  type="range"
                  data-row
                  min="0"
                  max={r.level.max}
                  step="5"
                  value={r.level.value ?? 0}
                  disabled={r.level.value === null}
                  aria-label={r.label}
                  style="--fill: {((r.level.value ?? 0) / r.level.max) * 100}%"
                  onfocus={() => (at = i)}
                  oninput={(e) => changeLevel(r, Number(e.currentTarget.value) - (r.level.value ?? 0))}
                />
                <span class="value">{r.level.value === null ? '—' : i === at ? `‹  ${r.level.value}%  ›` : `${r.level.value}%`}</span>
              </div>
            {:else}
              <button type="button" data-row role="menuitem" class="row" class:on={i === at} onfocus={() => (at = i)} onmouseenter={() => (at = i)} onclick={() => act(r.id)}>
                <span class="label">{r.label}</span>
                {#if r.opens}<span class="go" aria-hidden="true">›</span>{/if}
              </button>
            {/if}
          {/each}
        </div>
      {:else if page === 'rules'}
        <div class="scroll" bind:this={scroller} tabindex="-1">
          {#if cards.length}
            <div class="part">{cards.length > 1 ? 'Instruction cards' : 'Instruction card'}</div>
            {#each cards as c (c.url)}<img class="card" src={c.url} alt="Instruction card" />{/each}
          {/if}
          {#if ruleBlocks.length}
            <div class="part">Its rules</div>
            <div class="own">
              {#each ruleBlocks as b, i (i)}
                {#if b.kind === 'h'}<h2>{b.text}</h2>
                {:else if b.kind === 'li'}<p class="li">{b.text}</p>
                {:else if b.kind === 'img'}<img class="card" src={`${state.pictures}${encodeURIComponent(b.name)}`} alt={b.alt || 'From its rules'} />
                {:else}<p>{b.text}</p>{/if}
              {/each}
            </div>
          {/if}
          {#if tableCards.length}
            <div class="part">On the table</div>
            {#each tableCards as name (name)}
              <img class="card" src={`${state.pictures}${encodeURIComponent(name)}`} alt={name} onerror={(e) => e.currentTarget.remove()} />
            {/each}
          {/if}
          {#if flyers.length}
            <div class="part">{flyers.length > 1 ? 'Flyers' : 'Flyer'}</div>
            {#each flyers as f (f.url)}<img class="card" src={f.url} alt="Flyer" />{/each}
          {/if}
          {#if docs.length}
            <div class="part">To read</div>
            <div class="docs">
              {#each docs as d (d.url)}
                <button type="button" class="back" onclick={() => (reader = { title: t.short, docs: [d] })}><Icon name="file" size={16} />{d.name}</button>
              {/each}
            </div>
          {/if}
          {#if nothing}
            <p class="none">Nothing is kept for {t?.short ?? 'this table'} yet: no instruction card, and no rules in its table file. Add an instruction card on its page and it shows here.{vpsRules ? ` VPS lists ${vpsRules === 1 ? 'one' : vpsRules} for this game.` : ''}</p>
          {/if}
        </div>
        <button type="button" class="back" onclick={back}><Icon name="chevL" size={16} />Back</button>
      {:else if page === 'guide'}
        {#if guide?.pages}
          <div class="guide-head">
            {#if guide.pages.length > 1}
              <div class="choices">
                {#each guide.pages as p, i (p.name)}
                  <button type="button" class="back" class:on={i === shown} data-pad="skip" onclick={() => showPage(i)}>{p.title}</button>
                {/each}
              </div>
            {/if}
            <button type="button" class="source" data-pad="skip" title="Open it in your browser" onclick={browse}>
              <span class="who">Pinball Primer’s page</span>
              <span class="address">{guide.pages[shown].address.replace(/^https:\/\//, '')}</span>
              <Icon name="ext" size={14} />
            </button>
          </div>
        {/if}
        <!-- the guide's own page shows over this, in a view of its own, once it has loaded -->
        <div class="view" bind:this={viewBox} data-loading={!guide || (guide.pages && !viewLoaded && !viewProblem) ? '' : undefined}>
          {#if !guide}
            <p class="none">Finding the guide on Pinball Primer…</p>
          {:else if guide.problem}
            <p class="none">{guide.problem}</p>
          {:else if viewProblem}
            <p class="none">{viewProblem}</p>
          {:else if !viewLoaded}
            <p class="none">Opening Pinball Primer’s page…</p>
          {/if}
        </div>
        <button type="button" class="back" onclick={back}><Icon name="chevL" size={16} />Back</button>
      {:else if page === 'controls'}
        <div class="scroll" bind:this={scroller} tabindex="-1">
          {#each state?.controls || [] as l, i (i)}
            {#if l.section}<div class="section" class:first={i === 0}>{l.section}</div>{/if}
            <div class="line"><span class="what">{l.label}</span><span class="on-keys">{l.keys}</span></div>
          {/each}
        </div>
        <button type="button" class="back" onclick={back}><Icon name="chevL" size={16} />Back</button>
      {:else if page === 'achievements'}
        <div class="scroll" bind:this={scroller} tabindex="-1">
          {#each state?.lines || [] as l, i (i)}
            {#if l.heading}
              <div class="section" class:first={i === 0}>{l.heading.replaceAll('**', '')}</div>
            {:else}
              {@const [head, ...about] = l.line.split('\n')}
              {@const parts = head.replaceAll('**', '').split(' · ')}
              <div class="ach" class:earned={parts.at(-1) === 'Earned' || parts.includes('Earned')}>
                <span class="what">{parts[0]}</span><span class="how">{parts.slice(1).join(' · ')}</span>
                {#if about.length}<small>{about.join(' ')}</small>{/if}
              </div>
            {/if}
          {/each}
        </div>
        <button type="button" class="back" onclick={back}><Icon name="chevL" size={16} />Back</button>
      {/if}

      <footer>
        {#if note}<p class="note">{note}</p>{/if}
        <p class="keys">{keysHint}</p>
      </footer>
    </section>
  {/if}

  {#if reader}<Reader title={reader.title} docs={reader.docs} onclose={() => (reader = null)} />{/if}

  {#if app.notices.length}
    <div class="notices">{#each app.notices.slice(-2) as n (n.id)}<p class:problem={n.kind === 'problem'}>{n.text}</p>{/each}</div>
  {/if}
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .pause {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: grid;
    place-items: center;
    overflow: hidden;
    color: #e8ecf2;
    font-family: Inter, sans-serif;
  }
  .behind {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: rgba(4, 6, 10, 0.55);
  }
  .panel {
    position: relative;
    width: calc(720px * var(--s));
    max-height: calc(100vh - 24px);
    box-sizing: border-box;
    padding: calc(36px * var(--s)) calc(40px * var(--s)) calc(28px * var(--s));
    display: flex;
    flex-direction: column;
    background: #0f141d;
    border: max(1px, calc(1.5px * var(--s))) solid #273041;
    border-radius: calc(18px * var(--s));
    box-shadow: 0 calc(24px * var(--s)) calc(64px * var(--s)) rgba(0, 0, 0, 0.55);
  }
  .panel.wide {
    width: min(calc(100vw - 32px), calc(1100px * var(--s)));
  }
  header {
    display: flex;
    flex-direction: column;
  }
  .heading {
    font-family: 'Barlow Condensed', Inter, sans-serif;
    font-weight: 700;
    font-size: calc(46px * var(--s));
    line-height: 1;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    color: var(--accent, #4cc2ff);
  }
  h1 {
    margin: calc(6px * var(--s)) 0 0;
    font-size: calc(24px * var(--s));
    font-weight: 700;
    line-height: 1.2;
    text-wrap: balance;
  }
  .byline {
    margin: calc(2px * var(--s)) 0 0;
    font-size: calc(16px * var(--s));
    color: #aab3c2;
  }
  .rows {
    margin: calc(14px * var(--s)) calc(-14px * var(--s)) 0;
    padding: 0 calc(14px * var(--s));
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: calc(2px * var(--s));
  }
  .section {
    margin: calc(18px * var(--s)) 0 calc(6px * var(--s));
    font-size: calc(13px * var(--s));
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #8b94a6;
  }
  .section.gap {
    margin: calc(12px * var(--s)) 0 0;
  }
  .section.first {
    margin-top: 0;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: calc(14px * var(--s));
    flex: none;
    height: calc(46px * var(--s));
    margin: 0 calc(-14px * var(--s));
    padding: 0 calc(14px * var(--s));
    border: 0;
    border-radius: calc(10px * var(--s));
    background: transparent;
    color: #c3cad6;
    font: inherit;
    font-size: calc(20px * var(--s));
    font-weight: 600;
    text-align: left;
    cursor: pointer;
    outline: none;
  }
  .row.on {
    background: #24314a;
    color: #fff;
  }
  .row.on::before {
    content: '';
    position: absolute;
    left: 0;
    top: calc(10px * var(--s));
    bottom: calc(10px * var(--s));
    width: calc(4px * var(--s));
    border-radius: calc(2px * var(--s));
    background: var(--accent, #4cc2ff);
  }
  .row.level {
    cursor: default;
  }
  .label {
    flex: 1;
    min-width: 0;
  }
  .go {
    font-size: calc(26px * var(--s));
    line-height: 1;
    color: #8b94a6;
  }
  .row.on .go {
    color: var(--accent, #4cc2ff);
  }
  .value {
    min-width: calc(96px * var(--s));
    text-align: right;
    font-size: calc(18px * var(--s));
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    white-space: pre;
  }
  input[type='range'] {
    -webkit-appearance: none;
    appearance: none;
    width: calc(190px * var(--s));
    height: calc(6px * var(--s));
    margin: 0;
    border-radius: 99px;
    background: linear-gradient(to right, var(--accent, #4cc2ff) var(--fill), #2c3445 var(--fill));
    opacity: 0.75;
    outline: none;
    cursor: pointer;
  }
  .row.on input[type='range'] {
    opacity: 1;
  }
  input[type='range']::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: calc(16px * var(--s));
    height: calc(16px * var(--s));
    border-radius: 50%;
    background: #fff;
    opacity: 0;
  }
  .row.on input[type='range']::-webkit-slider-thumb {
    opacity: 1;
  }
  .scroll {
    margin-top: calc(18px * var(--s));
    min-height: 0;
    flex: 1 1 auto;
    overflow: auto;
    outline: none;
    display: flex;
    flex-direction: column;
    gap: calc(8px * var(--s));
    font-size: calc(17px * var(--s));
    color: #c3cad6;
  }
  .part {
    margin-top: calc(8px * var(--s));
    font-size: calc(13px * var(--s));
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #8b94a6;
  }
  .part:first-child {
    margin-top: 0;
  }
  .card {
    display: block;
    width: 100%;
    height: auto;
    border-radius: calc(8px * var(--s));
  }
  .docs {
    display: flex;
    flex-wrap: wrap;
    gap: calc(8px * var(--s));
  }
  .docs .back {
    margin-top: 0;
  }
  .none {
    margin: 0;
    line-height: 1.45;
    color: #aab3c2;
  }
  .own {
    display: flex;
    flex-direction: column;
    gap: calc(8px * var(--s));
    line-height: 1.45;
  }
  .own h2 {
    margin: calc(6px * var(--s)) 0 0;
    font-size: calc(19px * var(--s));
    color: #fff;
  }
  .own p {
    margin: 0;
  }
  .own .li::before {
    content: '•  ';
    color: var(--accent, #4cc2ff);
  }
  /* the Guide page: as tall as the window allows, for the site's own page shown over its view */
  .panel.tall {
    height: calc(100vh - 24px);
  }
  .guide-head {
    margin-top: calc(14px * var(--s));
    display: flex;
    flex-direction: column;
    gap: calc(8px * var(--s));
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: calc(8px * var(--s));
  }
  .choices .back {
    margin-top: 0;
  }
  .choices .back.on {
    border-color: var(--accent, #4cc2ff);
    color: #fff;
  }
  .source {
    align-self: flex-start;
    display: inline-flex;
    align-items: baseline;
    gap: calc(8px * var(--s));
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: calc(15px * var(--s));
    color: #aab3c2;
    cursor: pointer;
  }
  .source .address {
    color: var(--accent, #4cc2ff);
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .source:hover .address,
  .source:focus-visible .address {
    color: #fff;
  }
  .view {
    margin-top: calc(10px * var(--s));
    flex: 1 1 auto;
    min-height: calc(160px * var(--s));
    display: grid;
    place-items: center;
    padding: calc(16px * var(--s));
    background: #161c27;
    text-align: center;
  }
  .line,
  .ach {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: calc(2px * var(--s)) calc(16px * var(--s));
    align-items: baseline;
    padding: calc(2px * var(--s)) 0;
  }
  .what {
    font-weight: 600;
    color: #c3cad6;
  }
  .on-keys {
    color: #fff;
    text-align: right;
  }
  .ach .how {
    color: #aab3c2;
    font-size: calc(15px * var(--s));
    white-space: nowrap;
  }
  .ach small {
    grid-column: 1 / -1;
    color: #8b94a6;
    font-size: calc(14px * var(--s));
  }
  .ach.earned .what {
    color: #fff;
  }
  .ach.earned .how {
    color: #ffc65c;
  }
  .back {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: calc(6px * var(--s));
    margin-top: calc(14px * var(--s));
    padding: calc(8px * var(--s)) calc(14px * var(--s));
    border: 1px solid #2a3344;
    border-radius: calc(8px * var(--s));
    background: #161c27;
    color: #e8ecf2;
    font: inherit;
    font-size: calc(16px * var(--s));
    cursor: pointer;
  }
  .back:focus-visible {
    outline: 2px solid var(--accent, #4cc2ff);
  }
  footer {
    margin-top: calc(16px * var(--s));
    padding-top: calc(12px * var(--s));
    border-top: 1px solid #273041;
  }
  .note {
    margin: 0 0 calc(4px * var(--s));
    font-size: calc(15px * var(--s));
    color: #aab3c2;
  }
  .keys {
    margin: 0;
    font-size: calc(13px * var(--s));
    color: #8b94a6;
  }
  .notices {
    position: fixed;
    left: 8vw;
    right: 8vw;
    bottom: 4vh;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .notices p {
    margin: 0;
    padding: 10px 14px;
    border-radius: 10px;
    background: #1b2433;
  }
  .notices p.problem {
    background: #3a1d26;
  }
</style>
