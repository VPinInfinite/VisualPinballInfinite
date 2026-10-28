<script>
  // Tools › Media: the pictures, videos and sound a front end shows while you browse — each
  // table's playfield, backglass, score display, topper, wheel, flyer and instruction card (PinUP
  // packs, which a table plays in the game, are their own page). Making what's missing for every
  // table at once, in the background, one table at a time, with how long it takes; PinUP Popper's
  // part (the media used put in its folders); and every table's media, kind by kind, those it lacks
  // as empty squares — media files dropped onto a table's row are for that table.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import SearchBox from './SearchBox.svelte';
  import { app, drop, makePictures, notice, openTable, savePrefs } from './store.svelte.js';

  // the kinds a front end shows, each: its words, whether our player makes it (and its Make kind)
  const KINDS = [
    ['playfield', 'Playfield', 'playfield'],
    ['playfield-video', 'Playfield video', 'video'],
    ['backglass', 'Backglass', 'backglass'],
    ['dmd', 'Score display', 'dmd'],
    ['topper', 'Topper', null],
    ['wheel', 'Wheel', 'wheel'],
    ['gameinfo', 'Flyer', null],
    ['gamehelp', 'Instruction card', null],
    ['sound', 'Sound', 'video'],
  ];
  // a kind it has: a picture or a video of it (a playfield picture, which a video isn't)
  const has = (t, k) => (k === 'sound' ? !!t.audio : t.media.some((m) => m.kind === k || (k !== 'playfield' && m.kind === `${k}-video`)));
  // a kind our player found it has none of when last made (no backglass file, no score display)
  const none = (t, make) => !!make && (t.none || []).includes(make);
  // what our player can make of what it lacks
  const toMake = (t) => [...new Set(KINDS.filter(([k, , make]) => make && !has(t, k) && !none(t, make)).map(([, , make]) => make))];
  const NONE = { backglass: 'No backglass file to draw it from', dmd: 'It has no score display' };

  const tables = $derived([...app.tables].filter((t) => !t.hiddenTable).sort((a, b) => a.name.localeCompare(b.name)));
  const wanting = $derived(tables.filter((t) => toMake(t).length));
  // a quarter of a minute a table, thirty-five seconds more with its video and its sound
  const seconds = $derived(wanting.reduce((n, t) => n + 15 + (toMake(t).includes('video') ? 35 : 0), 0));
  const roughly = (s) => (s < 90 ? `about ${Math.max(1, Math.round(s / 60))} minute` : s < 5400 ? `about ${Math.round(s / 60)} minutes` : `about ${(s / 3600).toFixed(1)} hours`);

  let asking = $state(false);
  async function makeAll() {
    asking = false;
    const queue = wanting.map((t) => [t.id, toMake(t)]);
    notice(`Making what’s missing for ${queue.length} ${queue.length === 1 ? 'table' : 'tables'}, one at a time: the bar at the top shows how far it’s got.`, { kind: 'done' });
    // all asked at once; the app makes them one table at a time, the rest waiting their turn
    await Promise.all(queue.map(([id, kinds]) => makePictures(id, { kinds }, { quiet: true })));
    notice(`What was missing is made for ${queue.length} ${queue.length === 1 ? 'table' : 'tables'}.`, { kind: 'done' });
  }

  // a row's tables: all, those lacking some kind the app can make, those lacking none
  let showing = $state('all');
  // the search: a table's name
  let query = $state('');
  const rows = $derived((showing === 'missing' ? wanting : showing === 'complete' ? tables.filter((t) => !toMake(t).length) : tables).filter((t) => !query.trim() || t.name.toLowerCase().includes(query.trim().toLowerCase())));

  // PinUP Popper: the media used put in its folders, off unless turned on; every table's on a word
  const popperOn = $derived(!!app.prefs.popperMedia);
  function setPopper(on) {
    app.prefs.popperMedia = on;
    savePrefs();
  }
  let popperAsking = $state(false);
  let busy = $state(false);
  const plural = (w, n) => (n === 1 ? w : w.endsWith('s') ? `${w}es` : `${w}s`);
  // the media a table uses put in PinballX's and PinballY's folders too
  const frontsOn = $derived(!!app.prefs.frontsMedia);
  let frontsAsking = $state(false);
  function setFronts(on) {
    app.prefs.frontsMedia = on;
    savePrefs();
    frontsAsking = false;
  }
  async function askFronts() {
    plan = null;
    frontsAsking = true;
    plan = await invoke('popper_media_plan').catch(() => []);
  }

  const FROM = { VPS: 'from VPS', ours: 'made by our player', Popper: 'of Popper’s own, brought to the front', PinballX: 'of PinballX’s', PinballY: 'of PinballY’s' };

  // the front ends there for the library, and the one a drop's media go in: the user's choice
  // where there's more than one
  let fronts = $state([]);
  let target = $state(null);
  invoke('front_ends').then(([there, into]) => {
    fronts = there;
    target = into;
  }).catch(() => {});
  const others = $derived(fronts.filter((f) => f.key !== 'Popper'));
  function setTarget(key) {
    app.prefs.mediaFrontEnd = key;
    target = key;
    savePrefs();
  }
  let plan = $state(null);
  async function ask() {
    popperAsking = true;
    plan = null;
    plan = await invoke('popper_media_plan').catch(() => []);
  }
  async function putAllIn() {
    popperAsking = false;
    busy = true;
    try {
      const n = await invoke('popper_media_all');
      notice(n ? `${n} ${n === 1 ? 'picture or video' : 'pictures and videos'} put in the front ends’ folders; their own ones kept beside them, numbered.` : 'The front ends show each table’s chosen media already.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }
</script>

<div class="page">
  <header>
    <div class="title">
      <h1 class="display">Media</h1>
      <p>The pictures, videos and sound a front end shows while you browse: each table’s playfield, backglass, score display, topper, wheel, flyer and instruction card. A table’s PinUP pack, which it plays in the game, is under PinUP packs.</p>
    </div>
  </header>

  <section class="part" aria-label="Make what’s missing">
    <h2 class="partname">Make what’s missing</h2>
    <p class="note">Our player draws a table’s playfield, backglass and score display, films a preview with its sound, with no window, and the app draws its wheel, for every table lacking them. Toppers, flyers and instruction cards come from VPS or PinUP Popper.</p>
    {#if !wanting.length}
      <p class="soft">Every table has all our player can make.</p>
    {:else if asking}
      <div class="ask">
        <span>Make what’s missing for {wanting.length} {wanting.length === 1 ? 'table' : 'tables'}? It takes {roughly(seconds)}, one table at a time in the background; you can use the app meanwhile.</span>
        <button type="button" class="btn small" onclick={makeAll}>Make them</button>
        <button type="button" class="btn small ghost" onclick={() => (asking = false)}>Cancel</button>
      </div>
    {:else}
      <div class="row-line">
        <button type="button" class="btn" disabled={app.makingIds.length > 0} onclick={() => (asking = true)}><Icon name="image" size={16} />Make missing media for all tables</button>
        <span class="soft">{wanting.length} {wanting.length === 1 ? 'table lacks' : 'tables lack'} some · {roughly(seconds)}{app.makingIds.length ? ` · ${app.makingIds.length} being made now` : ''}</span>
      </div>
    {/if}
  </section>

  <section class="part" aria-label="PinUP Popper">
    <h2 class="partname">PinUP Popper</h2>
    <p class="note">PinUP Popper shows the pictures and videos kept in its own folders. The ones our player makes, and VPS’s, are kept in ours, so Popper doesn’t see them unless they’re copied over.</p>
    <label class="switch"><input type="checkbox" checked={popperOn} onchange={(e) => setPopper(e.currentTarget.checked)} /><span><b>Show the same media in PinUP Popper</b></span></label>
    <ul class="what plain">
      <li><b>On:</b> whichever picture or video you pick on a table’s page (the one with the blue check) is copied into Popper’s folder, so Popper shows the same one. Media you make go in too, where Popper has none.</li>
      <li><b>Nothing of Popper’s is lost:</b> the one Popper had is kept beside it with a number (Name01.png), never deleted. Pick it again and they swap back. A copy put in before goes to the Recycle Bin instead: the picture it copied is still here.</li>
      <li><b>Off:</b> Popper’s folders are left alone. You can still copy one picture over from its own window.</li>
    </ul>
    {#if popperOn}
      {#if popperAsking}
        <div class="ask">
          <span>
            {#if plan === null}Counting what would go in…{:else if !plan.length}Nothing to put in: Popper shows what each table uses already.{:else}Put {plan.reduce((n, [, , k]) => n + k, 0)} in Popper’s folders now — {plan.map(([label, from, n]) => `${n} ${plural(label.toLowerCase(), n)} ${FROM[from]}`).join(', ')}? What you chose goes in front, Popper’s own kept beside it, numbered; where Popper shows nothing on a screen, the one used there fills it.{/if}
          </span>
          <button type="button" class="btn small" disabled={!plan?.length} onclick={putAllIn}>Put them in</button>
          <button type="button" class="btn small ghost" onclick={() => (popperAsking = false)}>Cancel</button>
        </div>
      {:else}
        <div class="row-line"><button type="button" class="btn small" disabled={busy} onclick={ask}><Icon name="download" size={14} />{busy ? 'Putting them in…' : 'Put them in for every table now'}</button></div>
      {/if}
    {/if}
  </section>

  {#if others.length}
    <section class="part" aria-label="Other front ends">
      <h2 class="partname">{others.map((f) => f.name).join(' and ')}</h2>
      {#each others as f (f.key)}
        <p class="note">{f.name}’s media for these tables are in {f.folder}: they show on each table’s page beside the rest, its own marked “{f.name} shows this one”.</p>
      {/each}
      <label class="switch"><input type="checkbox" checked={frontsOn} onchange={(e) => setFronts(e.currentTarget.checked)} /><span><b>Put the media a table uses in {others.map((f) => f.name).join(' and ')}’s folders too</b></span></label>
      <p class="what">When it’s on, the one with the blue check of each kind on a table’s page is put in front in {others.length === 1 ? 'its' : 'their'} folder for that screen, named as {others.length === 1 ? 'it names' : 'each names'} the game’s media, whenever you choose another. Their own one there before is kept beside it, numbered (Name 1.png), never deleted; a copy put in before goes to the Recycle Bin. VPS’s pictures go in as PNGs.</p>
      {#if frontsOn}
        {#if frontsAsking}
          <div class="ask">
            <span>{#if plan === null}Counting what would go in…{:else if !plan.length}Nothing to put in: they show what each table uses already.{:else}Put {plan.reduce((n, [, , k]) => n + k, 0)} in now — {plan.map(([label, from, n]) => `${n} ${plural(label.toLowerCase(), n)} ${FROM[from]}`).join(', ')}? What you chose goes in front, their own one there before kept beside it, numbered.{/if}</span>
            <button type="button" class="btn small" disabled={!plan?.length} onclick={() => { frontsAsking = false; putAllIn(); }}>Put them in</button>
            <button type="button" class="btn small ghost" onclick={() => (frontsAsking = false)}>Cancel</button>
          </div>
        {:else}
          <div class="row-line"><button type="button" class="btn small" disabled={busy} onclick={askFronts}><Icon name="download" size={14} />{busy ? 'Putting them in…' : 'Put them in for every table now'}</button></div>
        {/if}
      {/if}
      {#if fronts.length > 1}
        <label class="choose">
          <span><b>Media you drop in go in</b></span>
          <select class="select" value={target} onchange={(e) => setTarget(e.currentTarget.value)}>
            {#each fronts as f (f.key)}<option value={f.key}>{f.name}’s folders</option>{/each}
          </select>
        </label>
        <p class="what">Each is named as that front end names a table’s media: for its own name for the game, else the table file’s name. The others’ folders aren’t touched.</p>
      {:else}
        <p class="what">Media you drop in go in {others[0].name}’s folders, named as it names each table’s media: for its description of the game, else the table file’s name.</p>
      {/if}
    </section>
  {/if}

  <section class="part" aria-label="Your media">
    <h2 class="partname">Your media</h2>
    <p class="note">The pictures, videos and sound each table shows while you browse, in the library and in cabinet mode (never what it plays with), kind by kind: an empty square is one it hasn’t yet, a dash one it has none to make from. Missing some counts what the app can make; toppers, flyers and instruction cards come from VPS or PinUP Popper. Drop media files onto a table’s row to add them for it.</p>
    <div class="bar">
      <div class="seg" role="radiogroup" aria-label="Which tables show">
        {#each [['all', 'All', tables.length], ['missing', 'Missing some', wanting.length], ['complete', 'Complete', tables.length - wanting.length]] as [key, label, n] (key)}
          <button type="button" role="radio" aria-checked={showing === key} onclick={() => (showing = key)}>{label} <span class="n">{n}</span></button>
        {/each}
      </div>
      <SearchBox bind:value={query} placeholder="Search by table" />
    </div>
    <div class="scroll">
      <div class="grid head" aria-hidden="true">
        <span>Table</span>
        {#each KINDS as [k, label] (k)}<span class="k">{label}</span>{/each}
        <span></span>
      </div>
      {#each rows as t (t.id)}
        {@const make = toMake(t)}
        <div class="grid trow" class:over={drop.dragging && drop.over === t.id} data-drop-table={t.id}>
          <button type="button" class="link" onclick={() => openTable(t.id)}>{t.name}</button>
          {#each KINDS as [k, label, made] (k)}
            {#if has(t, k)}<span class="k" title={label}><Icon name="check" size={15} width={2.4} /></span>
            {:else if none(t, made)}<span class="k dash" title={NONE[made]}>–</span>
            {:else}<span class="k" title={`No ${label.toLowerCase()} yet`}><span class="empty"></span></span>{/if}
          {/each}
          <span class="act">{#if make.length}<button type="button" class="btn small ghost" disabled={app.makingIds.includes(t.id)} onclick={() => makePictures(t.id, { kinds: make })}>{app.makingIds.includes(t.id) ? 'Making…' : 'Make missing'}</button>{/if}</span>
        </div>
      {/each}
    </div>
  </section>
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 26px; max-width: 2200px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px 16px; }
  header { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-end; justify-content: space-between; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .part { display: flex; flex-direction: column; gap: 10px; padding-top: 22px; border-top: 1px solid var(--line); }
  .partname { margin: 0 0 4px; font-size: 20px; font-weight: 700; color: var(--fg); }
  .note { margin: -4px 0 0; color: var(--faint); font-size: 13px; max-width: 860px; line-height: 1.5; }
  .soft { margin: 0; color: var(--faint); font-size: 13px; }
  .what { margin: 0; color: var(--faint); font-size: 13px; line-height: 1.5; max-width: 860px; }
  .what.plain { padding-left: 18px; display: flex; flex-direction: column; gap: 4px; }
  .what.plain b { color: var(--fg-soft); }
  .row-line { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; }
  .ask { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px 12px; border-radius: 10px; background: var(--control); border: 1px solid var(--control-line); max-width: 960px; }
  .ask span { flex: 1 1 300px; font-size: 13px; color: var(--fg-soft); }
  .switch { display: inline-flex; align-items: center; gap: 8px; font-size: 14px; color: var(--fg); }
  .switch input { accent-color: var(--accent); }
  .choose { display: inline-flex; align-items: center; gap: 10px; font-size: 14px; color: var(--fg); }
  .choose select { height: 32px; }
  .seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--control-line); border-radius: 8px; overflow: hidden; }
  .seg button { height: 32px; padding: 0 14px; border: 0; background: var(--control); color: var(--fg-soft); font-size: 13px; }
  .seg button + button { border-left: 1px solid var(--control-line); }
  .seg button[aria-checked='true'] { background: var(--accent-bg); color: #fff; }
  .seg .n { margin-left: 4px; color: var(--faint); font-size: 12px; }
  .seg button[aria-checked='true'] .n { color: inherit; opacity: 0.75; }
  .scroll { max-height: max(360px, calc(100vh - 300px)); overflow: auto; border-radius: 12px; }
  :global(html[data-whole]) .scroll { max-height: none; }
  .grid { display: grid; grid-template-columns: minmax(200px, 1.6fr) repeat(9, minmax(60px, 0.5fr)) 120px; gap: 6px 10px; align-items: center; }
  .head { position: sticky; top: 0; z-index: 1; padding: 8px 12px; background: var(--bg); font-size: 11px; font-weight: 700; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .trow { padding: 8px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); margin-top: 4px; }
  .trow.over { border-color: var(--accent); background: var(--accent-bg); }
  .k { display: flex; justify-content: center; color: var(--good); }
  .head .k { justify-content: center; text-align: center; }
  .empty { width: 22px; height: 16px; border-radius: 4px; outline: 1px dashed var(--control-line); }
  .k.dash { color: var(--faint); }
  .act { display: flex; justify-content: flex-end; }
  .link { padding: 0; border: 0; background: none; color: var(--fg); font: inherit; font-size: 14px; text-align: left; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .link:hover { color: var(--accent); }
</style>
