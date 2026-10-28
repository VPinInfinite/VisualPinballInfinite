<script>
  // The question asked before an achievement pack's changes would be lost: leaving the achievements
  // editor, another page, another table or pack, or closing the app — save the draft and go on,
  // leave without saving, or stay (`guard`).
  import { guard, leaveSaving, leaveDropping } from './store.svelte.js';
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="ua-title" aria-describedby="ua-what">
    <h2 id="ua-title">Your achievement pack isn’t saved</h2>
    <p id="ua-what">What you’ve changed in the achievements editor since you last saved {guard.closing ? 'is lost if the app closes now' : 'is lost if you leave without saving'}. Saving keeps it as a draft, with no version, to come back to.</p>
    <div class="row">
      <button type="button" class="btn go" onclick={leaveSaving}>Save the draft and {guard.closing ? 'close' : 'go on'}</button>
      <button type="button" class="btn" onclick={leaveDropping}>{guard.closing ? 'Close without saving' : 'Leave without saving'}</button>
      <button type="button" class="btn ghost" onclick={() => (guard.ask = null)}>Stay</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 80; background: rgba(5, 7, 10, 0.62); display: flex; align-items: center; justify-content: center; padding: 20px; }
  .dialog { width: min(520px, 100%); padding: 20px 22px; border-radius: 16px; background: #131822; border: 1px solid #5a4a1c; box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 12px; }
  h2 { margin: 0; font-size: 19px; }
  p { margin: 0; color: var(--fg-soft); font-size: 14px; line-height: 1.5; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; }
  .btn.go { background: var(--accent-bg); border-color: var(--accent); color: #fff; }
</style>
