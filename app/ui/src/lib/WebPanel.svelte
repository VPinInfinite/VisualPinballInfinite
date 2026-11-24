<script module>
  // the Get last loaded, by its number: loaded once, wherever the panel shows
  let loaded = 0;
</script>

<script>
  // A file's page inside the app, in the VPS browser's panel or beside a table's page: its bars —
  // back to what showed before, the site's own back, forward and reload, its address, Sign out, In
  // my browser — and the page itself, a webview of the app's window laid over this panel's page area
  // (`web_view.rs`), kept there as the window and the panel change size, out of sight while a window
  // of the app's own is over it. `edge`: the panel's width, the notices at the window's right kept
  // clear of it.
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';
  import { app, vps, drop, guard, notice } from './store.svelte.js';

  let { back = 'Back', onback, edge = 480 } = $props();

  let area = $state();
  // a list of the top bar's open over it, or a window of the app's own
  const listOver = $derived(vps.inboxOpen || vps.tasksOpen);
  const covered = $derived(listOver || drop.open || drop.dragging || !!guard.ask || !!app.report || !!app.removing || !!app.askRatings || !!app.cabinet || !!app.vpxChoice || app.setupOpen || app.setupAgain || app.status?.phase === 'setup');
  const showing = $derived(!!vps.web && !!area && !covered);
  function rect() {
    const r = area.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }
  $effect(() => {
    if (!showing) {
      invoke('web_hide');
      return;
    }
    // a page a Get asked for loaded once; where it goes from there, the web view tells (`vps.web`)
    const ask = vps.ask;
    if (ask && ask.n !== loaded) {
      loaded = ask.n;
      invoke('web_show', { url: ask.url, game: ask.game, ...rect() }).catch((e) => {
        notice(String(e), { kind: 'problem' });
        vps.web = null;
      });
    } else {
      invoke('web_place', rect()).catch(() => {});
    }
    const place = () => invoke('web_place', rect()).catch(() => {});
    const seen = new ResizeObserver(place);
    seen.observe(area);
    window.addEventListener('resize', place);
    return () => {
      seen.disconnect();
      window.removeEventListener('resize', place);
    };
  });
  onDestroy(() => invoke('web_hide'));
  $effect(() => {
    const root = document.documentElement;
    if (showing) root.style.setProperty('--web-edge', `${edge + 20}px`);
    else root.style.removeProperty('--web-edge');
    return () => root.style.removeProperty('--web-edge');
  });

  const go = (how) => invoke('web_go', { how }).catch((e) => notice(String(e), { kind: 'problem' }));
  const address = $derived.by(() => {
    try {
      const u = new URL(vps.web?.url || '');
      return u.host + (u.pathname === '/' ? '' : decodeURI(u.pathname));
    } catch {
      return vps.web?.url || '';
    }
  });
  function signOut() {
    invoke('web_sign_out').then(() => notice('Signed out of the sites shown in the app.', { kind: 'done' })).catch((e) => notice(String(e), { kind: 'problem' }));
  }
</script>

<div class="webbody">
  <div class="webbar">
    <button type="button" class="btn small ghost" onclick={onback} title="Back to what showed before"><Icon name="left" size={15} />{back}</button>
    <span class="grow"></span>
    <button type="button" class="btn small ghost" onclick={signOut} title="Signs you out of every site shown here">Sign out</button>
    <button type="button" class="btn small ghost" onclick={() => invoke('web_outside').catch((e) => notice(String(e), { kind: 'problem' }))} title="Opens this page in your own browser"><Icon name="ext" size={14} />In my browser</button>
  </div>
  <div class="webbar address">
    <button type="button" class="btn icon ghost small" aria-label="Back" title="Back" onclick={() => go('back')}><Icon name="chevL" size={16} /></button>
    <button type="button" class="btn icon ghost small" aria-label="Forward" title="Forward" onclick={() => go('forward')}><Icon name="chevR" size={16} /></button>
    <button type="button" class="btn icon ghost small" aria-label="Load it again" title="Load it again" onclick={() => go('reload')}><Icon name="refresh" size={15} /></button>
    <span class="url" class:loading={vps.web?.loading} title={vps.web?.url}>{address}</span>
  </div>
  <div class="page" bind:this={area}>
    {#if covered && !listOver}<p class="quiet">The page shows again once this window closes.</p>{/if}
  </div>
</div>

<style>
  .webbody { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .webbar { display: flex; align-items: center; gap: 6px; padding: 8px 10px 0 6px; }
  .webbar.address { padding-bottom: 8px; border-bottom: 1px solid var(--line); }
  .grow { flex: 1; }
  .url { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 5px 10px; border-radius: 8px; background: #141a26; color: var(--fg-soft); font-size: 12.5px; }
  .url.loading { color: var(--faint); }
  .page { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; background: #0f131b; }
  .quiet { color: var(--muted); margin: 0; }
</style>
