<script>
  // The VPS browser's Recommended: machines with a VPX table that aren't in the library, picked for
  // the user by what they play — the makers, eras, themes and designers of their favourites,
  // best-rated and most-played tables — and by what VPS shows of a machine: VPin Workshop's tables,
  // how many releases it has, a release this month, its ROM already in VPinMAME's folder with no table
  // of it; now and then a maker the library has none of. The list kept until New list picks again,
  // each pick with why.
  import { invoke } from '@tauri-apps/api/core';
  import { app, vps, oneEach, savePrefs } from './store.svelte.js';
  import Icon from './Icon.svelte';

  let { onpick, tile = 190 } = $props();

  const HOW_MANY = 24;
  const DAY = 86400000;
  const era = (y) => (!y ? null : y < 1970 ? 'before 1970' : y < 1980 ? '1970s' : y < 1990 ? '1980s' : y < 2000 ? '1990s' : '2000s');
  const vpw = (m) => m.authors.some((a) => /^vpw$|vpin ?workshop/i.test(a.trim()));

  // the ROM sets in VPinMAME's folder, by name
  let romsThere = $state(null);
  invoke('rom_names').then((r) => (romsThere = new Set(r))).catch(() => (romsThere = new Set()));

  const byId = $derived(new Map(vps.machines.map((m) => [m.id, m])));

  /** What the user plays, by maker, era, theme and designer: each liked table counting by how
   *  much — a favourite, its stars, its plays — and the liked table each stands for. */
  function profile() {
    const tables = oneEach(app.tables.filter((t) => !t.hiddenTable));
    const weigh = (t) => (t.favourite ? 3 : 0) + Math.max(0, (t.rating || 0) - 2) + Math.min(t.played?.count || 0, 9) / 3;
    const p = { maker: new Map(), era: new Map(), theme: new Map(), designer: new Map(), total: 0, makers: new Set(), eras: new Set() };
    const add = (map, key, w, t) => {
      if (!key) return;
      const was = map.get(key) || { w: 0, best: null, bw: 0 };
      was.w += w;
      if (w > was.bw) Object.assign(was, { best: t, bw: w });
      map.set(key, was);
    };
    for (const t of tables) {
      const m = t.onVps?.sure ? byId.get(t.onVps.game) : null;
      const maker = m?.maker || t.maker;
      const year = m?.year || t.year;
      if (maker) p.makers.add(maker);
      if (year) p.eras.add(era(year));
      const w = weigh(t);
      if (!w) continue;
      p.total += w;
      add(p.maker, maker, w, t);
      add(p.era, era(year), w, t);
      for (const th of m?.theme || t.theme || []) add(p.theme, th, w, t);
      for (const d of m?.designers || t.designers || []) add(p.designer, d, w, t);
    }
    return p;
  }

  /** A machine's score for the user, and why, the strongest reasons first. */
  function score(m, p, now) {
    const why = [];
    let s = 0;
    const share = (map, key) => (p.total ? (map.get(key)?.w || 0) / p.total : 0);
    const mk = share(p.maker, m.maker);
    const er = share(p.era, era(m.year));
    s += 4 * mk + 2 * er;
    if (mk >= 0.12) {
      const like = p.maker.get(m.maker).best;
      why.push([3 * mk + 1, `Like ${like.short}: ${m.maker}${er >= 0.12 && era(like.year) === era(m.year) ? `, ${era(m.year)}` : ''}`]);
    } else if (er >= 0.2) why.push([er, `From the ${era(m.year)}, as most of what you play`]);
    const themes = m.theme.filter((th) => share(p.theme, th) >= 0.1);
    if (themes.length) {
      s += Math.min(2, themes.reduce((a, th) => a + 1.5 * share(p.theme, th), 0));
      why.push([1, `${themes.slice(0, 2).join(' and ')}, as ${p.theme.get(themes[0]).best.short}`]);
    }
    const designer = m.designers.find((d) => p.designer.has(d));
    if (designer) {
      s += 2;
      why.push([2.5, `Designed by ${designer}, as ${p.designer.get(designer).best.short}`]);
    }
    if (vpw(m)) {
      s += 1.5;
      why.push([1.5, 'A VPin Workshop table']);
    }
    s += 0.35 * Math.min(Math.log2(1 + (m.has.tables || m.releases)), 5);
    if ((m.has.tables || 0) >= 6) why.push([0.6, `${m.has.tables} table releases on VPS`]);
    if (m.newest && now - m.newest < 45 * DAY) {
      s += 1.2;
      why.push([1.2, 'A new release on VPS this month']);
    }
    if (romsThere && m.roms.some((r) => romsThere.has(r.toLowerCase()))) {
      s += 3;
      why.push([4, 'You have its ROM']);
    }
    s += 0.2 * ['pupPacks', 'colour', 'altSound'].filter((k) => m.has[k] > 0).length;
    if (m.maker && !p.makers.has(m.maker)) {
      s += 0.3;
      why.push([0.4, `A maker you have none of: ${m.maker}`]);
    }
    return { s, why: why.sort((a, b) => b[0] - a[0]).map(([, w]) => w) };
  }

  /** A new list: the best-scoring machines shuffled a little, at most three of one maker. */
  function pick() {
    const p = profile();
    const now = Date.now();
    const scored = vps.machines
      .filter((m) => m.formats.includes('VPX') && !m.mine)
      .map((m) => ({ m, ...score(m, p, now) }))
      .sort((a, b) => b.s - a.s)
      .slice(0, 160)
      .map((x) => ({ ...x, key: x.s + Math.random() * 1.6 }))
      .sort((a, b) => b.key - a.key);
    const out = [];
    const perMaker = new Map();
    for (const x of scored) {
      const n = perMaker.get(x.m.maker) || 0;
      if (n >= 3) continue;
      perMaker.set(x.m.maker, n + 1);
      out.push({ id: x.m.id, why: x.why.slice(0, 2) });
      if (out.length >= HOW_MANY) break;
    }
    app.prefs.vpsPicks = { made: now, picks: out };
    savePrefs();
  }

  // the list kept, those since added to the library left out; the first made once the library and
  // VPS's list are read
  const picks = $derived((app.prefs.vpsPicks?.picks || []).map((x) => ({ ...x, m: byId.get(x.id) })).filter((x) => x.m && !x.m.mine));
  $effect(() => {
    if (!app.prefs.vpsPicks && app.loaded && vps.machines.length && romsThere) pick();
  });

  let failed = $state({});
  const sub = (m) => [m.maker, m.year].filter(Boolean).join(' · ');
  const date = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'long' }) : '');
</script>

<section class="picks">
  <div class="top">
    <p class="intro">
      Machines you don’t have, picked by what you play — the makers, eras, themes and designers of your favourites, best-rated and most-played tables — and by what VPS shows: VPin Workshop’s tables, many releases, a new one this month, a ROM you have with no table.{app.prefs.vpsPicks?.made ? ` Picked ${date(app.prefs.vpsPicks.made)}.` : ''}
    </p>
    <button type="button" class="btn" onclick={pick} disabled={!vps.machines.length}><Icon name="refresh" size={15} />New list</button>
  </div>
  <div class="grid" style="--tile: {tile}px">
    {#each picks as x (x.id)}
      <button type="button" class="card" class:on={x.id === vps.sel} onclick={() => onpick(x.id)} aria-label="{x.m.name}, {sub(x.m)}" aria-pressed={x.id === vps.sel}>
        <div class="picture">
          {#if x.m.picture && !failed[x.m.picture]}
            <img src={x.m.picture} alt="" loading="lazy" decoding="async" onerror={() => (failed = { ...failed, [x.m.picture]: true })} />
          {:else}
            <div class="named"><span class="display">{x.m.name}</span></div>
          {/if}
          {#if x.m.badge}<span class="badge">{x.m.badge}</span>{/if}
        </div>
        <div class="text">
          <div class="name">{x.m.name}</div>
          <div class="msub">{sub(x.m) || 'Maker unknown'}</div>
          {#each x.why as w (w)}<div class="why">{w}</div>{/each}
        </div>
      </button>
    {/each}
  </div>
</section>

<style>
  .picks { display: flex; flex-direction: column; gap: 14px; }
  .top { display: flex; flex-wrap: wrap; align-items: flex-start; gap: 12px 20px; }
  .intro { flex: 1 1 420px; margin: 0; color: var(--fg-soft); font-size: 13.5px; line-height: 1.55; max-width: 760px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile), 1fr)); gap: 18px; }
  .card {
    display: flex; flex-direction: column; padding: 0; text-align: left; color: var(--fg);
    background: var(--card); border: 1px solid var(--card-line); border-radius: 12px; overflow: hidden;
    transition: transform 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
  }
  .card:hover, .card:focus-visible { transform: translateY(-3px); border-color: var(--accent); box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5); }
  .card.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .picture { position: relative; aspect-ratio: 9 / 16; min-height: 0; overflow: hidden; background: var(--picture-bg); }
  .picture img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .named {
    position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; padding: 14px; text-align: center;
    background: radial-gradient(circle at 50% 35%, #1b2a44 0%, #0f141d 70%);
  }
  .named span { font-size: 24px; line-height: 1.02; text-transform: uppercase; color: #dbe7ff; text-shadow: 0 2px 12px rgba(76, 194, 255, 0.35); }
  .badge { position: absolute; top: 8px; left: 8px; padding: 2px 7px; border-radius: 6px; background: rgba(8, 10, 15, 0.82); font-size: 11px; font-weight: 700; letter-spacing: 0.04em; }
  .text { padding: 11px 13px 13px; display: flex; flex-direction: column; gap: 4px; }
  .name { font-weight: 600; font-size: 14px; line-height: 1.3; }
  .msub { font-size: 12px; color: var(--muted); }
  .why { font-size: 12px; color: var(--accent); line-height: 1.35; }
</style>
