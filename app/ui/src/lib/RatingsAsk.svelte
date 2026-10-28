<script>
  // Asked on the first rating, where a front end's list could hold ratings too: whether the ratings
  // given here go in its list as well (`prefs.ratingsToFronts`), so it shows the same stars — and a
  // rating changed there then shows here. Settings › Library changes it later.
  import { app, answerRatings } from './store.svelte.js';

  const fronts = $derived(app.ratingFronts.join(' and '));
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ra-title">
    <h2 id="ra-title">Put your ratings in {fronts} too?</h2>
    <p>You gave {app.askRatings?.name ?? 'a table'} its first stars here. Your ratings always show in this app. Put in {fronts}’s list as well, they show there too, and a rating you change there shows here.</p>
    <p class="soft">Settings › Library changes this whenever you like.</p>
    <div class="row">
      <button type="button" class="btn go" onclick={() => answerRatings(true)}>Yes, put them there too</button>
      <button type="button" class="btn" onclick={() => answerRatings(false)}>No, keep them here</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(560px, 100%); padding: 22px 24px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  h2 { margin: 0; font-size: 20px; }
  p { margin: 0; color: var(--fg-soft); font-size: 14px; line-height: 1.55; }
  .soft { color: var(--faint); font-size: 13px; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 4px; }
  .btn.go { background: var(--accent-bg); border-color: var(--accent); color: #fff; }
</style>
