<script>
  // A read-me or a manual that came with a table, read in the app: a PDF in the window's own
  // viewer, a text file as text (the app serves its words as UTF-8, whatever it was written in).
  // Several of one row's files as tabs.
  import Icon from './Icon.svelte';

  let { title, docs, onclose } = $props();

  let at = $state(0);
  let words = $state('');
  let problem = $state('');
  let loading = $state(false);
  const doc = $derived(docs[at]);

  $effect(() => {
    const d = doc;
    words = '';
    problem = '';
    if (!d || d.pdf) return;
    loading = true;
    fetch(d.url)
      .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status}`))))
      .then((t) => (words = t))
      .catch(() => (problem = `${d.name} couldn’t be read.`))
      .finally(() => (loading = false));
  });

  function key(e) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="reader-title">
    <header>
      <h2 id="reader-title">{title}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    {#if docs.length > 1}
      <div class="tabs" role="tablist">
        {#each docs as d, i (d.url)}
          <button type="button" role="tab" class="tab" class:on={i === at} aria-selected={i === at} onclick={() => (at = i)}><Icon name="file" size={14} />{d.name}</button>
        {/each}
      </div>
    {:else}
      <p class="name"><Icon name="file" size={14} />{doc?.name}</p>
    {/if}
    {#if doc?.pdf}
      <iframe class="pdf" src={doc.url} title={doc.name}></iframe>
    {:else if problem}
      <p class="problem">{problem}</p>
    {:else if loading}
      <p class="soft">Reading…</p>
    {:else}
      <pre class="text">{words}</pre>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(1100px, 100%); height: calc(100vh - 40px); padding: 18px 20px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 10px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .tabs { display: flex; flex-wrap: wrap; gap: 6px; }
  .tab { display: inline-flex; align-items: center; gap: 6px; padding: 6px 10px; border-radius: 8px; border: 1px solid var(--control-line); background: transparent; color: var(--fg-soft); font: inherit; font-size: 13px; cursor: pointer; }
  .tab.on { background: var(--selected); color: var(--fg); border-color: var(--accent); }
  .name { margin: 0; display: inline-flex; align-items: center; gap: 6px; color: var(--muted); font-size: 13px; }
  .pdf { flex: 1; width: 100%; border: 0; border-radius: 10px; background: #fff; }
  .text { flex: 1; margin: 0; overflow: auto; padding: 14px 16px; border-radius: 10px; background: #0d1118; border: 1px solid var(--panel-line); color: var(--fg); font: 13px/1.5 Consolas, ui-monospace, monospace; white-space: pre-wrap; word-break: break-word; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
</style>
