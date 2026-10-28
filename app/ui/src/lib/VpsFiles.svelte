<script>
  // Every file VPS lists for a game, kind by kind — table releases, backglasses, ROMs, packs and the
  // rest — each kind opened or closed, a file's Get opening its page in the browser: on a table's
  // page ("More on VPS") and in the drop window's "See them". `mine`: what the user's own release is
  // called (Yours, Dropped). With `game` (its VPS ID), the achievement packs VPIDB lists for it too,
  // each added here with a click (downloaded from VPIDB and checked first).
  import Icon from './Icon.svelte';
  import { openLink, vpidbPacks, loadVpidbPacks, addVpidbPack } from './store.svelte.js';

  let { kinds, open = $bindable({}), mine = 'Yours', game = null } = $props();

  $effect(() => {
    if (game) loadVpidbPacks();
  });
  const packs = $derived(game ? (vpidbPacks.byGame?.[game] ?? []) : []);

  const date = (ms) => (ms ? new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '');
</script>

<div class="kinds">
  {#if packs.length}
    <div class="vkind" id="vps-achievements">
      <button type="button" class="vhead" aria-expanded={!!open.achievements} onclick={() => (open = { ...open, achievements: !open.achievements })}>
        <Icon name={open.achievements ? 'chevD' : 'chevR'} size={15} />
        <span class="vlabel">Achievement packs <span class="from">from VPIDB</span></span>
        <span class="vcount">{packs.length}</span>
      </button>
      {#if open.achievements}
        <div class="vfiles">
          {#each packs as p (p.id)}
            <div class="vfile" class:this={p.installed}>
              <span class="trophy"><Icon name="trophy" size={20} /></span>
              <div class="vtext">
                <div class="vtitle">
                  {p.name} {p.version}
                  {#if p.official}<span class="tag">Official</span>{/if}
                  {#if p.installed}<span class="yours">{p.installed === p.version ? 'Added' : `You have ${p.installed}`}</span>{/if}
                </div>
                <div class="vsub">{p.count} achievements · {p.points} points{p.by ? ` · by ${p.by}` : ''}{p.players ? ` · mastered by ${p.players}` : ''}</div>
                {#if p.about}<div class="vsub">{p.about}</div>{/if}
              </div>
              {#if p.installed !== p.version}
                <button type="button" class="btn small" disabled={vpidbPacks.adding === p.id} onclick={() => addVpidbPack(p)}><Icon name="download" size={14} />{vpidbPacks.adding === p.id ? 'Adding…' : p.installed ? 'Update' : 'Add'}</button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
  {#each kinds as k (k.kind)}
    <div class="vkind" id="vps-{k.kind}">
      <button type="button" class="vhead" aria-expanded={!!open[k.kind]} onclick={() => (open = { ...open, [k.kind]: !open[k.kind] })}>
        <Icon name={open[k.kind] ? 'chevD' : 'chevR'} size={15} />
        <span class="vlabel">{k.label}</span>
        <span class="vcount">{k.files.length}</span>
      </button>
      {#if open[k.kind]}
        <div class="vfiles">
          {#each k.files as f (f.id)}
            <div class="vfile" class:this={f.thisOne}>
              {#if f.picture}<img src={f.picture} alt="" loading="lazy" class={k.kind === 'tables' ? 'tall' : 'wide'} />{/if}
              <div class="vtext">
                <div class="vtitle">
                  {f.version ? (k.kind === 'roms' ? f.version : `Version ${f.version}`) : 'No version given'}{f.format && k.kind === 'tables' && f.format !== 'VPX' ? ` · ${f.format}` : f.format && k.kind !== 'tables' ? ` · ${f.format}` : ''}
                  {#if f.thisOne}<span class="yours">{mine}</span>{/if}
                </div>
                <div class="vsub">{f.authors.join(', ')}{f.added ? ` · on VPS since ${date(f.added)}` : ''}</div>
                {#if f.comment}<div class="vsub">{f.comment}</div>{/if}
                {#if f.features.length}<div class="vtags">{#each f.features as x (x)}<span class="tag">{x}</span>{/each}</div>{/if}
              </div>
              {#if f.links.length}
                <button type="button" class="btn small" title={f.links[0]} onclick={() => openLink(f.links[0])}><Icon name="ext" size={14} />Get</button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .kinds { overflow-y: auto; margin: 0 -8px; padding: 0 8px; display: flex; flex-direction: column; gap: 4px; }
  .vhead { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px 6px; border: 0; border-radius: 8px; background: transparent; text-align: left; font-weight: 600; }
  .vhead:hover { background: var(--hover); }
  .vlabel { flex: 1; }
  .vcount { color: var(--faint); font-size: 12px; font-weight: 400; }
  .vfiles { display: flex; flex-direction: column; gap: 6px; padding: 4px 0 10px 22px; }
  .vfile { display: flex; align-items: center; gap: 12px; padding: 8px 10px; border-radius: 10px; background: #161c27; border: 1px solid #222a3a; }
  .vfile.this { border-color: #2f6b4d; background: #142019; }
  .vfile img { flex: 0 0 auto; border-radius: 6px; object-fit: cover; background: var(--picture-bg); }
  .vfile img.tall { width: 40px; height: 70px; }
  .vfile img.wide { width: 80px; height: 60px; }
  .vtext { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .vtitle { font-weight: 600; font-size: 13px; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .yours { padding: 1px 6px; border-radius: 5px; background: #1f4a33; color: var(--good); font-size: 11px; font-weight: 700; }
  .vsub { color: var(--faint); font-size: 12px; overflow: hidden; text-overflow: ellipsis; }
  .vtags { display: flex; flex-wrap: wrap; gap: 4px; }
  .tag { padding: 1px 6px; border-radius: 5px; background: var(--tag-bg); color: var(--tag-fg); font-size: 10px; font-weight: 600; }
  .from { color: var(--faint); font-weight: 400; font-size: 12px; }
  .trophy { flex: 0 0 auto; width: 40px; height: 40px; border-radius: 8px; display: grid; place-items: center; background: var(--control); color: #ffc65c; }
</style>
