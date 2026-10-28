<script module>
  /** How long, in words: under a minute, 12 min, 1 h 05 min. */
  export function lasted(seconds) {
    if (seconds < 60) return 'under a minute';
    const m = Math.round(seconds / 60);
    return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')} min`;
  }
</script>

<script>
  // A table's plays, on its page: each play
  // from PLAY until the player closed, the newest first, how long it lasted, and its last game's
  // scores where they're known — from the machine's memory where its map says where they are, else
  // as the script last showed them on its backglass.
  let { t } = $props();

  const log = $derived([...(t.played?.log || [])].reverse());
  let all = $state(false);
  const shown = $derived(all ? log : log.slice(0, 6));
  const when = (ms) => new Date(ms).toLocaleString(undefined, { day: 'numeric', month: 'short', year: 'numeric', hour: 'numeric', minute: '2-digit' });
  const number = (n) => n.toLocaleString();
</script>

<section class="panel plays" aria-labelledby="plays">
  <h2 id="plays">Play log</h2>
  {#if !log.length}
    <p class="quiet">{t.played?.count ? 'Played before its plays were logged: the next ones show here.' : 'Not played here yet: each play shows here, when and for how long, and its score where it’s known.'}</p>
  {:else}
    <ol class="list">
      {#each shown as p (p.at)}
        <li>
          <span class="when">{when(p.at)}</span>
          <span class="long">{lasted(p.seconds)}</span>
          <span class="score" title={p.scores.length ? undefined : 'Its score isn’t known: its machine’s memory doesn’t say its last game’s, and its script showed none on a backglass.'}>
            {#if !p.scores.length}—{:else if p.scores.length === 1}{number(p.scores[0])}{:else}{p.scores.map((s, i) => `P${i + 1} ${number(s)}`).join(' · ')}{/if}
          </span>
        </li>
      {/each}
    </ol>
    {#if log.length > 6}
      <button type="button" class="btn small ghost more" onclick={() => (all = !all)}>{all ? 'Fewer' : `All ${log.length} plays`}</button>
    {/if}
  {/if}
</section>

<style>
  .plays { display: flex; flex-direction: column; gap: 8px; padding: 18px 20px; border-radius: 14px; background: var(--panel); border: 1px solid var(--panel-line); }
  h2 { margin: 0; font-size: 16px; font-weight: 700; }
  .list { list-style: none; margin: 0; padding: 0; }
  .list li { display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(90px, auto); gap: 14px; align-items: baseline; padding: 6px 2px; border-top: 1px solid #1f2532; font-size: 13px; }
  .when { color: var(--fg-soft); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .long { color: var(--muted); }
  .score { text-align: right; font-variant-numeric: tabular-nums; }
  .quiet { margin: 0; color: var(--muted); font-size: 13px; }
  .more { align-self: flex-start; }
</style>
