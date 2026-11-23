<script>
  // Settings: its pages down the side. Folders a part of Library, the Displays page a part of
  // Picture with the display timing from Screens, PinUP packs' settings a page of their own (the
  // packs themselves under Tools, with the other managers: ROMs, media, colour DMDs, AltSound),
  // Cabinet and Advanced as they are; the ones built open, the rest say what they'll hold.
  import Icon from './Icon.svelte';
  import Screens from './Screens.svelte';
  import Sound from './Sound.svelte';
  import Controls from './Controls.svelte';
  import LibrarySettings from './LibrarySettings.svelte';
  import Picture from './Picture.svelte';
  import General from './General.svelte';
  import PinUP from './PinUP.svelte';
  import Machines from './Machines.svelte';
  import PhysicsSets from './PhysicsSets.svelte';
  import CabinetModeSettings from './CabinetModeSettings.svelte';
  import Cabinet from './Cabinet.svelte';
  import Advanced from './Advanced.svelte';
  import { app } from './store.svelte.js';

  const pages = [
    { id: 'general', label: 'General', icon: 'sliders' },
    { id: 'library', label: 'Library', icon: 'book' },
    { id: 'screens', label: 'Screens', icon: 'tv' },
    { id: 'picture', label: 'Picture', icon: 'image' },
    { id: 'controls', label: 'Controls', icon: 'disc' },
    { id: 'sound', label: 'Sound', icon: 'speaker' },
    { id: 'machines', label: 'Machines', icon: 'chip' },
    { id: 'pinup', label: 'PinUP packs', icon: 'layers' },
    { id: 'cabinet', label: 'Cabinet', icon: 'topper' },
    { id: 'advanced', label: 'Advanced', icon: 'code' },
    // a part of Advanced: it changes how tables play
    { id: 'physics', label: 'Physics sets', icon: 'gauge', sub: true },
  ];

  // an old page's name: Folders is a part of Library now, Displays a part of Picture
  const moved = { folders: 'library', displays: 'picture' };
  const current = $derived(pages.find((p) => p.id === (moved[app.settingsPage] ?? app.settingsPage)) || pages[2]);
  // the managers that were settings pages before: under Tools now
  $effect(() => {
    if (app.settingsPage === 'media' || app.settingsPage === 'roms') {
      app.toolsPage = app.settingsPage;
      app.settingsPage = 'screens';
      app.page = 'tools';
    }
  });
</script>

<div class="settings">
  <nav aria-label="Settings pages">
    {#each pages as p (p.id)}
      <button type="button" class="page" class:sub={p.sub} class:on={current.id === p.id} aria-current={current.id === p.id ? 'page' : undefined} onclick={() => (app.settingsPage = p.id)}>
        <Icon name={p.icon} size={17} />{p.label}
      </button>
    {/each}
  </nav>
  <div class="content">
    {#if app.vpxOlder && ['screens', 'sound', 'controls', 'picture', 'general', 'physics', 'cabinet', 'advanced'].includes(current.id)}
      <!-- VPX 10.8.0 plays the tables, reading its own settings file: these pages set 10.8.1's -->
      <div class="older" role="status">
        <Icon name="alert" size={17} />
        <span>VPX 10.8.0 plays your tables, and keeps its own settings: this page sets VPX {app.vpxInstalled.find((v) => v.addons)?.line ?? '10.8.1'}’s. Change 10.8.0’s in its own Preferences.</span>
        <button type="button" class="btn small ghost" onclick={() => (app.vpxChoice = { choosing: null, current: '10.8.0' })}>How they differ</button>
      </div>
    {/if}
    {#if current.id === 'screens'}
      <Screens />
    {:else if current.id === 'sound'}
      <Sound />
    {:else if current.id === 'controls'}
      <Controls />
    {:else if current.id === 'library'}
      <LibrarySettings />
    {:else if current.id === 'picture'}
      <Picture />
    {:else if current.id === 'general'}
      <General />
    {:else if current.id === 'pinup'}
      <PinUP />
    {:else if current.id === 'machines'}
      <Machines />
    {:else if current.id === 'physics'}
      <PhysicsSets />
    {:else if current.id === 'cabinet'}
      <CabinetModeSettings />
      <Cabinet />
    {:else if current.id === 'advanced'}
      <Advanced />
    {:else}
      <div class="later">
        <h1 class="display">{current.label}</h1>
        <p>{current.later}</p>
        <p class="soft">Every setting VPX reads already works behind the scenes; this page puts them on screen. It comes after the ones being built now.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .settings { flex: 1; display: flex; min-height: 0; }
  .older { display: flex; align-items: center; gap: 10px; margin: 18px 28px 0; padding: 10px 14px; border-radius: 10px; background: #2a2414; border: 1px solid #5a4a20; color: var(--warn); font-size: 14px; line-height: 1.4; }
  .older span { flex: 1; min-width: 0; }
  /* the pages down the side stay put while a page scrolls */
  nav {
    flex: 0 0 220px; padding: 18px 12px; background: var(--side); border-right: 1px solid var(--line);
    display: flex; flex-direction: column; gap: 2px;
    position: sticky; top: 61px; align-self: flex-start; height: calc(100vh - 61px); overflow-y: auto; box-sizing: border-box;
  }
  .page {
    display: flex; align-items: center; gap: 12px; height: 40px; padding: 0 12px; border: 0; border-radius: 8px;
    background: transparent; color: var(--fg-soft); font-size: 14px; text-align: left;
  }
  .page:hover { background: var(--hover); }
  .page.on { background: var(--selected); color: #fff; font-weight: 600; }
  .page.sub { height: 34px; margin-left: 22px; font-size: 13px; }
  .content { flex: 1; min-width: 0; overflow: auto; }
  .later { max-width: 640px; padding: 40px 36px; display: flex; flex-direction: column; gap: 12px; }
  .later h1 { font-size: 38px; line-height: 1; }
  .later p { margin: 0; color: var(--muted); font-size: 15px; line-height: 1.6; }
  .later .soft { color: var(--faint); font-size: 14px; }
</style>
