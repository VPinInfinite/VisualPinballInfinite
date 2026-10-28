<script>
  // Making an AltSound pack from a table's ROM: what it does and what it takes, asked before it
  // starts; made in the background, the tasks bar showing how far it's come (`make_altsound`).
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, rom, onclose } = $props();

  function make() {
    const name = t.short || t.name;
    invoke('make_altsound', { id: t.id })
      .then(() => notice(`${name}’s AltSound pack is made.`))
      .catch((e) => notice(String(e), { kind: 'problem' }));
    onclose();
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ma-title">
    <header>
      <h2 id="ma-title">Make an AltSound pack from its ROM</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    <p>Every sound {rom}’s sound board can play is recorded, one at a time, into a pack this table plays in place of PinMAME’s sound, at the volume the machine plays out of the box. It sounds about the same as before; each sound becomes a file you can listen to, or replace with your own.</p>
    <ul class="facts">
      <li><Icon name="clock" size={16} />A minute or so, in the background: the tasks bar shows how far it’s come.</li>
      <li><Icon name="folder" size={16} />Up to a few hundred MB, in VPinMAME’s altsound folder, where the player finds it.</li>
      <li><Icon name="music" size={16} />Made by rom2altsound (BSD-3-Clause), which comes with the app.</li>
    </ul>
    <footer>
      <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
      <button type="button" class="btn go" onclick={make}>Make the pack</button>
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(560px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  p { margin: 0; color: var(--fg-soft); line-height: 1.5; }
  .facts { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 8px; font-size: 14px; color: var(--fg-soft); }
  .facts li { display: flex; align-items: center; gap: 10px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
