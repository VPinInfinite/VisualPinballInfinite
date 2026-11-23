<script>
  // A rating, one to five stars: shown, or set with a click where `onrate` is given — the star set
  // clicked again clears it — the pointer over a star showing what a click would set.
  import Icon from './Icon.svelte';

  let { value = 0, onrate = null, size = 18, label = 'Rating', title = '' } = $props();

  let over = $state(0);
  const shown = $derived(over || value || 0);
  const words = (n) => `${n} ${n === 1 ? 'star' : 'stars'}`;
</script>

{#if onrate}
  <div class="stars" role="group" aria-label={value ? `${label}: ${words(value)}` : `${label}: not rated`} title={title || undefined}>
    {#each [1, 2, 3, 4, 5] as n (n)}
      <button
        type="button"
        class:lit={n <= shown}
        aria-current={value === n ? 'true' : undefined}
        aria-label={value === n ? `${words(n)}: clear the rating` : `Rate it ${words(n)}`}
        title={value === n ? 'Clear the rating' : words(n)}
        onpointerenter={() => (over = n)}
        onpointerleave={() => (over = 0)}
        onfocus={() => (over = n)}
        onblur={() => (over = 0)}
        onclick={() => onrate(value === n ? 0 : n)}
      ><Icon name="star" {size} fill={n <= shown} /></button>
    {/each}
  </div>
{:else}
  <span class="stars still" role="img" aria-label={value ? `${words(value)} of 5` : 'Not rated'} title={title || undefined}>
    {#each [1, 2, 3, 4, 5] as n (n)}<span class:lit={n <= value}><Icon name="star" {size} fill={n <= value} /></span>{/each}
  </span>
{/if}

<style>
  .stars { display: inline-flex; align-items: center; gap: 1px; color: var(--faint); }
  .stars button { display: inline-flex; padding: 2px; border: 0; border-radius: 6px; background: none; color: var(--faint); cursor: pointer; }
  .stars button:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .lit, .stars button.lit { color: #f2c14e; }
  .still { gap: 0; }
  .still span { display: inline-flex; }
</style>
