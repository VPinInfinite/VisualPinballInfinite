<script>
  // Your stats: what you've played, from the plays the library keeps for each table — how many and how
  // long, the most played, the latest plays with their scores, the last twelve weeks — and what the
  // library holds, by decade and by maker. Each table opens its page.
  import Icon from './Icon.svelte';
  import { lasted } from './PlayLog.svelte';
  import { app, openTable, groupOf } from './store.svelte.js';

  const T = $derived(app.tables);
  const played = $derived(T.filter((t) => t.played?.count));
  const plays = $derived(played.reduce((n, t) => n + t.played.count, 0));
  const seconds = $derived(played.reduce((n, t) => n + (t.played.seconds || 0), 0));

  const hours = (s) => (s < 3600 ? lasted(s) : `${(s / 3600).toFixed(s < 36000 ? 1 : 0)} h`);
  // the big number: short
  const total = (s) => (s < 60 ? '<1 min' : s < 3600 ? `${Math.round(s / 60)} min` : hours(s));
  const date = (ms) => new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
  const when = (ms) => new Date(ms).toLocaleString(undefined, { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' });
  const number = (n) => n.toLocaleString();

  // the most played, by time, the plays beside it
  const most = $derived([...played].sort((a, b) => (b.played.seconds || 0) - (a.played.seconds || 0) || b.played.count - a.played.count).slice(0, 10));
  const mostTop = $derived(Math.max(1, ...most.map((t) => t.played.seconds || 0)));

  // the first play the log kept, which the page counts from
  const first = $derived(Math.min(...played.flatMap((t) => (t.played.log || []).map((p) => p.at))));
  const since = $derived(Number.isFinite(first) ? new Date(first).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' }) : '');

  // the latest plays, from every table's log
  const latest = $derived(
    played
      .flatMap((t) => (t.played.log || []).map((p) => ({ t, p })))
      .sort((a, b) => b.p.at - a.p.at)
      .slice(0, 8),
  );

  // the last twelve weeks, Monday to Sunday, the time played in each
  const weeks = $derived.by(() => {
    const day = 864e5;
    const now = new Date();
    const monday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - ((now.getDay() + 6) % 7)).getTime();
    const out = Array.from({ length: 12 }, (_, i) => ({ start: monday - (11 - i) * 7 * day, seconds: 0, plays: 0 }));
    for (const t of played) {
      for (const p of t.played.log || []) {
        const w = out.findLast((x) => p.at >= x.start);
        if (w && p.at < w.start + 7 * day) {
          w.seconds += p.seconds;
          w.plays += 1;
        }
      }
    }
    return out;
  });
  const weekTop = $derived(Math.max(1, ...weeks.map((w) => w.seconds)));

  // what the library holds: by decade, and its ten biggest makers
  const count = (by) => {
    const m = new Map();
    for (const t of T) {
      const g = groupOf(t, by);
      m.set(g, (m.get(g) || 0) + 1);
    }
    return [...m];
  };
  const decades = $derived(count('decade').sort((a, b) => a[0].localeCompare(b[0])));
  const decadeTop = $derived(Math.max(1, ...decades.map(([, n]) => n)));
  const makers = $derived(count('maker').filter(([g]) => g !== 'Not on VPS' && !g.endsWith('unknown')).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, 10));
  const makerTop = $derived(Math.max(1, ...makers.map(([, n]) => n)));

  const open = (t) => openTable(t.id, app.tables.map((x) => x.id));
</script>

<div class="page">
  <header>
    <h1 class="display">Your stats</h1>
    <p>{since ? `From every play since ${since}, the first the library kept.` : 'From every play here.'}</p>
  </header>

  <div class="tiles">
    <div class="tile"><span class="n">{T.length}</span><span class="what">tables in your library</span></div>
    <div class="tile"><span class="n">{played.length}</span><span class="what">of them played here</span></div>
    <div class="tile"><span class="n">{number(plays)}</span><span class="what">{plays === 1 ? 'play' : 'plays'}</span></div>
    <div class="tile"><span class="n">{total(seconds)}</span><span class="what">played in all</span></div>
  </div>

  {#if !played.length}
    <div class="empty">Nothing played here yet: press PLAY on a table, and what you play shows here.</div>
  {:else}
    <div class="grid">
      <section class="card">
        <h2>Most played</h2>
        <ol class="bars">
          {#each most as t (t.id)}
            <li>
              <button type="button" class="name" onclick={() => open(t)}>{t.name}</button>
              <span class="bar"><span style="width: {(100 * (t.played.seconds || 0)) / mostTop}%"></span></span>
              <span class="val">{hours(t.played.seconds || 0)} · {t.played.count}×</span>
            </li>
          {/each}
        </ol>
      </section>

      <section class="card">
        <h2>Latest plays</h2>
        <ol class="plays">
          {#each latest as { t, p } (t.id + p.at)}
            <li>
              <button type="button" class="name" onclick={() => open(t)}>{t.name}</button>
              <span class="when">{when(p.at)}</span>
              <span class="long">{lasted(p.seconds)}</span>
              <span class="score">{p.scores.length ? number(Math.max(...p.scores)) : '—'}</span>
            </li>
          {/each}
        </ol>
      </section>

      <section class="card wide">
        <h2>The last twelve weeks</h2>
        <div class="weeks" role="img" aria-label="Time played each week for the last twelve weeks">
          {#each weeks as w (w.start)}
            <div class="week" title="{date(w.start)}: {w.plays} {w.plays === 1 ? 'play' : 'plays'}, {hours(w.seconds)}">
              <span class="col"><span style="height: {(100 * w.seconds) / weekTop}%"></span></span>
              <span class="label">{date(w.start)}</span>
            </div>
          {/each}
        </div>
      </section>
    </div>
  {/if}

  <div class="grid">
    <section class="card">
      <h2>Your library by decade</h2>
      <ol class="bars">
        {#each decades as [g, n] (g)}
          <li>
            <span class="name plain">{g}</span>
            <span class="bar"><span style="width: {(100 * n) / decadeTop}%"></span></span>
            <span class="val">{n}</span>
          </li>
        {/each}
      </ol>
    </section>
    <section class="card">
      <h2>Your library's makers</h2>
      <ol class="bars">
        {#each makers as [g, n] (g)}
          <li>
            <span class="name plain">{g}</span>
            <span class="bar"><span style="width: {(100 * n) / makerTop}%"></span></span>
            <span class="val">{n}</span>
          </li>
        {/each}
      </ol>
    </section>
  </div>
  <p class="soft"><Icon name="clock" size={14} />Each play counts from PLAY until the player closes; a score is the last game's, where its machine or script shows it.</p>
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 20px; max-width: 1240px; width: 100%; margin: 0 auto; box-sizing: border-box; }
  header h1 { font-size: 38px; line-height: 1; margin: 0; }
  header p { margin: 8px 0 0; color: var(--muted); }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 14px; }
  .tile { display: flex; flex-direction: column; gap: 4px; padding: 16px 18px; border-radius: 14px; background: var(--card); border: 1px solid var(--card-line); }
  .tile .n { font-family: var(--display, inherit); font-size: 34px; font-weight: 700; line-height: 1; }
  .tile .what { color: var(--muted); font-size: 13px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(420px, 1fr)); gap: 16px; }
  .card { padding: 16px 18px; border-radius: 14px; background: var(--card); border: 1px solid var(--card-line); display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .card.wide { grid-column: 1 / -1; }
  .card h2 { margin: 0; font-size: 16px; }
  .bars, .plays { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .bars li { display: grid; grid-template-columns: minmax(0, 200px) minmax(0, 1fr) auto; align-items: center; gap: 12px; font-size: 13px; }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; padding: 0; border: 0; background: none; color: var(--fg); font: inherit; cursor: pointer; }
  .name:hover { color: var(--accent); }
  .name.plain { cursor: default; color: var(--fg-soft); }
  .name.plain:hover { color: var(--fg-soft); }
  .bar { height: 10px; border-radius: 999px; background: var(--control); overflow: hidden; }
  .bar span { display: block; height: 100%; border-radius: 999px; background: var(--accent); }
  .val { color: var(--muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .plays li { display: grid; grid-template-columns: minmax(0, 1fr) auto auto auto; align-items: baseline; gap: 12px; font-size: 13px; }
  .when, .long { color: var(--muted); white-space: nowrap; }
  .score { font-variant-numeric: tabular-nums; min-width: 90px; text-align: right; }
  .weeks { display: grid; grid-template-columns: repeat(12, 1fr); gap: 8px; height: 160px; align-items: end; }
  .week { display: flex; flex-direction: column; align-items: center; gap: 6px; height: 100%; }
  .col { flex: 1; width: 100%; max-width: 42px; display: flex; align-items: flex-end; border-radius: 6px; background: var(--control); overflow: hidden; }
  .col span { display: block; width: 100%; background: var(--accent); border-radius: 6px 6px 0 0; }
  .label { font-size: 11px; color: var(--faint); white-space: nowrap; }
  .empty { padding: 32px 24px; border: 1px dashed var(--control-line); border-radius: 14px; text-align: center; color: var(--muted); }
  .soft { margin: 0; color: var(--faint); font-size: 12px; display: inline-flex; gap: 6px; align-items: center; }
</style>
