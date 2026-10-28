<script>
  // A table's details edited ("Edit its details"): its name, maker, year, type, version and authors as the app shows them, each
  // the user's own over what VPS and the table's file say — kept in VPI's library alone, the table's
  // file and other programs left as they are. A field left empty shows VPS's again.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, onclose } = $props();

  const TYPES = [
    ['', 'As VPS has it'],
    ['SS', 'Solid state (SS)'],
    ['EM', 'Electro-mechanical (EM)'],
    ['PM', 'Pure mechanical (PM)'],
    ['DG', 'Digital (DG)'],
    ['Original', 'Original'],
  ];

  const own = t.ownDetails || {};
  const usual = t.usualDetails || {};
  let name = $state(own.name ?? '');
  let maker = $state(own.maker ?? '');
  let year = $state(own.year != null ? String(own.year) : '');
  let badge = $state(own.badge ?? '');
  let version = $state(own.version ?? '');
  let authors = $state(own.authors ? own.authors.join(', ') : '');
  let busy = $state(false);

  const yearOk = $derived(!year.trim() || /^\d{4}$/.test(year.trim()));
  const usualType = $derived(TYPES.find(([v]) => v && v === usual.badge)?.[1]);

  async function save(details) {
    busy = true;
    try {
      await invoke('set_details', { id: t.id, details });
      notice(Object.keys(details).length ? `${t.short}'s details are kept.` : `${t.short} shows VPS's details again.`, { kind: 'done' });
      onclose();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  function keep() {
    const d = {};
    const text = (v) => v.trim() || undefined;
    if (text(name)) d.name = text(name);
    if (text(maker)) d.maker = text(maker);
    if (year.trim()) d.year = Number(year.trim());
    if (badge) d.badge = badge;
    if (text(version)) d.version = text(version);
    const a = authors.split(',').map((s) => s.trim()).filter(Boolean);
    if (a.length) d.authors = a;
    save(d);
  }

  function key(e) {
    if (e.key === 'Escape' && !busy) onclose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim">
  <div class="win" role="dialog" aria-modal="true" aria-labelledby="ed-title">
    <header>
      <div>
        <h1 id="ed-title" class="display">Edit {t.short}’s details</h1>
        <p>As the app shows them. They stay in VPI: the table’s file and other programs keep theirs. Leave a field empty for VPS’s.</p>
      </div>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose} disabled={busy}><Icon name="x" /></button>
    </header>
    <form class="body" onsubmit={(e) => { e.preventDefault(); if (yearOk) keep(); }}>
      <label class="field wide"><span>Name</span><input class="select" bind:value={name} placeholder={usual.name || ''} /></label>
      <label class="field"><span>Maker</span><input class="select" bind:value={maker} placeholder={usual.maker || ''} /></label>
      <label class="field small"><span>Year</span><input class="select" class:bad={!yearOk} bind:value={year} inputmode="numeric" maxlength="4" placeholder={usual.year ? String(usual.year) : ''} /></label>
      <label class="field">
        <span>Type</span>
        <select class="select" bind:value={badge}>
          {#each TYPES as [v, label] (v)}<option value={v}>{v ? label : usualType ? `${label}: ${usualType}` : label}</option>{/each}
        </select>
      </label>
      <label class="field small"><span>Version</span><input class="select" bind:value={version} placeholder={usual.version || ''} /></label>
      <label class="field wide"><span>Authors, with commas between</span><input class="select" bind:value={authors} placeholder={(usual.authors || []).join(', ')} /></label>
      {#if !yearOk}<p class="problem wide">A year is four digits, such as 1995.</p>{/if}
      <button type="submit" hidden aria-hidden="true"></button>
    </form>
    <footer>
      {#if t.ownDetails}<button type="button" class="btn ghost back" onclick={() => save({})} disabled={busy}>Back to VPS’s details</button>{/if}
      <button type="button" class="btn ghost" onclick={onclose} disabled={busy}>Cancel</button>
      <button type="button" class="btn save" onclick={keep} disabled={busy || !yearOk}><Icon name="check" size={16} />Keep them</button>
    </footer>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 45; background: rgba(5, 7, 11, 0.72); display: flex; align-items: center; justify-content: center; padding: 28px; }
  .win { width: min(640px, 100%); max-height: 100%; display: flex; flex-direction: column; background: var(--panel); border: 1px solid var(--panel-line); border-radius: 18px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6); }
  header { display: flex; justify-content: space-between; gap: 16px; padding: 22px 24px 14px; border-bottom: 1px solid var(--line-soft); }
  h1 { font-size: 28px; line-height: 1.05; margin: 0; }
  header p { margin: 6px 0 0; color: var(--muted); }
  .body { overflow: auto; padding: 18px 24px; display: grid; grid-template-columns: 1fr 1fr; gap: 14px 16px; }
  .field { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .field span { font-size: 13px; color: var(--muted); }
  .field input, .field select { height: 38px; width: 100%; box-sizing: border-box; }
  .wide { grid-column: 1 / -1; }
  input.bad { border-color: #c0566a; }
  .problem { margin: 0; color: #f0a0b0; font-size: 13px; }
  footer { display: flex; gap: 12px; justify-content: flex-end; padding: 14px 24px 18px; border-top: 1px solid var(--line-soft); }
  .back { margin-right: auto; }
  .save { background: var(--accent-bg); border-color: var(--accent); display: inline-flex; gap: 6px; align-items: center; }
</style>
