<script>
  // The player hub's achievements: every pack kept (`achievements_all`) — its tables in the library,
  // how much of it is earned and its points, its masteries — added from a file (`achievements_add`)
  // or deleted for good, asked first and warned it can't be recovered (`achievements_remove`); a pack's row opened to
  // show its achievements, those earned lit up; what hardcore and softcore mean, each a filter of
  // what's counted as earned. Sharing with VPIDB is set in Settings › General.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import { app, notice, openTable } from './store.svelte.js';

  let packs = $state(null);
  let asking = $state(null);
  // the packs opened to show their achievements, by ID
  let opened = $state({});
  // which earned achievements count: every one, those earned in hardcore, or those only in softcore
  let showing = $state('all');
  async function load() {
    packs = await invoke('achievements_all').catch(() => []);
  }
  $effect(() => {
    load();
    const off = listen('library-changed', () => load());
    return () => off.then((f) => f());
  });

  const counts = (a) => (showing === 'hardcore' ? !!a.hardcore : showing === 'softcore' ? !!a.softcore && !a.hardcore : !!a.earned);
  const earned = (p) => p.achievements.filter(counts);
  const all = $derived(packs ? packs.flatMap((p) => p.achievements) : []);
  const hardcoreCount = $derived(all.filter((a) => a.hardcore).length);
  const softcoreCount = $derived(all.filter((a) => a.softcore && !a.hardcore).length);
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

  async function remove(p) {
    asking = null;
    try {
      await invoke('achievements_remove', { id: p.id });
      notice(`${p.name} is deleted. What you earned with it stays earned.`, { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }
</script>

<div class="page">
  <header class="title">
    <h1 class="display">Achievements</h1>
    <p>Goals to earn as you play a table, made by its author or anyone else: an achievement pack goes with a table like its backglass, and a table can have several. Every table’s, earned and not, shows here and on its page.</p>
  </header>

  <section class="part" aria-labelledby="ach-yours">
    <div class="row-head">
      <h2 id="ach-yours" class="partname">Your packs</h2>
      <button type="button" class="btn small" onclick={add}>Add a pack…</button>
    </div>
    <div class="modes" role="radiogroup" aria-label="Which earned achievements to count">
      <button type="button" class="mode" role="radio" aria-checked={showing === 'hardcore'} onclick={() => (showing = showing === 'hardcore' ? 'all' : 'hardcore')}>
        <b>Hardcore <span class="n">{hardcoreCount} earned</span></b>
        <p>The table played as its author made it: its own script, physics, options and difficulty, no cheats. Only hardcore counts on VPIDB, for masteries and the lists of who mastered what. Click to count only these.</p>
      </button>
      <button type="button" class="mode" role="radio" aria-checked={showing === 'softcore'} onclick={() => (showing = showing === 'softcore' ? 'all' : 'softcore')}>
        <b>Softcore <span class="n">{softcoreCount} earned</span></b>
        <p>Anything else: changed physics, an edited script, the table’s options changed. You still earn them, kept for you, marked softcore. Click to count only these.</p>
      </button>
    </div>
    {#if showing !== 'all'}<p class="note">Counting only what’s earned in {showing}. <button type="button" class="link" onclick={() => (showing = 'all')}>Count every one</button></p>{/if}
    {#if packs === null}
      <p class="note">…</p>
    {:else if !packs.length}
      <div class="empty">
        <Icon name="trophy" size={28} />
        <div>
          <b>No achievement packs yet</b>
          <p>Ten sample packs, for Attack from Mars, Medieval Madness, Twilight Zone and seven more well-known tables, are on VPIDB (vpidb.org). A pack made for a table — its zip, or its achievements.json — is added here; what you earn shows here and on the table’s page, with the day you earned it.</p>
        </div>
      </div>
    {:else}
      <div class="packs">
        {#each packs as p (p.id)}
          {@const got = earned(p)}
          <div class="pack" class:open={opened[p.id]}>
            <button type="button" class="opener" aria-expanded={!!opened[p.id]} aria-label="{opened[p.id] ? 'Fold' : 'Open'} {p.name}" title="{opened[p.id] ? 'Fold it' : 'Its achievements'}" onclick={() => (opened = { ...opened, [p.id]: !opened[p.id] })}><Icon name={opened[p.id] ? 'chevD' : 'chevR'} size={16} /></button>
            <span class="pic">{#if p.icon}<img src={p.icon} alt="" />{:else}<Icon name="trophy" size={20} />{/if}</span>
            <span class="what">
              <b>{p.name}</b> <span class="faint">{p.version}{p.authors.length ? ` · by ${p.authors.join(', ')}` : ''}</span>
              <span class="tables">
                {#if p.tables.length}
                  {#each p.tables as [id, name], i (id)}{i ? ', ' : ''}<button type="button" class="link" onclick={() => openTable(id)}>{name}</button>{/each}
                {:else}
                  <span class="faint">For {p.game}: no table of yours it fits</span>
                {/if}
              </span>
            </span>
            <span class="got">
              <span class="meter" aria-hidden="true"><span style:width="{p.achievements.length ? (100 * got.length) / p.achievements.length : 0}%"></span></span>
              <span>{got.length} of {p.achievements.length} · {points(got)} of {points(p.achievements)} points</span>
              {#if p.mastered.length}<span class="mastered">Mastered {p.mastered.map(([v, at]) => `${v}, ${day(at)}`).join('; ')}</span>{/if}
            </span>
            <button type="button" class="btn small bin" aria-label="Delete {p.name}…" title="Delete this pack" onclick={() => (asking = p)}><Icon name="trash" size={15} /></button>
            {#if opened[p.id]}
              <div class="achs">
                {#each p.achievements as a (a.id)}
                  {@const got = counts(a)}
                  {@const hidden = a.secret && !a.earned}
                  <div class="ach" class:earned={got}>
                    <span class="apic">{#if a.icon && !hidden}<img src={a.icon} alt="" />{:else}<Icon name={hidden ? 'lock' : 'trophy'} size={18} />{/if}</span>
                    <span class="awords"><b>{hidden ? 'Secret achievement' : a.name}</b><span>{hidden ? 'Earn it to see what it is.' : a.about}</span></span>
                    <span class="apts">{a.points} {a.points === 1 ? 'point' : 'points'}{got ? ` · ${day(a.hardcore || a.softcore || a.earned[0])}${a.hardcore ? '' : ' · softcore'}` : ''}</span>
                    {#if got}<span class="tick"><Icon name="check" size={12} width={3} /></span>{/if}
                  </div>
                {/each}
              </div>
            {/if}
            {#if asking === p}
              <div class="ask">
                <span>Delete the achievement pack {p.name}? It’s deleted for good and can’t be recovered, not even from the Recycle Bin. It goes from every table it’s made for. What you’ve earned with it stays earned.</span>
                <button type="button" class="btn small" onclick={() => remove(p)}>Delete it</button>
                <button type="button" class="btn small ghost" onclick={() => (asking = null)}>Keep it</button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

  </section>

  <p class="note">Sharing what you earn with VPIDB: <button type="button" class="link" onclick={() => { app.settingsPage = 'general'; app.page = 'settings'; }}>Settings › General</button>.</p>
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 26px; max-width: 1240px; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .part { display: flex; flex-direction: column; gap: 14px; padding-top: 22px; border-top: 1px solid var(--line); }
  .partname { margin: 0; font-size: 20px; font-weight: 700; color: var(--fg); }
  .row-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; max-width: 860px; }
  .packs { display: flex; flex-direction: column; gap: 6px; max-width: 980px; }
  .pack { display: grid; grid-template-columns: 28px 44px minmax(0, 1fr) minmax(220px, 280px) 34px; gap: 8px 14px; align-items: center; padding: 12px 14px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); }
  .pack.open { border-color: var(--control-line); }
  .opener { width: 28px; height: 28px; display: grid; place-items: center; border: 0; border-radius: 7px; background: transparent; color: var(--fg-soft); cursor: pointer; }
  .opener:hover { background: var(--hover); color: var(--fg); }
  .achs { grid-column: 1 / -1; display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 6px; padding-top: 6px; }
  .ach { position: relative; display: grid; grid-template-columns: 38px minmax(0, 1fr); grid-template-rows: auto auto; gap: 2px 10px; align-items: center; padding: 9px 12px; border-radius: 10px; background: var(--panel); border: 1px solid var(--panel-line); opacity: 0.55; }
  .ach:not(.earned) .apic { filter: grayscale(0.85); }
  .ach.earned { opacity: 1; border-color: color-mix(in srgb, var(--good) 70%, var(--panel-line)); background: color-mix(in srgb, var(--good) 13%, var(--panel)); }
  .apic { grid-row: 1 / 3; width: 38px; height: 38px; border-radius: 9px; display: grid; place-items: center; background: var(--control); color: var(--faint); overflow: hidden; }
  .ach.earned .apic { color: var(--good); }
  .apic img { width: 100%; height: 100%; object-fit: cover; }
  .awords { min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .awords b { color: var(--fg); font-size: 13px; }
  .awords span { color: var(--fg-soft); font-size: 12px; line-height: 1.35; }
  .apts { grid-column: 2; color: var(--faint); font-size: 11.5px; }
  .ach.earned .apts { color: var(--good); }
  .tick { position: absolute; top: 7px; right: 7px; width: 18px; height: 18px; border-radius: 50%; display: grid; place-items: center; background: var(--good); color: #0b1410; }
  .pic { width: 44px; height: 44px; border-radius: 10px; display: grid; place-items: center; background: var(--control); color: var(--faint); overflow: hidden; }
  .pic img { width: 100%; height: 100%; object-fit: cover; }
  .what { min-width: 0; font-size: 14px; color: var(--fg-soft); }
  .what b { color: var(--fg); }
  .tables { display: block; margin-top: 3px; font-size: 13px; }
  .faint { color: var(--faint); }
  .got { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: var(--fg-soft); }
  .meter { height: 6px; border-radius: 999px; background: var(--control); overflow: hidden; }
  .meter span { display: block; height: 100%; background: var(--good); }
  .mastered { color: var(--good); font-weight: 600; }
  .bin { padding: 0 8px; }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; text-decoration: underline; text-decoration-color: var(--control-line); text-underline-offset: 3px; cursor: pointer; }
  .link:hover { color: var(--accent); }
  .ask { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); }
  .ask span { flex: 1 1 260px; font-size: 13px; color: var(--fg-soft); }
  .note { margin: 0; color: var(--faint); font-size: 13px; }
  .empty { display: flex; gap: 16px; align-items: flex-start; max-width: 860px; padding: 18px 20px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); color: var(--faint); }
  .empty b { color: var(--fg); font-size: 15px; }
  .empty p { margin: 4px 0 0; color: var(--fg-soft); font-size: 14px; line-height: 1.5; }
  .modes { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 8px; max-width: 860px; }
  .mode { padding: 12px 14px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .mode:hover { border-color: var(--control-line); }
  .mode[aria-checked='true'] { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, var(--card)); }
  .mode b { color: var(--fg); display: flex; align-items: baseline; gap: 8px; }
  .mode .n { color: var(--faint); font-size: 12px; font-weight: 600; }
  .mode p { margin: 4px 0 0; color: var(--faint); font-size: 13px; line-height: 1.5; }
</style>
