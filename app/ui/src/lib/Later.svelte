<script>
  // A screen still to be built: what it will hold, said plainly.
  import Icon from './Icon.svelte';
  import { goLibrary } from './store.svelte.js';

  let { title, text } = $props();
</script>

<main>
  <div class="card">
    <h1 class="display">{title}</h1>
    <p>{text}</p>
    <button type="button" class="btn" onclick={goLibrary}><Icon name="left" />Back to the library</button>
  </div>
</main>

<style>
  main { flex: 1; display: flex; align-items: center; justify-content: center; padding: 40px 28px; }
  .card {
    max-width: 560px; padding: 32px; border-radius: 16px; background: var(--panel); border: 1px solid var(--panel-line);
    display: flex; flex-direction: column; gap: 14px; align-items: flex-start;
  }
  h1 { font-size: 40px; line-height: 1; }
  p { margin: 0; color: var(--muted); font-size: 15px; line-height: 1.6; }
</style>
