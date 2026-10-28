<script>
  // A search box for a list: what's typed narrows the list as it's typed, the × clears it. The
  // Tools pages each search their rows by table, set or pack name with it.
  import Icon from './Icon.svelte';

  let { value = $bindable(''), placeholder = 'Search', label = placeholder } = $props();
</script>

<label class="search">
  <Icon name="search" size={15} />
  <input type="search" bind:value {placeholder} aria-label={label} spellcheck="false" />
  {#if value}<button type="button" class="clear" aria-label="Clear the search" title="Clear" onclick={() => (value = '')}><Icon name="x" size={14} /></button>{/if}
</label>

<style>
  .search { position: relative; display: inline-flex; align-items: center; gap: 8px; height: 34px; padding: 0 10px; border-radius: 8px; background: var(--control); border: 1px solid var(--control-line); color: var(--faint); min-width: 220px; }
  .search:focus-within { border-color: var(--accent); }
  input { flex: 1; min-width: 0; height: 100%; padding: 0; border: 0; outline: 0; background: transparent; color: var(--fg); font: inherit; font-size: 13px; }
  input::-webkit-search-cancel-button { display: none; }
  .clear { display: grid; place-items: center; width: 22px; height: 22px; padding: 0; border: 0; border-radius: 6px; background: transparent; color: var(--faint); }
  .clear:hover { background: var(--hover); color: var(--fg); }
</style>
