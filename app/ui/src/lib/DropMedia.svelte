<script>
  // A download's pictures, videos and sounds that nothing in their names placed, in a box of their
  // own under its files, folded to begin with: a card each with its picture (or its video, or a
  // sound's mark), its size and shape, the app's guess of where it goes and why — its name says it,
  // or its shape alone (to check) — and a place to choose, the guess chosen to begin with, else the
  // table's own media (beside the table, shown on its page). Its picture opens larger, its places
  // beside it. Every card placed at once from the box's head.
  import Icon from './Icon.svelte';
  import { drop } from './store.svelte.js';

  let { d, i } = $props();

  let open = $state(false);
  let large = $state(null);

  const cards = $derived(d.rows.map((r, k) => [r, k]).filter(([r]) => r.media));
  const choice = $derived(drop.choices[i]);
  // the places a media card can go: the front end's screens, the table's screenshots, its own media
  const places = $derived(d.places.filter(([key]) => key.startsWith('media:') || key === 'screenshots' || key === 'tableMedia'));
  const label = (key) => places.find(([k]) => k === key)?.[1] || '';
  const where = (k) => (choice?.include[k] ? choice.put[k] || 'tableMedia' : '');
  // the card's file named in its address with its download, so a later drop's card at the same
  // place shows its own picture, never the one before
  const url = (r) => `${d.mediaBase}${r.media.step}?k=${encodeURIComponent(`${d.mediaKey}-${r.name}-${r.size}`)}`;

  const WHAT = {
    wheel: 'a wheel', playfield: 'the playfield', backglass: 'the backglass', topper: 'the topper', dmd: 'the score display', fulldmd: 'a full DMD',
    loading: 'a loading video', flyer: 'a flyer', card: 'an instruction card', audio: 'its sound', launch: 'a launch sound', screenshot: 'a screenshot',
  };
  const why = (m) => {
    if (m.guess && m.why === 'name') return ['guess', 'Our guess', `Its name says ${WHAT[m.what] || m.what}`];
    if (m.guess && m.why === 'like') return ['check', 'Check this', `It looks like the table’s own ${m.what || 'backglass'}`];
    if (m.guess) return ['check', 'Check this', `Its shape looks like ${WHAT[m.what] || m.what}`];
    if (m.what) return ['none', 'No place for it', `Looks like ${WHAT[m.what] || m.what}, which your front end has no place for`];
    return ['none', 'Not told', 'Nothing in its name or shape says what it is'];
  };

  function place(k, key) {
    if (key) {
      choice.put[k] = key;
      choice.include[k] = true;
    } else {
      choice.include[k] = false;
    }
  }

  function placeAll(key) {
    if (key === '') return;
    for (const [, k] of cards) place(k, key === 'out' ? '' : key);
  }

  // what Add does with them, in words
  const counts = $derived.by(() => {
    let [guessed, own, out] = [0, 0, 0];
    for (const [, k] of cards) {
      const w = where(k);
      if (!w) out++;
      else if (w === 'tableMedia') own++;
      else guessed++;
    }
    return { guessed, own, out };
  });
  const summary = $derived(
    [counts.guessed ? `${counts.guessed} placed` : '', counts.own ? `${counts.own} to the table’s media` : '', counts.out ? `${counts.out} left out` : ''].filter(Boolean).join(', '),
  );
  const shape = (m) => (m.width && m.height ? `${m.width} × ${m.height}` : '');
  const size = (b) => (b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

  function key(e) {
    if (large !== null && e.key === 'Escape') large = null;
  }
</script>

<svelte:window onkeydown={key} />

<section class="media" aria-label="Its pictures, videos and sounds">
  <button type="button" class="head" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name={open ? 'chevD' : 'chevR'} size={16} />
    <b>Media</b>
    <span>{cards.length} {cards.length === 1 ? 'file' : 'files'} nothing in their names placed · {summary}</span>
  </button>
  {#if open}
    <div class="all">
      <label>
        <span>Place all as</span>
        <select class="select small" value="" onchange={(e) => { placeAll(e.currentTarget.value); e.currentTarget.value = ''; }}>
          <option value="">Choose…</option>
          {#each places as [k, l] (k)}<option value={k}>{l}</option>{/each}
          <option value="out">Leave them all out</option>
        </select>
      </label>
      <span class="soft">The table’s media go beside the table, in a folder named for it, and show on its page.</span>
    </div>
    <div class="cards">
      {#each cards as [r, k] (k)}
        {@const m = r.media}
        {@const [look, chip, tip] = why(m)}
        <div class="card" class:out={!where(k)}>
          <button type="button" class="pic" aria-label="See {r.name} larger" onclick={() => (large = k)}>
            {#if m.kind === 'picture'}
              <img src={url(r)} alt="" loading="lazy" decoding="async" />
            {:else if m.kind === 'video'}
              <video src={url(r)} muted preload="metadata"></video>
            {:else}
              <span class="sound"><Icon name="music" size={28} /></span>
            {/if}
            {#if shape(m)}<span class="shape">{shape(m)}</span>{/if}
          </button>
          <div class="body">
            <div class="name" title={r.name}>{r.name}</div>
            <div class="meta">{r.label} · {size(r.size)}</div>
            <div class="why" title={tip}><i class={look}>{chip}</i><span>{tip}</span></div>
            <select class="select small" aria-label="Where {r.name} goes" value={where(k)} onchange={(e) => place(k, e.currentTarget.value)}>
              {#each places as [pk, pl] (pk)}<option value={pk}>{pl}</option>{/each}
              <option value="">Leave it out</option>
            </select>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

{#if large !== null}
  {@const r = d.rows[large]}
  <div class="preview" role="presentation" onclick={(e) => e.target === e.currentTarget && (large = null)}>
    <div class="sheet" role="dialog" aria-modal="true" aria-label={r.name}>
      <div class="big">
        {#if r.media.kind === 'picture'}
          <img src={url(r)} alt={r.name} />
        {:else if r.media.kind === 'video'}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video src={url(r)} controls autoplay muted loop></video>
        {:else}
          <audio src={url(r)} controls></audio>
        {/if}
      </div>
      <div class="side">
        <h3>{r.name}</h3>
        <p class="soft">{[r.label, shape(r.media), size(r.size)].filter(Boolean).join(' · ')}</p>
        <p class="soft">{why(r.media)[2]}.</p>
        <span class="eyebrow">Place it as</span>
        <div class="places">
          {#each places as [pk, pl] (pk)}
            <button type="button" aria-pressed={where(large) === pk} onclick={() => place(large, pk)}>{pl.replace(/^PinUP Popper: |^PinballX: |^PinballY: /, '')}</button>
          {/each}
          <button type="button" aria-pressed={where(large) === ''} onclick={() => place(large, '')}>Leave it out</button>
        </div>
        <p class="soft">{where(large) ? `Goes to ${label(where(large)).replace(/^The /, 'the ')}.` : 'Not copied: it stays in the download.'}</p>
        <button type="button" class="btn small ghost" onclick={() => (large = null)}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .media { margin-top: 12px; border: 1px solid var(--panel-line); border-radius: 12px; background: rgba(13, 16, 22, 0.5); }
  .head { display: flex; align-items: center; gap: 8px; width: 100%; padding: 10px 12px; background: none; border: 0; color: var(--fg); font: inherit; text-align: left; cursor: pointer; border-radius: 12px; }
  .head:hover { background: rgba(255, 255, 255, 0.03); }
  .head span { color: var(--muted); font-size: 13px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .all { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; padding: 0 12px 10px; }
  .all label { display: flex; align-items: center; gap: 8px; color: var(--muted); font-size: 13px; }
  .soft { margin: 0; color: var(--muted); font-size: 12.5px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 10px; padding: 0 12px 12px; align-items: start; }
  .card { display: flex; flex-direction: column; background: var(--card); border: 1px solid var(--card-line); border-radius: 10px; overflow: hidden; min-width: 0; }
  .card.out .pic img, .card.out .pic video { opacity: 0.45; }
  /* the same shape for every card's picture, whatever the picture's own */
  .pic { position: relative; display: block; width: 100%; height: 0; padding: 62.5% 0 0; border: 0; background: var(--picture-bg); cursor: zoom-in; overflow: hidden; }
  .pic img, .pic video { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; display: block; }
  .sound { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; color: var(--muted); }
  .shape { position: absolute; left: 6px; bottom: 6px; padding: 1px 6px; border-radius: 5px; background: rgba(5, 7, 11, 0.78); font-size: 11px; font-weight: 600; color: var(--fg-soft); font-variant-numeric: tabular-nums; }
  .body { display: flex; flex-direction: column; gap: 6px; padding: 8px 10px 10px; }
  .name { font-weight: 600; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--muted); font-size: 12px; }
  .why { display: flex; gap: 6px; align-items: flex-start; font-size: 12px; color: var(--muted); }
  .why span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .why i { flex: none; font-style: normal; font-size: 10px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; padding: 1px 6px; border-radius: 5px; }
  .why i.guess { background: var(--accent-bg, #24314a); color: var(--accent); }
  .why i.check { background: #2a2414; color: var(--warn); }
  .why i.none { background: #1d2330; color: var(--muted); }
  .body select { width: 100%; }
  .preview { position: fixed; inset: 0; z-index: 70; background: rgba(5, 7, 11, 0.84); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .sheet { width: min(1060px, 100%); max-height: 100%; overflow: auto; background: #131822; border: 1px solid var(--control-line); border-radius: 16px; display: grid; grid-template-columns: minmax(0, 1.7fr) minmax(0, 1fr); }
  .big { background: var(--picture-bg); display: flex; align-items: center; justify-content: center; min-height: 240px; padding: 12px; }
  .big img, .big video { max-width: 100%; max-height: 72vh; object-fit: contain; display: block; }
  .side { display: flex; flex-direction: column; gap: 10px; padding: 18px; }
  .side h3 { margin: 0; font-size: 17px; word-break: break-word; }
  .eyebrow { font-size: 11px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
  .places { display: flex; flex-wrap: wrap; gap: 6px; }
  .places button { border: 1px solid var(--control-line); background: var(--control, #161b25); color: var(--fg); border-radius: 999px; padding: 5px 11px; font-family: inherit; font-size: 12.5px; font-weight: 500; cursor: pointer; }
  .places button[aria-pressed='true'] { border-color: var(--accent); background: var(--accent-bg, #24314a); color: var(--accent); }
  .side .btn { align-self: flex-start; }
</style>
