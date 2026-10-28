<script>
  // A table's achievements and high scores, on its page, in one card flipped between them
  // (`prefs.scoreCard`): its achievements (`TableAchievements`, those earned lit up), or its high
  // scores — yours, from its play log, or all of them: from the machine's memory (VPX's brought over
  // too) and from table scripts that keep their own, as its machine's memory or the values its
  // script saves keep them. All is one list: ours, and VPX's beside them while the box is ticked —
  // highest first, each of VPX's marked, a score both keep shown once. VPX's copy is taken as ours
  // to start from the first time the table plays here, by itself. The box greyed where VPX keeps
  // none for it, the card saying so where neither keeps any that can be read.
  import { invoke } from '@tauri-apps/api/core';
  import TableAchievements from './TableAchievements.svelte';
  import { app, savePrefs, openLink } from './store.svelte.js';

  let { t } = $props();

  let hs = $state(null);

  $effect(() => {
    const id = t.id;
    hs = null;
    invoke('high_scores', { id })
      .then((v) => t.id === id && (hs = v))
      .catch((e) => t.id === id && (hs = { kind: 'none', kept: [], problem: String(e) }));
  });

  const withVpx = $derived(app.prefs.vpxScores ?? true);
  function setWithVpx(on) {
    app.prefs.vpxScores = on;
    savePrefs();
  }

  // which side of the card shows: its achievements or its high scores
  const side = $derived(app.prefs.scoreCard === 'achievements' ? 'achievements' : 'scores');
  function setSide(v) {
    app.prefs.scoreCard = v;
    savePrefs();
  }

  // the user's own: the scores of the games they played here, from its play log, highest first
  const mineOnly = $derived(!!app.prefs.myScores);
  function setMine(on) {
    app.prefs.myScores = on;
    savePrefs();
  }
  const mine = $derived(
    (t.played?.log || [])
      .flatMap((g) => (g.scores || []).filter((s) => s > 0).map((score) => ({ score, at: g.at })))
      .sort((a, b) => b.score - a.score)
      .slice(0, 10),
  );


  const ours = $derived(hs?.kept.find((k) => k.by === 'ours'));
  const vpx = $derived(hs?.kept.find((k) => k.by === 'vpx'));

  /** Ours, and VPX's that ours hasn't (the same initials and score), while they're shown: highest
   *  first, a place without a score last, as kept. */
  function merged(key) {
    const list = (ours?.[key] || []).map((e) => ({ ...e, vpx: false }));
    if (withVpx && vpx) {
      for (const e of vpx[key]) {
        if (!list.some((o) => o.initials === e.initials && o.score === e.score && (key === 'scores' || o.label === e.label))) list.push({ ...e, vpx: true });
      }
    }
    // champions keep the machine's order; a list both keep is ranked by score
    if (key === 'scores' && list.some((e) => e.vpx) && ours?.scores.length) list.sort((a, b) => (b.score ?? -1) - (a.score ?? -1));
    return list;
  }

  const scores = $derived(merged('scores'));
  const champions = $derived(merged('champions'));
  // with VPX's among ours, places are counted, not named: two lists' "Grand Champion"s would clash
  const mixed = $derived(scores.some((e) => e.vpx) && scores.some((e) => !e.vpx));
  const number = (n) => (n == null ? '' : n.toLocaleString());
  const date = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '');
  const games = (k) => (k?.gamesPlayed != null ? `${k.gamesPlayed} ${k.gamesPlayed === 1 ? 'game' : 'games'}, ` : '');
</script>

<section class="panel scores" id="achievements" aria-label="Achievements and high scores">
  <div class="head">
    <span class="flip" role="tablist" aria-label="Show">
      <button type="button" role="tab" aria-selected={side === 'achievements'} onclick={() => setSide('achievements')}>Achievements</button>
      <button type="button" role="tab" aria-selected={side === 'scores'} onclick={() => setSide('scores')}>High scores</button>
    </span>
    {#if side === 'scores'}
      <span class="boxes">
        <span class="seg" role="radiogroup" aria-label="Whose scores">
          <button type="button" role="radio" aria-checked={mineOnly} onclick={() => setMine(true)}>Yours</button>
          <button type="button" role="radio" aria-checked={!mineOnly} onclick={() => setMine(false)}>All</button>
        </span>
        {#if hs && hs.kind !== 'none' && !mineOnly}
          <label class="check" class:off={!vpx} title={vpx ? undefined : 'VPX keeps none for this table'}><input type="checkbox" checked={withVpx && !!vpx} disabled={!vpx} onchange={(e) => setWithVpx(e.currentTarget.checked)} />Played in VPX</label>
        {/if}
      </span>
    {/if}
  </div>
  {#if side === 'achievements'}
    <TableAchievements {t} />
  {:else if !hs}
    <p class="quiet">…</p>
  {:else if mineOnly}
      {#if mine.length}
        <ol class="list">
          {#each mine as e, i (i)}
            <li>
              <span class="place">{i + 1}.</span>
              <span class="who">{date(e.at)}</span>
              <span class="score">{number(e.score)}</span>
              <span class="from"></span>
            </li>
          {/each}
        </ol>
        <p class="said">From your games here, as its play log keeps them.</p>
      {:else}
        <p class="quiet">None yet: your scores show here once you’ve played it here.</p>
      {/if}
    {:else if hs.kind === 'none'}
      <p class="quiet">None that can be read: it drives no machine, and its script saves no high scores of its own. Yours shows the scores of your games here.</p>
    {:else if !scores.length && !champions.length}
      <p class="quiet">{hs.problem ?? (vpx && !withVpx ? 'None played here yet; VPX’s are hidden.' : hs.kind === 'machine' ? 'None yet: its machine keeps them once it’s played.' : 'None yet: its script keeps them once it’s played.')}</p>
    {:else}
      <ol class="list">
        {#each scores as e, i (i)}
          <li class:vpx={e.vpx}>
            <span class="place" title={mixed ? (e.label ?? '') : undefined}>{mixed || !e.label ? `${i + 1}.` : e.label}</span>
            <span class="who">{e.initials}</span>
            <span class="score">{number(e.score)}</span>
            <span class="from">{#if e.vpx}<span class="tag" title="Played in VPX">VPX</span>{/if}</span>
          </li>
        {/each}
      </ol>
      {#if champions.length}
        <h3>Champions</h3>
        <ol class="list">
          {#each champions as e, i (i)}
            <li class:vpx={e.vpx}>
              <span class="place">{e.label ?? ''}</span>
              <span class="who">{e.initials}</span>
              <span class="score">{[number(e.score), e.extra].filter(Boolean).join(' ')}</span>
              <span class="from">{#if e.vpx}<span class="tag" title="Played in VPX">VPX</span>{/if}</span>
            </li>
          {/each}
        </ol>
      {/if}
      <p class="said">
        {[ours && `Here: ${games(ours)}changed ${date(ours.changed)}`, vpx && withVpx && `In VPX: ${games(vpx)}changed ${date(vpx.changed)}`].filter(Boolean).join(' · ')}
      </p>
      {#if hs.kind === 'machine'}
        <p class="credit">Read from its machine’s memory with the <button type="button" class="link" onclick={() => openLink('https://github.com/tomlogic/pinball-memory-maps')}>Pinball Memory Maps</button>, under the ODbL.</p>
      {/if}
  {/if}
</section>

<style>
  /* the table page's panels */
  .scores { scroll-margin-top: 150px; display: flex; flex-direction: column; gap: 10px; padding: 18px 20px; border-radius: 14px; background: var(--panel); border: 1px solid var(--panel-line); }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
  .flip { display: inline-flex; gap: 4px; }
  .flip button { padding: 4px 2px; margin-right: 12px; border: 0; border-bottom: 2px solid transparent; background: none; color: var(--faint); font: inherit; font-size: 16px; font-weight: 700; cursor: pointer; }
  .flip button:hover { color: var(--fg-soft); }
  .flip button[aria-selected='true'] { color: var(--fg); border-bottom-color: var(--accent); }
  .boxes { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 6px 14px; }
  .seg { display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 28px; padding: 0 12px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .check { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--fg-soft); }
  .check.off { color: var(--faint); }
  .list { list-style: none; margin: 0; padding: 0; }
  .list li { display: grid; grid-template-columns: minmax(0, 1fr) 56px auto 40px; gap: 12px; align-items: baseline; padding: 6px 2px; border-top: 1px solid #1f2532; }
  .place { color: var(--fg-soft); font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who { font-family: var(--display); letter-spacing: 0.08em; }
  .score { font-variant-numeric: tabular-nums; text-align: right; }
  .from { text-align: right; }
  .tag { padding: 1px 6px; border-radius: 5px; background: #2a2140; color: #cdb8ff; font-size: 10px; font-weight: 700; letter-spacing: 0.04em; }
  li.vpx .who, li.vpx .score { color: var(--fg-soft); }
  h3 { margin: 10px 0 2px; font-size: 13px; color: var(--muted); font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; }
  .quiet, .said, .credit { margin: 0; color: var(--muted); font-size: 13px; }
  .credit { color: var(--faint); }
  .link { padding: 0; border: 0; background: none; color: var(--accent); font: inherit; cursor: pointer; }
  .link:hover { text-decoration: underline; }
</style>
