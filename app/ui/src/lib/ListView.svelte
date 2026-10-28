<script>
  // The library as a list: every file each table holds at a glance; a column's heading sorts by it.
  // A table that needs a look shows the tiles' triangle beside its name. Columns: those the user
  // chooses (the Columns button), its media by kind among them, kept with the preferences.
  import Icon from './Icon.svelte';
  import Stars from './Stars.svelte';
  import { app, play, problems, savePrefs } from './store.svelte.js';

  let { tables, open } = $props();

  let sortBy = $state(null);
  let down = $state(false);

  // a kind of media it has: a picture of it, or a video (but a playfield picture, which a video isn't)
  const hasMedia = (t, k) => t.media.some((m) => m.kind === k || (k !== 'playfield' && m.kind === `${k}-video`));
  const media = (id, label, full, kind) => ({ id, label, full, width: '74px', media: kind, get: (t) => hasMedia(t, kind) });

  // every column there is, each with its width; the name always shown
  const ALL = [
    { id: 'name', label: 'Table', full: 'Table', width: 'minmax(220px, 3fr)', get: (t) => t.name },
    { id: 'maker', label: 'Maker', full: 'Maker', width: 'minmax(80px, 1fr)', get: (t) => t.maker || '' },
    { id: 'year', label: 'Year', full: 'Year', width: '50px', get: (t) => t.year || 0 },
    { id: 'rating', label: 'Rating', full: 'Rating', width: '78px', get: (t) => t.rating || 0 },
    { id: 'version', label: 'Version', full: 'Version', width: 'minmax(60px, 0.8fr)', get: (t) => t.version || '' },
    { id: 'authors', label: 'Authors', full: 'Authors', width: 'minmax(120px, 2fr)', get: (t) => t.authors.join(', ') },
    { id: 'b2s', label: 'Backglass', full: 'Backglass file', width: '74px', get: (t) => !!t.holds.backglass },
    { id: 'rom', label: 'ROM', full: 'ROM', width: 'minmax(80px, 1fr)', get: (t) => (t.rom ? (t.holds.rom ? 2 : 1) : 0) },
    { id: 'pup', label: 'PinUP', full: 'PinUP pack', width: '54px', get: (t) => !!t.holds.pupPack },
    { id: 'alt', label: 'AltSound', full: 'AltSound', width: '64px', get: (t) => !!t.holds.altsound },
    { id: 'colour', label: 'Color', full: 'Color DMD', width: '90px', get: (t) => !!t.holds.colour },
    media('m-playfield', 'Playfield', 'Media: playfield picture', 'playfield'),
    media('m-video', 'PF video', 'Media: playfield video', 'playfield-video'),
    media('m-backglass', 'BG image', 'Media: backglass picture', 'backglass'),
    media('m-dmd', 'DMD image', 'Media: score display', 'dmd'),
    media('m-topper', 'Topper', 'Media: topper', 'topper'),
    media('m-wheel', 'Wheel', 'Media: wheel', 'wheel'),
    media('m-flyer', 'Flyer', 'Media: flyer', 'gameinfo'),
    media('m-card', 'Card', 'Media: instruction card', 'gamehelp'),
    { id: 'm-sound', label: 'Sound', full: 'Media: sound', width: '64px', media: 'sound', get: (t) => !!t.audio },
    { id: 'played', label: 'Played', full: 'Last played', width: '96px', get: (t) => t.played?.last || 0 },
  ];
  const SHOWN = ['name', 'maker', 'year', 'rating', 'version', 'authors', 'b2s', 'rom', 'pup', 'alt', 'colour', 'played'];
  const shownIds = $derived(app.prefs.listColumns ?? SHOWN);
  const columns = $derived(ALL.filter((c) => c.id === 'name' || shownIds.includes(c.id)));
  const template = $derived(['30px', ...columns.map((c) => c.width)].join(' '));

  let choosing = $state(false);
  function toggle(id, on) {
    const now = new Set(shownIds);
    if (on) now.add(id);
    else now.delete(id);
    app.prefs.listColumns = ALL.map((c) => c.id).filter((c) => now.has(c));
    savePrefs();
  }

  const rows = $derived.by(() => {
    if (!sortBy) return tables;
    const col = columns.find((c) => c.id === sortBy);
    if (!col) return tables;
    const sorted = [...tables].sort((a, b) => {
      const x = col.get(a);
      const y = col.get(b);
      return typeof x === 'string' ? x.localeCompare(y) : Number(x) - Number(y);
    });
    return down ? sorted.reverse() : sorted;
  });

  function sort(id) {
    if (sortBy === id) down = !down;
    else {
      sortBy = id;
      down = false;
    }
  }

  const when = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '');
</script>

<div class="cols">
  <button type="button" class="btn small ghost" aria-expanded={choosing} onclick={() => (choosing = !choosing)}><Icon name="list" size={14} />Columns</button>
  {#if choosing}
    <div class="chooser" role="group" aria-label="Columns shown">
      {#each ALL.filter((c) => c.id !== 'name') as c (c.id)}
        <label class="check"><input type="checkbox" checked={shownIds.includes(c.id)} onchange={(e) => toggle(c.id, e.currentTarget.checked)} />{c.full}</label>
      {/each}
    </div>
  {/if}
</div>
<div class="list" role="table" aria-label="Tables">
  <div class="row head" role="row" style:grid-template-columns={template}>
    <span role="columnheader" aria-hidden="true"></span>
    {#each columns as c (c.id)}
      <button type="button" role="columnheader" class="col {c.id}" title={c.full} aria-sort={sortBy === c.id ? (down ? 'descending' : 'ascending') : 'none'} onclick={() => sort(c.id)}>
        {c.label}{#if sortBy === c.id}<Icon name={down ? 'chevD' : 'chevR'} size={12} />{/if}
      </button>
    {/each}
  </div>
  {#each rows as t (t.id)}
    {@const issues = problems(t)}
    <div class="row" class:glow={app.glow === t.id} role="row" data-table={t.id} style:grid-template-columns={template}>
      <button type="button" class="playing" aria-label="Play {t.name}" title="Play" onclick={() => play(t)}><Icon name="play" size={14} fill /></button>
      {#each columns as c (c.id)}
        {#if c.id === 'name'}
          <button type="button" class="col name link" role="cell" onclick={() => open(t)}>
            <span class="n">{#if issues.length}<span class="issue" class:bad={issues.some((p) => p.look === 'bad')} title={issues.map((p) => p.text).join('\n')}><Icon name="alert" size={13} width={2.4} /></span>{/if}{t.name}</span>
            <span class="f">{t.file}</span>
          </button>
        {:else if c.id === 'maker'}<span class="col maker" role="cell">{t.maker || '—'}</span>
        {:else if c.id === 'year'}<span class="col year" role="cell">{t.year || ''}</span>
        {:else if c.id === 'rating'}<span class="col rating" role="cell">{#if t.rating}<Stars value={t.rating} size={12} />{/if}</span>
        {:else if c.id === 'version'}<span class="col version" role="cell">{t.version || ''}</span>
        {:else if c.id === 'authors'}<span class="col authors" role="cell" title={t.authors.join(', ')}>{t.authors.join(', ')}</span>
        {:else if c.id === 'b2s'}<span class="col b2s" role="cell">{#if t.holds.backglass}<Icon name="check" size={15} width={2.4} />{/if}</span>
        {:else if c.id === 'rom'}<span class="col rom" role="cell" class:missing={t.rom && !t.holds.rom}>{t.rom ? (t.holds.rom ? t.rom : `${t.rom} missing`) : ''}</span>
        {:else if c.id === 'pup'}<span class="col pup" role="cell">{#if t.holds.pupPack}<Icon name="check" size={15} width={2.4} />{/if}</span>
        {:else if c.id === 'alt'}<span class="col alt" role="cell">{#if t.holds.altsound}<Icon name="check" size={15} width={2.4} />{/if}</span>
        {:else if c.id === 'colour'}<span class="col colour" role="cell">{t.holds.colour ? t.holds.colour.kind : ''}</span>
        {:else if c.media}<span class="col media" role="cell" class:has={c.get(t)}>{#if c.get(t)}<Icon name="check" size={15} width={2.4} />{:else}<span class="none" title="None yet">—</span>{/if}</span>
        {:else if c.id === 'played'}<span class="col played" role="cell">{when(t.played?.last)}</span>
        {/if}
      {/each}
    </div>
  {/each}
</div>

<style>
  .list { border: 1px solid var(--panel-line); border-radius: 12px; overflow: hidden; background: var(--panel); }
  /* its columns, as chosen: their widths set on each row */
  .row { display: grid; align-items: center; gap: 0 12px; padding: 6px 14px; border-top: 1px solid var(--line-soft); }
  .cols { position: relative; display: flex; justify-content: flex-end; margin-bottom: 8px; }
  .chooser { position: absolute; right: 0; top: calc(100% + 6px); z-index: 10; display: grid; grid-template-columns: repeat(2, minmax(180px, auto)); gap: 6px 18px; padding: 14px 16px; border-radius: 12px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 18px 40px rgba(0, 0, 0, 0.5); }
  .check { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); white-space: nowrap; }
  .check input { accent-color: var(--accent); }
  .media.has { color: var(--good); }
  .media .none { color: var(--faint); }
  .row:first-child { border-top: 0; }
  .row:hover:not(.head) { background: #171d29; }
  .head { padding-top: 10px; padding-bottom: 10px; background: #111620; }
  .head button {
    display: inline-flex; align-items: center; gap: 4px; padding: 0; border: 0; background: none; text-align: left;
    color: var(--faint); font-size: 11px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase;
  }
  .col { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; color: var(--fg-soft); }
  .b2s, .pup, .alt { color: var(--good); }
  .rom.missing { color: var(--warn); }
  .link { display: flex; flex-direction: column; align-items: flex-start; padding: 2px 0; border: 0; background: none; text-align: left; }
  .link .n { font-weight: 600; font-size: 14px; color: var(--fg); max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  .link .f { font-size: 12px; color: var(--faint); max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  .link:hover .n { color: var(--accent); }
  .issue { display: inline-flex; vertical-align: -2px; margin-right: 6px; color: var(--warn); }
  .issue.bad { color: var(--bad); }
  .playing {
    width: 26px; height: 26px; border-radius: 50%; border: 0; display: inline-flex; align-items: center; justify-content: center;
    background: var(--play); color: var(--play-fg); opacity: 0.85;
  }
  .playing:hover { opacity: 1; background: var(--play-hover); }
  /* just added, the library scrolled to it: lit, then fading */
  .row.glow { animation: glow 4s ease-out forwards; }
  @keyframes glow {
    0%, 60% { background: var(--accent-bg); }
    100% { background: transparent; }
  }
</style>
