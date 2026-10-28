<script>
  // A table's achievements, in its page's achievements and high scores card: the packs made for it
  // (`achievements_for`), one at a time — chosen from a list where there are several, the choice
  // kept for the table (`prefs.achPack`) — with its name, version and authors, how VPIDB tags it
  // (official, community, sample, a mod; only on this PC where VPIDB doesn't have it) and whether
  // what's earned is shared with VPIDB (`vpidb_packs`, `vpidb_link`); its achievements as a grid of
  // cards or a list (`prefs.achView`), each with the picture its author gave it, what to do, its
  // points; those earned lit up, with the day (softcore marked); a secret one hidden until earned.
  // Whether the table plays hardcore or softcore here, and why. Read again whenever the library is
  // (a game played).
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import { app, savePrefs, notice, vpidbPacks, loadVpidbPacks } from './store.svelte.js';

  let { t } = $props();

  let got = $state(null);
  async function load() {
    try {
      got = await invoke('achievements_for', { id: t.id });
    } catch (e) {
      got = { packs: [], practice: [], problem: String(e) };
    }
  }
  $effect(() => {
    void t.id;
    load();
    const off = listen('library-changed', () => load());
    return () => off.then((f) => f());
  });

  const view = $derived(app.prefs.achView === 'list' ? 'list' : 'grid');
  function setView(v) {
    app.prefs.achView = v;
    savePrefs();
  }

  // the pack shown: the one chosen for this table, else the first
  const pack = $derived(got?.packs.find((p) => p.id === app.prefs.achPack?.[t.id]) ?? got?.packs[0] ?? null);
  function choose(id) {
    app.prefs.achPack = { ...(app.prefs.achPack || {}), [t.id]: id };
    savePrefs();
  }
  // how VPIDB has each pack, and whether what's earned is shared there
  let link = $state(null);
  $effect(() => {
    loadVpidbPacks();
    invoke('vpidb_link').then((l) => (link = l)).catch(() => {});
  });
  const online = $derived(Object.fromEntries(Object.values(vpidbPacks.byGame || {}).flat().map((o) => [o.id, o])));
  // as VPIDB's own pages name them
  const KIND = { official: 'Official', community: 'Community pack', sample: 'Sample pack', mod: 'Mod' };
  const kindOf = (p) => (online[p.id] ? KIND[online[p.id].kind] || 'Community pack' : vpidbPacks.byGame ? 'Only on this PC' : '');
  const syncOf = (p) => (!online[p.id] ? null : link?.connected && link?.sharing ? 'shared' : 'off');

  const all = $derived(pack ? pack.achievements : []);
  const earned = $derived(all.filter((a) => a.earned));
  const points = (list) => list.reduce((n, a) => n + a.points, 0);
  const day = (iso) => new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });

  async function add() {
    try {
      const name = await invoke('achievements_add');
      if (name) notice(`${name} is added.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
</script>

{#snippet card(a)}
  {@const hidden = a.secret && !a.earned}
  <div class="ach" class:earned={a.earned} class:row={view === 'list'}>
    <span class="pic">{#if a.icon && !hidden}<img src={a.icon} alt="" />{:else}<Icon name={hidden ? 'lock' : 'trophy'} size={view === 'list' ? 18 : 22} />{/if}</span>
    <span class="words">
      <b>{hidden ? 'Secret achievement' : a.name}</b>
      <span class="about">{hidden ? 'Earn it to see what it is.' : a.about}</span>
    </span>
    <span class="pts">{[`${a.points} ${a.points === 1 ? 'point' : 'points'}`, a.earned && `earned ${day(a.earned[0])}`, a.earned?.[1] === 'practice' && 'softcore'].filter(Boolean).join(' · ')}</span>
    {#if a.earned}<span class="got" title="Earned"><Icon name="check" size={13} width={3} /></span>{/if}
  </div>
{/snippet}

<div class="achievements">
  <div class="head">
    {#if all.length}<span class="sum">{earned.length} of {all.length} earned · {points(earned)} of {points(all)} points</span>{/if}
    {#if all.length}
      <span class="seg" role="radiogroup" aria-label="Show them as">
        <button type="button" role="radio" aria-checked={view === 'grid'} aria-label="A grid" title="A grid" onclick={() => setView('grid')}><Icon name="grid" size={15} /></button>
        <button type="button" role="radio" aria-checked={view === 'list'} aria-label="A list" title="A list" onclick={() => setView('list')}><Icon name="list" size={15} /></button>
      </span>
    {/if}
  </div>
  {#if !got}
    <p class="note">…</p>
  {:else if !got.packs.length}
    <p class="note">No achievement pack for this table yet. A pack made for it — by its author or anyone — adds goals to earn as you play.</p>
    <div><button type="button" class="btn small" onclick={add}>Add a pack…</button></div>
  {:else}
    {#if got.practice.length}
      <p class="note warn">Here they’re earned as softcore, kept for you but not counted towards masteries: {got.practice.join('; ')}. PLAY’s ▾ plays it Hardcore.</p>
    {/if}
    {@const p = pack}
    <div class="pack">
      {#if p.icon}<img class="packpic" src={p.icon} alt="" />{/if}
      {#if got.packs.length > 1}
        <select class="select packsel" aria-label="Which pack" value={p.id} onchange={(e) => choose(e.currentTarget.value)}>
          {#each got.packs as o (o.id)}<option value={o.id}>{o.name} {o.version}{online[o.id] ? ` · ${KIND[online[o.id].kind] || 'Community pack'}` : ''}</option>{/each}
        </select>
      {:else}
        <b>{p.name}</b>
      {/if}
      {#if kindOf(p)}<span class="kind {online[p.id]?.kind || 'local'}" title={online[p.id] ? `Tagged ${kindOf(p).toLowerCase()} on VPIDB` : 'VPIDB doesn’t have this pack: what you earn with it stays on this PC'}>{kindOf(p)}</span>{/if}
      {#if syncOf(p)}<span class="sync" class:off={syncOf(p) === 'off'} title={syncOf(p) === 'shared' ? 'What you earn with it is shared with VPIDB' : 'On VPIDB, but sharing what you earn is off (Settings › General)'}><Icon name="refresh" size={13} width={2.4} /></span>{/if}
      <span class="byline">{got.packs.length > 1 ? '' : `${p.version}`}{p.authors.length ? `${got.packs.length > 1 ? '' : ' · '}by ${p.authors.join(', ')}` : ''}{#if p.mastered.length} · <span class="mastered">mastered {p.mastered.map(([v, at]) => `${v}, ${day(at)}`).join('; ')}</span>{/if}</span>
    </div>
    <div class={view === 'list' ? 'list' : 'grid'}>
      {#each p.achievements as a (a.id)}{@render card(a)}{/each}
    </div>
  {/if}
</div>

<style>
  .achievements { display: flex; flex-direction: column; gap: 12px; }
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; min-height: 30px; }
  .sum { color: var(--faint); font-size: 14px; }
  .seg { margin-left: auto; display: inline-flex; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 30px; width: 34px; display: grid; place-items: center; border: 0; background: var(--control); color: var(--fg-soft); }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .note { margin: 0; color: var(--faint); font-size: 13px; line-height: 1.5; }
  .note.warn { color: var(--warn); }
  .pack { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 10px; color: var(--fg-soft); font-size: 14px; }
  .packsel { max-width: 320px; }
  .kind { padding: 2px 8px; border-radius: 6px; font-size: 11px; font-weight: 700; letter-spacing: 0.03em; text-transform: uppercase; background: var(--control); color: var(--fg-soft); }
  .kind.official { background: color-mix(in srgb, var(--accent) 22%, transparent); color: var(--accent-soft); }
  .kind.sample { background: color-mix(in srgb, var(--warn) 18%, transparent); color: var(--warn); }
  .kind.mod { background: color-mix(in srgb, #b48cff 20%, transparent); color: #c8b0ff; }
  .kind.local { background: transparent; border: 1px solid var(--control-line); color: var(--faint); }
  .sync { width: 22px; height: 22px; display: grid; place-items: center; border-radius: 50%; background: color-mix(in srgb, var(--good) 18%, transparent); color: var(--good); }
  .sync.off { background: var(--control); color: var(--faint); }
  .byline { color: var(--faint); font-size: 13px; }
  .pack b { color: var(--fg); }
  .packpic { width: 28px; height: 28px; border-radius: 6px; object-fit: cover; }
  .mastered { color: var(--good); font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 8px; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .ach { position: relative; display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); opacity: 0.55; }
  .ach:not(.earned) .pic { filter: grayscale(0.85); }
  /* earned: lit up, the square in the grid and the row in the list */
  .ach.earned { opacity: 1; border-color: color-mix(in srgb, var(--good) 70%, var(--card-line)); background: color-mix(in srgb, var(--good) 13%, var(--card)); box-shadow: 0 0 0 1px color-mix(in srgb, var(--good) 35%, transparent) inset; }
  .got { position: absolute; top: 8px; right: 8px; width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; background: var(--good); color: #0b1410; }
  .row .got { position: static; flex: 0 0 auto; }
  .ach.row { flex-direction: row; align-items: center; gap: 12px; padding: 8px 12px; }
  .pic { width: 44px; height: 44px; border-radius: 10px; display: grid; place-items: center; background: var(--control); color: var(--faint); overflow: hidden; flex: 0 0 auto; }
  .row .pic { width: 34px; height: 34px; border-radius: 8px; }
  .ach.earned .pic { color: var(--good); }
  .pic img { width: 100%; height: 100%; object-fit: cover; }
  .words { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .row .words { flex: 1; }
  .words b { color: var(--fg); font-size: 14px; }
  .about { color: var(--fg-soft); font-size: 13px; line-height: 1.4; }
  .pts { color: var(--faint); font-size: 12px; }
  .ach.earned .pts { color: var(--good); }
  .row .pts { white-space: nowrap; }
</style>
