<script>
  // The player hub: the player's own, for every table in one place — high scores, achievements and
  // stats — its pages down the side, as Settings' are. A table's page shows its own.
  import Icon from './Icon.svelte';
  import AllHighScores from './AllHighScores.svelte';
  import Achievements from './Achievements.svelte';
  import Stats from './Stats.svelte';
  import { app } from './store.svelte.js';

  const pages = [
    { id: 'scores', label: 'High scores', icon: 'star' },
    { id: 'achievements', label: 'Achievements', icon: 'trophy' },
    { id: 'stats', label: 'Stats', icon: 'clock' },
  ];
  const current = $derived(pages.find((p) => p.id === app.hubPage) || pages[0]);
</script>

<div class="hub">
  <nav aria-label="Player hub">
    {#each pages as p (p.id)}
      <button type="button" class="page" class:on={current.id === p.id} aria-current={current.id === p.id ? 'page' : undefined} onclick={() => (app.hubPage = p.id)}>
        <Icon name={p.icon} size={17} />{p.label}
      </button>
    {/each}
  </nav>
  <div class="content">
    {#if current.id === 'scores'}
      <AllHighScores />
    {:else if current.id === 'achievements'}
      <Achievements />
    {:else}
      <Stats />
    {/if}
  </div>
</div>

<style>
  .hub { flex: 1; display: flex; min-height: 0; }
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
</style>
