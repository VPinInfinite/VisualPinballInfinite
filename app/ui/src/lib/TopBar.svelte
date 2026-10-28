<script>
  // The top bar: the sections, the smart search — the library's, or the VPS browser's while it shows,
  // each searching its own — what runs in the background, adding tables, cabinet mode, settings. The
  // background's bar beside Add tables: pointed at, each task and how far —
  // those waiting their turn too, and each one's steps, done, under way or to come.
  import Icon from './Icon.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { app, vps, drop, goLibrary, openTable, toggleFullscreen, leaving } from './store.svelte.js';

  // a page left from the achievements editor with changes not saved asks first
  const go = (page) => () => leaving(() => (app.page = page));
  const goHome = () => leaving(goLibrary);

  // VPIDB beside Add tables: connected and sharing (a green check), or sharing failing (a red ✕,
  // the app trying again until VPIDB answers); its Settings page on a click
  let vpidb = $state(null);
  const vpidbNow = () => invoke('vpidb_status').then((s) => (vpidb = s)).catch(() => {});
  $effect(() => {
    vpidbNow();
    const off = listen('vpidb-sync', (e) => (vpidb = e.payload));
    return () => off.then((f) => f());
  });
  // connected or disconnected on its page: seen again on leaving it
  $effect(() => {
    void app.page;
    void app.settingsPage;
    vpidbNow();
  });
  const vpidbTitle = $derived(!vpidb?.connected ? 'Disconnected' : vpidb.ok === false ? `Not syncing: ${vpidb.problem || 'VPIDB didn’t answer'}. Trying again every two minutes.` : 'Connected');

  let open = $state(false);
  let box = $state();

  const inBrowser = $derived(app.page === 'browser');

  // how far the tasks under way have come, each that can tell, together; null: no telling
  const going = $derived(app.tasks.filter((t) => !t.waiting));
  const waiting = $derived(app.tasks.length - going.length);
  const sure = $derived(going.filter((t) => t.of > 0));
  const overall = $derived(sure.length === going.length && sure.length ? sure.reduce((n, t) => n + t.done / t.of, 0) / sure.length : null);
  const barName = $derived(app.tasks.length === 1 ? app.tasks[0].label : waiting ? `${going.length} running, ${waiting} waiting` : `${app.tasks.length} tasks running`);
  const q = $derived((inBrowser ? vps.query : app.query).trim().toLowerCase());

  // VPS's machines, makers, people and ROMs that match, each kind on its own
  const foundVps = $derived.by(() => {
    if (q.length < 2) return [];
    const has = (s) => (s || '').toLowerCase().includes(q);
    const groups = [];
    const sub = (m) => [m.maker, m.year].filter(Boolean).join(' · ');
    const machines = vps.machines.filter((m) => has(m.name));
    if (machines.length) groups.push({ label: 'Machines', items: machines.slice(0, 6).map((m) => ({ text: m.name, sub: sub(m), go: () => (vps.sel = m.id) })) });
    const makers = new Map();
    for (const m of vps.machines) if (has(m.maker)) makers.set(m.maker, (makers.get(m.maker) || 0) + 1);
    if (makers.size) groups.push({ label: 'Makers', items: [...makers].sort((a, b) => b[1] - a[1]).slice(0, 4).map(([m, n]) => ({ text: m, sub: `${n} machines`, go: () => Object.assign(vps, { maker: m, query: '' }) })) });
    const people = new Map();
    for (const m of vps.machines) for (const a of [...m.designers, ...m.authors]) if (has(a)) people.set(a, (people.get(a) || 0) + 1);
    if (people.size) groups.push({ label: 'People', items: [...people].sort((a, b) => b[1] - a[1]).slice(0, 5).map(([a, n]) => ({ text: a, sub: `${n} ${n === 1 ? 'machine' : 'machines'}`, go: () => (vps.query = a) })) });
    const roms = [];
    for (const m of vps.machines) for (const r of m.roms) if (has(r) && !roms.some((x) => x.text === r)) roms.push({ text: r, sub: m.name, go: () => (vps.sel = m.id) });
    if (roms.length) groups.push({ label: 'ROMs', items: roms.slice(0, 5) });
    return groups;
  });

  // the library's tables, people, makers and files that match, each kind on its own
  const found = $derived.by(() => {
    if (inBrowser) return foundVps;
    if (q.length < 2) return [];
    const has = (s) => (s || '').toLowerCase().includes(q);
    const groups = [];
    const tables = app.tables.filter((t) => has(t.name) || has(t.file));
    if (tables.length) groups.push({ label: 'Tables', items: tables.slice(0, 6).map((t) => ({ text: t.name, sub: t.file, go: () => openTable(t.id, tables.map((x) => x.id)) })) });
    const makers = [...new Set(app.tables.map((t) => t.maker).filter((m) => has(m)))];
    if (makers.length) groups.push({ label: 'Makers', items: makers.slice(0, 4).map((m) => ({ text: m, sub: `${app.tables.filter((t) => t.maker === m).length} tables`, go: () => showGroup('maker', m) })) });
    const people = new Map();
    for (const t of app.tables) for (const a of [...t.authors, ...t.designers]) if (has(a)) people.set(a, (people.get(a) || 0) + 1);
    if (people.size) groups.push({ label: 'People', items: [...people].sort((a, b) => b[1] - a[1]).slice(0, 5).map(([a, n]) => ({ text: a, sub: `${n} ${n === 1 ? 'table' : 'tables'}`, go: () => filterBy(a) })) });
    const roms = app.tables.filter((t) => has(t.rom));
    if (roms.length) groups.push({ label: 'ROMs', items: roms.slice(0, 5).map((t) => ({ text: t.rom, sub: t.name, go: () => openTable(t.id, roms.map((x) => x.id)) })) });
    const files = [];
    for (const t of app.tables) {
      for (const [kind, path] of [['Backglass', t.holds.backglass], ['PinUP pack', t.holds.pupPack], ['AltSound', t.holds.altsound], ['POV', t.holds.pov], ['Physics', t.holds.physics]]) {
        const name = path ? path.split('\\').pop() : '';
        if (name && name.toLowerCase().includes(q)) files.push({ text: name, sub: `${kind} · ${t.name}`, go: () => openTable(t.id, [t.id]) });
      }
    }
    if (files.length) groups.push({ label: 'Files', items: files.slice(0, 5) });
    return groups;
  });

  function showGroup(by, name) {
    app.prefs.groupBy = by;
    app.pick = `g:${name}`;
    app.query = '';
    goLibrary();
  }

  function filterBy(text) {
    app.query = text;
    app.pick = 'all';
    goLibrary();
  }

  function pickFirst() {
    const first = found[0]?.items[0];
    if (inBrowser) {
      // the browser's search filters its cards as it is; one machine named is shown beside them
      if (first && found[0].label === 'Machines' && found[0].items.length === 1) first.go();
      open = false;
      return;
    }
    if (first && found[0].label === 'Tables' && found[0].items.length === 1) first.go();
    open = false;
    goLibrary();
  }

  function clickAway(e) {
    if (box && !box.contains(e.target)) open = false;
  }
</script>

<svelte:window onclick={clickAway} />

<header>
  <button type="button" class="brand" onclick={goHome} aria-label="Visual Pinball Infinite: the library">
    <svg width="28" height="28" viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="logo-neon" gradientUnits="userSpaceOnUse" x1="12" y1="0" x2="52" y2="0"><stop offset="0" stop-color="#b48cff"></stop><stop offset="0.5" stop-color="#7fa8ff"></stop><stop offset="1" stop-color="#4cc2ff"></stop></linearGradient><filter id="logo-glow" x="-30%" y="-60%" width="160%" height="220%"><feGaussianBlur stdDeviation="2.2"></feGaussianBlur></filter><linearGradient id="logo-ringNeon" gradientUnits="userSpaceOnUse" x1="3" y1="0" x2="61" y2="0"><stop offset="0" stop-color="#b48cff"></stop><stop offset="0.5" stop-color="#7fa8ff"></stop><stop offset="1" stop-color="#4cc2ff"></stop></linearGradient><filter id="logo-ringGlow" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="1.2"></feGaussianBlur></filter></defs><circle cx="32" cy="32" r="26" fill="#0d1016"></circle><circle cx="32" cy="32" r="26" fill="none" stroke="url(#logo-ringNeon)" stroke-width="6.07" opacity="0.5" filter="url(#logo-ringGlow)"></circle><circle cx="32" cy="32" r="26" fill="none" stroke="url(#logo-ringNeon)" stroke-width="4.57"></circle><circle cx="32" cy="32" r="26" fill="none" stroke="#ffffff" stroke-width="1.1" opacity="0.4"></circle><g transform="translate(32 32) scale(0.86) translate(-32 -38)"><path d="M32 38 C38 28 52 28 52 38 C52 48 38 48 32 38 C26 28 12 28 12 38 C12 48 26 48 32 38Z" fill="none" stroke="url(#logo-neon)" stroke-width="9" stroke-linejoin="round" opacity="0.55" filter="url(#logo-glow)"></path><path d="M32 38 C38 28 52 28 52 38 C52 48 38 48 32 38 C26 28 12 28 12 38 C12 48 26 48 32 38Z" fill="none" stroke="url(#logo-neon)" stroke-width="5.6" stroke-linejoin="round"></path><path d="M32 38 C38 28 52 28 52 38 C52 48 38 48 32 38 C26 28 12 28 12 38 C12 48 26 48 32 38Z" fill="none" stroke="#ffffff" stroke-width="1.5" stroke-linejoin="round" opacity="0.55"></path></g></svg>
    <span>Visual Pinball <span class="accent">Infinite</span></span>
  </button>
  <nav aria-label="Sections">
    <button type="button" class:current={app.page === 'library' || app.page === 'table'} aria-current={app.page === 'library' ? 'page' : undefined} onclick={goHome}>Library</button>
    <button type="button" class:current={app.page === 'browser'} aria-current={app.page === 'browser' ? 'page' : undefined} onclick={go('browser')}>VPS browser</button>
    <button type="button" class:current={app.page === 'tools'} aria-current={app.page === 'tools' ? 'page' : undefined} onclick={go('tools')}>Tools</button>
    <button type="button" class:current={app.page === 'editor'} aria-current={app.page === 'editor' ? 'page' : undefined} onclick={() => (app.page = 'editor')}>Table editor</button>
    <button type="button" class:current={app.page === 'hub'} aria-current={app.page === 'hub' ? 'page' : undefined} onclick={go('hub')}>Player hub</button>
  </nav>
  <div class="search" bind:this={box}>
    <label>
      <Icon name="search" />
      <input
        type="search"
        aria-label={inBrowser ? 'Search VPS' : 'Search your tables'}
        placeholder={inBrowser ? 'Search VPS: machines, makers, people, ROMs' : 'Search tables, makers, people, ROMs, files'}
        value={inBrowser ? vps.query : app.query}
        onfocus={() => (open = true)}
        oninput={(e) => {
          open = true;
          if (inBrowser) {
            vps.query = e.currentTarget.value;
            return;
          }
          app.query = e.currentTarget.value;
          if (app.page !== 'library' && app.page !== 'table') app.page = 'library';
        }}
        onkeydown={(e) => {
          if (e.key === 'Enter') pickFirst();
          if (e.key === 'Escape') open = false;
        }}
      />
    </label>
    {#if open && found.length}
      <div class="results" role="listbox" aria-label="What matches">
        {#each found as g (g.label)}
          <div class="group">
            <div class="section-title">{g.label}</div>
            {#each g.items as item, i (g.label + i)}
              <button
                type="button"
                class="hit"
                onclick={() => {
                  item.go();
                  open = false;
                }}
              >
                <span class="text">{item.text}</span>
                <span class="sub">{item.sub}</span>
              </button>
            {/each}
          </div>
        {/each}
      </div>
    {/if}
  </div>
  <div class="actions">
    {#if app.tasks.length}
      <div class="tasks" role="status" tabindex="0" aria-label="In the background: {app.tasks.map((t) => t.label).join(', ')}">
        <span class="tname">{barName}</span>
        <span class="tbar" class:unsure={overall === null}><span style="width: {overall === null ? 40 : Math.max(4, overall * 100)}%"></span></span>
        <div class="pop">
          <div class="section-title">In the background</div>
          <!-- the first five, the rest counted -->
          {#each app.tasks.slice(0, 5) as task (task.id)}
            {@const at = task.steps.findIndex((x) => !x.done)}
            <div class="task" class:waiting={task.waiting}>
              <span>{task.label}</span>
              {#if task.waiting}
                <span class="n wait">Waiting its turn</span>
              {:else}
                <span class="tbar" class:unsure={!task.of}><span style="width: {task.of ? Math.max(4, (100 * task.done) / task.of) : 40}%"></span></span>
                {#if task.of}<span class="n">{task.done} of {task.of}</span>{/if}
              {/if}
              {#if task.steps.length}
                <ol class="steps">
                  {#each task.steps as step, i (i)}
                    <li class:done={step.done} class:now={i === at}>
                      <span class="mark">{#if step.done}<Icon name="check" size={12} width={2.8} />{:else if i === at}<span class="dot"></span>{/if}</span>{step.label}
                    </li>
                  {/each}
                </ol>
              {/if}
            </div>
          {/each}
          {#if app.tasks.length > 5}<div class="more-tasks">+ {app.tasks.length - 5} more</div>{/if}
        </div>
      </div>
    {/if}
    <button type="button" class="btn" onclick={() => (drop.open = true)}><Icon name="plus" />Add tables</button>
    <button type="button" class="btn vpidb" title={vpidbTitle} onclick={() => leaving(() => { app.settingsPage = 'general'; app.page = 'settings'; setTimeout(() => document.getElementById('vpidb-settings')?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 120); })}>
      <Icon name="trophy" size={17} />VPIDB
      {#if vpidb?.connected}<span class="mark" class:bad={vpidb.ok === false} aria-label={vpidb.ok === false ? 'not syncing' : 'connected'}><Icon name={vpidb.ok === false ? 'x' : 'check'} size={11} width={3.2} /></span>{/if}
    </button>
    <button type="button" class="btn" onclick={() => leaving(() => (app.cabinet = true))}><Icon name="tv" />Cabinet mode</button>
    {#if app.fullscreen}
      <!-- full screen hides the window's own frame, its close button with it -->
      <button type="button" class="btn" title="Leave full screen (F11)" onclick={toggleFullscreen}><Icon name="shrink" size={17} />Exit full screen</button>
    {:else}
      <button type="button" class="btn icon" aria-label="Full screen" title="Full screen (F11)" onclick={toggleFullscreen}><Icon name="expand" size={19} /></button>
    {/if}
    <button type="button" class="btn icon" aria-label="Report a problem" title="Report a problem" onclick={() => (app.report = { table: null })}><Icon name="bug" size={19} /></button>
    <button type="button" class="btn icon" aria-label="Settings" class:on={app.page === 'settings'} onclick={go('settings')}><Icon name="sliders" size={19} /></button>
    {#if app.fullscreen}<button type="button" class="btn icon" aria-label="Close the app" title="Close the app" onclick={() => leaving(() => invoke('quit'), { closing: true })}><Icon name="x" size={19} /></button>{/if}
  </div>
</header>

<style>
  .vpidb { position: relative; }
  .vpidb .mark { display: grid; place-items: center; width: 17px; height: 17px; border-radius: 50%; background: var(--good); color: #0b1410; }
  .vpidb .mark.bad { background: var(--bad); color: #fff; }
  header {
    position: sticky; top: 0; z-index: 20;
    display: flex; flex-wrap: wrap; align-items: center; gap: 12px 20px; padding: 10px 20px;
    background: var(--bar); border-bottom: 1px solid var(--line);
  }
  .brand { display: flex; align-items: center; gap: 10px; padding: 0; border: 0; background: none; font-weight: 700; font-size: 16px; letter-spacing: 0.01em; }
  .accent { color: var(--accent); }
  nav { display: flex; gap: 4px; }
  nav button { padding: 8px 14px; border: 0; border-radius: 8px; background: transparent; color: var(--muted); font-weight: 500; font-size: 14px; }
  nav button:hover { background: var(--hover); }
  nav button.current { background: #1f2532; color: var(--fg); font-weight: 600; }
  .search { position: relative; flex: 1 1 260px; max-width: 520px; }
  .search label { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 40px; border-radius: 10px; background: var(--bg); border: 1px solid #2a3242; color: var(--faint); }
  .search input { flex: 1; min-width: 0; background: transparent; border: 0; outline: none; color: var(--fg); font-size: 14px; }
  .results {
    position: absolute; top: 46px; left: 0; right: 0; z-index: 30; max-height: 70vh; overflow: auto;
    padding: 8px; border-radius: 12px; background: #151a24; border: 1px solid #2c3445; box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55);
    display: flex; flex-direction: column; gap: 6px;
  }
  .group { display: flex; flex-direction: column; gap: 2px; }
  .group .section-title { padding: 6px 10px 2px; }
  .hit { display: flex; flex-direction: column; align-items: flex-start; gap: 1px; padding: 7px 10px; border: 0; border-radius: 8px; background: transparent; text-align: left; }
  .hit:hover, .hit:focus-visible { background: var(--selected); }
  .hit .text { font-weight: 600; }
  .hit .sub { font-size: 12px; color: var(--faint); }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-left: auto; }
  .tasks { position: relative; display: flex; flex-direction: column; justify-content: center; gap: 5px; width: 190px; height: 40px; padding: 0 12px; border-radius: 10px; background: var(--control); border: 1px solid var(--control-line); cursor: default; }
  .tname { font-size: 12px; color: var(--fg-soft); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tbar { position: relative; display: block; height: 4px; border-radius: 999px; background: #1d3554; overflow: hidden; }
  .tbar span { display: block; height: 100%; background: var(--accent); transition: width 0.3s ease; }
  .tbar.unsure span { position: absolute; animation: unsure 1.4s ease-in-out infinite; }
  @keyframes unsure { from { left: -40%; } to { left: 100%; } }
  @media (prefers-reduced-motion: reduce) { .tbar.unsure span { animation: none; left: 30%; } }
  .pop {
    display: none; position: absolute; top: 46px; right: 0; z-index: 30; width: 320px; padding: 10px 12px; border-radius: 12px;
    background: #151a24; border: 1px solid #2c3445; box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55); flex-direction: column; gap: 10px;
  }
  .tasks:hover .pop, .tasks:focus-within .pop { display: flex; }
  /* the gap between the bar and its list, kept pointed at */
  .pop::before { content: ''; position: absolute; left: 0; right: 0; top: -8px; height: 8px; }
  .task { display: grid; grid-template-columns: 1fr auto; gap: 4px 10px; align-items: center; font-size: 13px; color: var(--fg-soft); }
  .task .tbar { grid-column: 1; }
  .task .n { grid-column: 2; grid-row: 2; font-size: 11px; color: var(--faint); }
  .task .n.wait { grid-column: 1 / -1; grid-row: auto; }
  .task.waiting > span:first-child { color: var(--muted); }
  .more-tasks { font-size: 12px; color: var(--faint); padding: 2px 0; }
  .steps { grid-column: 1 / -1; list-style: none; margin: 2px 0 0; padding: 0; display: flex; flex-direction: column; gap: 3px; }
  .steps li { display: flex; align-items: center; gap: 7px; font-size: 12px; color: var(--faint); }
  .steps li.done { color: var(--fg-soft); }
  .steps li.now { color: var(--fg); font-weight: 600; }
  .steps .mark { display: inline-flex; align-items: center; justify-content: center; width: 14px; height: 14px; color: var(--good); }
  .steps .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); animation: beat 1.2s ease-in-out infinite; }
  @keyframes beat { 50% { opacity: 0.35; } }
  @media (prefers-reduced-motion: reduce) { .steps .dot { animation: none; } }
</style>
