<script>
  // One table's tile: its playfield, backglass or wheel (its name drawn when there's none), its type,
  // name, maker and year, tags for the files it has, and a PLAY button when pointed at; a yellow or
  // red triangle at its top right when something needs a look, what in its tip.
  import Icon from './Icon.svelte';
  import { app, drop, play, problems, turnOf } from './store.svelte.js';

  let { t, picture, onopen } = $props();

  let failed = $state({});
  const src = $derived(t.picture[picture]);
  const showPicture = $derived(src && !failed[src]);
  // a playfield kept on its side, shown upright
  const turn = $derived(showPicture ? turnOf(t, src) : 0);

  const tags = $derived(
    [
      t.holds.backglass && 'B2S',
      t.holds.rom && 'ROM',
      t.holds.pupPack && 'PUP',
      t.holds.altsound && 'AltSound',
      t.holds.colour && 'Color',
    ].filter(Boolean),
  );
  // two tables of one name (two releases of a machine) told apart by their version and first author
  const twin = $derived(app.tables.some((x) => x.id !== t.id && x.name.toLowerCase() === t.name.toLowerCase()));
  const sub = $derived([t.maker, t.year, ...(twin ? [t.version, t.authors[0]] : [])].filter(Boolean).join(' · ') || 'Not on VPS');
  const issues = $derived(problems(t));
  const worst = $derived(issues.some((p) => p.look === 'bad') ? 'bad' : 'warn');
</script>

<div class="tile" class:dropover={drop.over === t.id} class:glow={app.glow === t.id} data-table={t.id}>
  <button type="button" class="open" onclick={onopen} aria-label="{t.name}, {sub}">
    <div class="picture {picture}" class:turn-frame={turn}>
      {#if showPicture}
        <img src={src} alt="" loading="lazy" decoding="async" class:turned={turn === 90 || turn === 270} class:ccw={turn === 270} class:upside={turn === 180} onerror={() => (failed = { ...failed, [src]: true })} />
      {:else}
        <div class="named"><span class="display">{t.short}</span></div>
      {/if}
      {#if t.badge}<span class="badge">{t.badge}</span>{/if}
      {#if issues.length}
        <span class="issue {worst}" role="img" aria-label="Needs a look: {issues.map((p) => p.text).join('; ')}" title={issues.map((p) => p.text).join('\n')}><Icon name="alert" size={15} width={2.4} /></span>
      {/if}
      {#if t.favourite}<span class="fav" class:shifted={issues.length} aria-label="A favourite"><Icon name="heart" size={14} fill /></span>{/if}
    </div>
    <div class="text">
      <div class="name">{t.name}</div>
      <div class="sub">{sub}</div>
      {#if t.rating}
        <div class="stars" role="img" aria-label="Rated {t.rating} of 5" title="Rated {t.rating} of 5">{#each [1, 2, 3, 4, 5] as n (n)}<span class:on={n <= t.rating}>★</span>{/each}</div>
      {/if}
      {#if tags.length}
        <div class="tags">
          {#each tags as g (g)}<span class="tag">{g}</span>{/each}
        </div>
      {/if}
    </div>
  </button>
  <div class="over {picture}">
    <button type="button" class="play" aria-label="Play {t.name}" title="Play" onclick={() => play(t)}>
      <Icon name="play" size={22} fill />
    </button>
  </div>
</div>

<style>
  .tile { position: relative; }
  .tile.dropover { outline: 3px solid var(--accent); outline-offset: 3px; border-radius: 12px; }
  /* just added, the library scrolled to it: lit, then fading */
  .tile.glow { border-radius: 12px; animation: glow 4s ease-out forwards; }
  @keyframes glow {
    0%, 60% { outline: 3px solid var(--accent); outline-offset: 3px; box-shadow: 0 0 28px rgba(79, 177, 255, 0.55); }
    100% { outline: 3px solid transparent; outline-offset: 3px; box-shadow: none; }
  }
  .open {
    display: flex; flex-direction: column; width: 100%; height: 100%; padding: 0; text-align: left;
    background: var(--card); border: 1px solid var(--card-line); border-radius: 12px; overflow: hidden; color: var(--fg);
    transition: transform 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
  }
  .tile:hover .open, .open:focus-visible { transform: translateY(-3px); border-color: var(--accent); box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5); }
  .picture { position: relative; background: var(--picture-bg); }
  .picture.playfield { aspect-ratio: 9 / 16; }
  .picture.backglass { aspect-ratio: 4 / 3; }
  .picture.wheel { aspect-ratio: 1 / 1; }
  img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .wheel img { object-fit: contain; padding: 10%; }
  .named {
    position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; padding: 14px; text-align: center;
    background: radial-gradient(circle at 50% 35%, #1b2a44 0%, #0f141d 70%);
  }
  .named span { font-size: 28px; line-height: 1.02; text-transform: uppercase; color: #dbe7ff; text-shadow: 0 2px 12px rgba(76, 194, 255, 0.35); }
  .badge {
    position: absolute; top: 8px; left: 8px; padding: 2px 7px; border-radius: 6px; background: rgba(8, 10, 15, 0.82);
    font-size: 11px; font-weight: 700; letter-spacing: 0.04em;
  }
  .fav { position: absolute; top: 8px; right: 8px; color: #ff7aa2; display: inline-flex; padding: 4px; border-radius: 6px; background: rgba(8, 10, 15, 0.7); }
  .fav.shifted { right: 42px; }
  .issue { position: absolute; top: 8px; right: 8px; z-index: 1; display: inline-flex; padding: 4px; border-radius: 6px; background: rgba(8, 10, 15, 0.82); }
  .issue.warn { color: var(--warn); }
  .issue.bad { color: var(--bad); }
  .text { padding: 12px 14px 14px; display: flex; flex-direction: column; gap: 5px; }
  .name { font-weight: 600; font-size: 15px; line-height: 1.3; }
  .sub { font-size: 13px; color: var(--muted); }
  .tags { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 5px; }
  .tag { padding: 2px 7px; border-radius: 5px; background: var(--tag-bg); color: var(--tag-fg); font-size: 11px; font-weight: 600; }
  /* PLAY over the picture, shown when the tile is pointed at */
  .over {
    position: absolute; left: 1px; right: 1px; top: 1px; border-radius: 11px 11px 0 0;
    display: flex; align-items: center; justify-content: center; pointer-events: none;
    transition: background 0.15s ease, transform 0.15s ease;
  }
  .over.playfield { aspect-ratio: 9 / 16; }
  .over.backglass { aspect-ratio: 4 / 3; }
  .over.wheel { aspect-ratio: 1 / 1; }
  .tile:hover .over { background: rgba(5, 8, 12, 0.35); transform: translateY(-3px); }
  .play {
    width: 54px; height: 54px; border-radius: 50%; border: 0; pointer-events: auto;
    display: inline-flex; align-items: center; justify-content: center;
    background: var(--play); color: var(--play-fg); box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
    opacity: 0; transition: opacity 0.15s ease;
  }
  .tile:hover .play, .play:focus-visible { opacity: 1; }
  .play:hover { background: var(--play-hover); }
  .stars { display: flex; gap: 1px; font-size: 13px; line-height: 1; color: #3a4152; letter-spacing: 0; }
  .stars .on { color: #ffc65c; }
</style>
