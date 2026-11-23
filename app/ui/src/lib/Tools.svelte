<script>
  // Tools: the managers, each for every table at once — ROMs, PinUP packs, media, color DMDs and
  // AltSound packs — their pages down the side, as Settings' are. A table's own page still adds,
  // makes and removes each of these for that table alone.
  import Icon from './Icon.svelte';
  import Roms from './Roms.svelte';
  import PupPacks from './PupPacks.svelte';
  import MediaSettings from './MediaSettings.svelte';
  import ColourManager from './ColourManager.svelte';
  import AltSoundManager from './AltSoundManager.svelte';
  import { app } from './store.svelte.js';

  const pages = [
    { id: 'roms', label: 'ROMs', icon: 'box' },
    { id: 'pinup', label: 'PinUP packs', icon: 'layers' },
    { id: 'media', label: 'Media', icon: 'film' },
    { id: 'colour', label: 'Color DMD', icon: 'palette' },
    { id: 'altsound', label: 'AltSound', icon: 'music' },
  ];
  const current = $derived(pages.find((p) => p.id === app.toolsPage) || pages[0]);
</script>

<div class="tools">
  <nav aria-label="Tools">
    {#each pages as p (p.id)}
      <button type="button" class="page" class:on={current.id === p.id} aria-current={current.id === p.id ? 'page' : undefined} onclick={() => (app.toolsPage = p.id)}>
        <Icon name={p.icon} size={17} />{p.label}
      </button>
    {/each}
  </nav>
  <div class="content">
    {#if current.id === 'roms'}
      <Roms />
    {:else if current.id === 'pinup'}
      <div class="frame">
        <header class="title">
          <h1 class="display">PinUP packs</h1>
          <p>Your packs, each with the tables it plays for, its layout and its status. How packs play is in Settings › PinUP packs; PinUP Popper’s media are under Media.</p>
        </header>
        <PupPacks />
      </div>
    {:else if current.id === 'media'}
      <MediaSettings />
    {:else if current.id === 'colour'}
      <ColourManager />
    {:else}
      <AltSoundManager />
    {/if}
  </div>
</div>

<style>
  .tools { flex: 1; display: flex; min-height: 0; }
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
  .content { flex: 1; min-width: 0; overflow: auto; }
  .frame { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 22px; max-width: 2200px; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
</style>
