<script>
  // The VPS browser's For your tables: the library against VPS's list — for each kind of file, how
  // many of the user's tables have one, out of those VPS lists one for — and, kind by kind, the
  // tables without one VPS lists, each opening its machine beside the list at that kind's files.
  import { app, vps, oneEach } from './store.svelte.js';

  /** `onpick(gameId, kind)`: the table's machine shown in the panel, that kind's files open. */
  let { onpick } = $props();

  const media = (t, kinds) => t.media.some((m) => kinds.includes(m.kind) && m.from !== 'VPS');
  // each kind: what VPS calls it, whether a table has one, and which tables count (a ROM machine's
  // own kinds only for a table that drives PinMAME)
  const KINDS = [
    { kind: 'backglasses', label: 'Backglasses', one: 'a backglass', has: (t) => !!t.holds.backglass || t.holds.backglasses.length > 0 },
    { kind: 'roms', label: 'ROMs', one: 'its ROM', has: (t) => !!t.holds.rom, only: (t) => !!t.rom },
    { kind: 'pupPacks', label: 'PinUP packs', one: 'a PinUP pack', has: (t) => !!t.holds.pupPack },
    { kind: 'altSound', label: 'AltSound', one: 'an AltSound pack', has: (t) => !!t.holds.altsound, only: (t) => !!t.rom },
    { kind: 'colour', label: 'Color DMDs', one: 'a color DMD', has: (t) => !!t.holds.colour },
    { kind: 'pov', label: 'POV files', one: 'a POV file', has: (t) => !!t.holds.pov },
    { kind: 'wheels', label: 'Wheels', one: 'a wheel', has: (t) => media(t, ['wheel']) },
    { kind: 'toppers', label: 'Toppers', one: 'a topper', has: (t) => media(t, ['topper', 'topper-video']) },
  ];

  // the library's tables on VPS, each table once (its builds as one)
  const mine = $derived(oneEach(app.tables.filter((t) => !t.hiddenTable)));
  const onVps = $derived(mine.filter((t) => t.onVps?.sure && t.onVps?.game));
  const games = $derived(new Set(onVps.map((t) => t.onVps.game)).size);
  const vpxMachines = $derived(vps.machines.filter((m) => m.formats.includes('VPX')).length);

  const counts = $derived(
    KINDS.map((k) => {
      const listed = onVps.filter((t) => (t.vpsHas?.[k.kind] || 0) > 0 && (!k.only || k.only(t)));
      const missing = listed.filter((t) => !k.has(t)).sort((a, b) => a.name.localeCompare(b.name));
      return { ...k, listed: listed.length, have: listed.length - missing.length, missing };
    }),
  );

  // the kind whose tables show: the user's pick, else the first with tables to get
  let chosen = $state(null);
  const shown = $derived(counts.find((c) => c.kind === chosen) || counts.find((c) => c.missing.length) || counts[0]);

  let failed = $state({});
  const picture = (t) => t.picture?.playfield || t.picture?.wheel || t.picture?.backglass;
</script>

<section class="yours">
  <p class="intro">
    {#if onVps.length}
      Your library against VPS’s list: {onVps.length.toLocaleString()} of your {mine.length.toLocaleString()} tables are on VPS, {games.toLocaleString()} of the {vpxMachines.toLocaleString()} machines it lists with a VPX table. Each count is how many of your tables have that kind of file, out of those VPS lists one for.
    {:else if !app.loaded}
      Reading your library…
    {:else}
      None of your tables is matched to VPS yet: a table’s page finds it on VPS.
    {/if}
  </p>

  <div class="cards">
    {#each counts as c (c.kind)}
      <button type="button" class="count" class:on={c.kind === shown?.kind} aria-pressed={c.kind === shown?.kind} disabled={!c.listed} onclick={() => (chosen = c.kind)}>
        <span class="label">{c.label}</span>
        <span class="n"><b>{c.have}</b>/{c.listed}</span>
        <span class="bar"><span style="width: {c.listed ? (100 * c.have) / c.listed : 0}%"></span></span>
      </button>
    {/each}
  </div>

  {#if shown}
    <h2 class="display">To get: {shown.label.toLowerCase()}</h2>
    {#if shown.missing.length}
      <p class="quiet">{shown.missing.length} of your tables {shown.missing.length === 1 ? 'has' : 'have'} no {shown.one.replace(/^(a|an|its) /, '')} where VPS lists one. Choose one to see VPS’s beside the list, and Get to open its page.</p>
      <div class="rows">
        {#each shown.missing as t (t.id)}
          <button type="button" class="row" class:on={vps.sel === t.onVps.game} onclick={() => onpick(t.onVps.game, shown.kind)}>
            <span class="thumb">
              {#if picture(t) && !failed[picture(t)]}
                <img src={picture(t)} alt="" loading="lazy" onerror={() => (failed = { ...failed, [picture(t)]: true })} />
              {/if}
            </span>
            <span class="words">
              <span class="name">{t.name}</span>
              <span class="sub">{[t.maker, t.year].filter(Boolean).join(' · ')}</span>
            </span>
            <span class="listed">VPS lists {t.vpsHas[shown.kind]}</span>
          </button>
        {/each}
      </div>
    {:else if shown.listed}
      <p class="quiet">Every one of your tables VPS lists {shown.one.replace(/^its /, 'a ')} for has one.</p>
    {/if}
  {/if}
</section>

<style>
  .yours { display: flex; flex-direction: column; gap: 14px; }
  .intro { margin: 0; color: var(--fg-soft); font-size: 13.5px; line-height: 1.55; max-width: 760px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
  .count {
    display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; text-align: left; color: var(--fg);
    background: var(--card); border: 1px solid var(--card-line); border-radius: 12px;
  }
  .count:hover:not(:disabled), .count:focus-visible { border-color: var(--accent); }
  .count.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .count:disabled { opacity: 0.5; }
  .label { font-size: 11px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; color: var(--fg-soft); }
  .n { font-size: 15px; color: var(--muted); }
  .n b { font-size: 26px; font-weight: 600; color: var(--fg); }
  .bar { height: 5px; border-radius: 3px; background: var(--control-line); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--good); border-radius: 3px; }
  h2 { margin: 8px 0 0; font-size: 24px; }
  .quiet { margin: 0; color: var(--muted); font-size: 13px; }
  .rows { display: flex; flex-direction: column; gap: 6px; }
  .row {
    display: flex; align-items: center; gap: 12px; padding: 8px 12px 8px 8px; text-align: left; color: var(--fg);
    background: var(--card); border: 1px solid var(--card-line); border-radius: 10px;
  }
  .row:hover, .row:focus-visible { border-color: var(--accent); }
  .row.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .thumb { flex: 0 0 auto; width: 34px; height: 60px; border-radius: 6px; overflow: hidden; background: var(--picture-bg); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .words { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .name { font-weight: 600; font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { color: var(--muted); font-size: 12px; }
  .listed { flex: 0 0 auto; color: var(--faint); font-size: 12px; }
</style>
