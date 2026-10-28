<script>
  // Make media (offered on every table, a window of checkboxes for what to make, all ticked): our
  // player draws the table with no window — its playfield, a twenty-second preview video of it starting
  // a game, with its sound, its backglass and its score display — and the screens draw a wheel from
  // its pictures. Those not ticked stay as
  // they were; what the app made of it before is replaced only once the user says so.
  import Icon from './Icon.svelte';
  import { app, makePictures, savePrefs } from './store.svelte.js';

  let { t, onclose } = $props();

  const KINDS = [
    ['playfield', 'Playfield picture', 'The playfield seen from straight above, flippers at the bottom, cut to the playfield as VPS’s pictures show tables.'],
    ['video', 'Playfield video, with its sound', 'Twenty seconds of it starting a game, seen the same way, with its sound (off until you turn it on), to preview it here, in front ends and in cabinet mode.'],
    ['backglass', 'Backglass picture', 'From its backglass file, lit as its script lights it.'],
    ['dmd', 'Score display picture', 'Its DMD or score digits, once it’s up.'],
    ['wheel', 'Wheel', 'A round logo for front ends: its backglass in a ring, its name across it.'],
  ];

  let ticked = $state(Object.fromEntries(KINDS.map(([k]) => [k, true])));

  const chosen = $derived(KINDS.filter(([k]) => ticked[k]).map(([k]) => k));
  const all = $derived(chosen.length === KINDS.length);

  function setAll(on) {
    for (const [k] of KINDS) ticked[k] = on;
  }

  // what of the ticked the app made before, asked about before it's replaced
  const again = $derived(KINDS.filter(([k]) => ticked[k] && (t.made || []).includes(k)));
  let asking = $state(false);

  function make() {
    if (again.length && !asking) {
      asking = true;
      return;
    }
    savePrefs();
    makePictures(t.id, { kinds: chosen });
    onclose();
  }
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="mm-title">
    <header>
      <h2 id="mm-title">Make media for {t.short}</h2>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </header>
    <p class="lead">Our player draws it with no window and no sound, in about a minute with the video. What you don’t tick stays as it is.</p>
    <label class="check all"><input type="checkbox" checked={all} indeterminate={!all && chosen.length > 0} onchange={(e) => setAll(e.currentTarget.checked)} />Everything</label>
    <div class="kinds">
      {#each KINDS as [k, label, what] (k)}
        <label class="kind">
          <input type="checkbox" bind:checked={ticked[k]} />
          <span><b>{label}</b><span class="what">{what}</span></span>
        </label>
      {/each}
    </div>
    {#if asking}
      <p class="again">The app made {again.length === 1 ? 'its' : 'these'} before: {again.map(([, label]) => label.toLowerCase()).join(', ')}. Replace {again.length === 1 ? 'it' : 'them'} with new ones?</p>
    {/if}
    <footer>
      <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
      <button type="button" class="btn go" disabled={!chosen.length || app.makingIds.includes(t.id)} onclick={make}><Icon name="image" size={16} />{asking ? 'Replace them' : `Make ${chosen.length === 1 ? 'it' : 'them'}`}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(560px, 100%); max-height: calc(100vh - 40px); overflow: auto; padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  .lead { margin: 0; color: var(--muted); font-size: 14px; }
  .check { display: flex; align-items: center; gap: 10px; font-size: 14px; color: var(--fg-soft); }
  .all { font-weight: 600; color: var(--fg); padding-bottom: 6px; border-bottom: 1px solid var(--line-soft); }
  .kinds { display: flex; flex-direction: column; gap: 4px; }
  .kind { display: flex; align-items: flex-start; gap: 10px; padding: 8px 4px; border-radius: 8px; cursor: pointer; }
  .kind:hover { background: var(--hover); }
  .kind input { margin-top: 3px; }
  .kind > span { display: flex; flex-direction: column; gap: 2px; }
  .what { color: var(--muted); font-size: 13px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  .go { background: var(--accent-bg); border-color: var(--accent); }
  .again { margin: 0; padding: 10px 12px; border-radius: 10px; background: #2a2414; color: #ffd27a; font-size: 13px; }
</style>
