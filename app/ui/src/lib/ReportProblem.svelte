<script>
  // Report a problem: what happened in the user's words, with the app's and the
  // player's logs, the settings (a PIN2DMD key never), this PC's facts and, from a table's page, the
  // table's — gathered into one zip the user looks through and attaches to a post themselves, or, in
  // a build that sends reports, sends to the team with Send it to us. Nothing is sent without that
  // press; their own folder is taken out of every path.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { table = null, onclose } = $props();

  // whether this build sends reports at all: a build made from the source alone only makes them
  let canSend = $state(false);
  invoke('report_can_send').then((yes) => (canSend = yes)).catch(() => (canSend = false));

  let note = $state('');
  let logs = $state(true);
  let settings = $state(true);
  let pc = $state(true);
  let busy = $state(false);
  let made = $state(null);
  let sending = $state(false);
  let sent = $state(false);

  async function make() {
    busy = true;
    try {
      made = await invoke('make_report', { asked: { note, table: table?.id ?? null, logs, settings, pc } });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  const show = () => invoke('show_report', { path: made }).catch((e) => notice(String(e), { kind: 'problem' }));

  async function send() {
    sending = true;
    try {
      await invoke('send_report', { path: made, note, table: table?.name ?? null });
      sent = true;
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    sending = false;
  }

  function key(e) {
    if (e.key === 'Escape' && !busy) onclose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim">
  <div class="win" role="dialog" aria-modal="true" aria-labelledby="rp-title">
    <header>
      <div>
        <h1 id="rp-title" class="display">Report a problem{table ? ` with ${table.short}` : ''}</h1>
        <p>{canSend ? 'Makes one file to send to us, or to attach to your post in the Discord server\'s #bug-reports or a GitHub issue. Nothing is sent until you say: you see it first.' : 'Makes one file to attach to your post in the Discord server\'s #bug-reports or a GitHub issue. You see it first.'}</p>
      </div>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose} disabled={busy}><Icon name="x" /></button>
    </header>

    {#if made}
      <div class="body">
        <p class="done"><Icon name="check" size={18} width={2.4} />Made: {made.split('\\').pop()}</p>
        {#if canSend}
          <p class="soft">In your Visual Pinball Infinite data folder, under Logs\Reports. Show it to look through what's in it, then attach it to your post with what you were doing when it happened — or send it to us: its text goes, with your note, to the Visual Pinball Infinite team's private channel on Discord, where only the team sees it.</p>
        {:else}
          <p class="soft">In your Visual Pinball Infinite data folder, under Logs\Reports. Show it to look through what's in it, then attach it to your post with what you were doing when it happened.</p>
        {/if}
        {#if sent}<p class="done"><Icon name="check" size={18} width={2.4} />Sent. Thank you!</p>{/if}
      </div>
      <footer>
        <button type="button" class="btn ghost" onclick={onclose} disabled={sending}>Close</button>
        <button type="button" class="btn ghost" onclick={show}><Icon name="folder" size={16} />Show it</button>
        {#if canSend}
          <button type="button" class="btn save" onclick={send} disabled={sending || sent}><Icon name="upload" size={16} />{sending ? 'Sending…' : sent ? 'Sent' : 'Send it to us'}</button>
        {/if}
      </footer>
    {:else}
      <div class="body">
        <label class="what">
          <span>What happened?</span>
          <textarea bind:value={note} rows="5" placeholder={table ? 'The backglass stayed dark after the second ball…' : 'What you did, what happened, what you expected…'}></textarea>
        </label>
        <fieldset>
          <legend>What goes in it</legend>
          <label class="check"><input type="checkbox" bind:checked={logs} />The app's and the player's logs, from this time and the time before</label>
          <label class="check"><input type="checkbox" bind:checked={settings} />Your settings (a PIN2DMD key never)</label>
          <label class="check"><input type="checkbox" bind:checked={pc} />This PC: Windows, processor, memory, graphics card and screens</label>
          {#if table}<label class="check"><input type="checkbox" checked disabled />{table.short}: what its page shows, and its own settings</label>{/if}
        </fieldset>
        <p class="soft">Your Windows user folder is taken out of every path in it.</p>
      </div>
      <footer>
        <button type="button" class="btn ghost" onclick={onclose} disabled={busy}>Cancel</button>
        <button type="button" class="btn save" onclick={make} disabled={busy}><Icon name="download" size={16} />{busy ? 'Making it…' : 'Make the report'}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 45; background: rgba(5, 7, 11, 0.72); display: flex; align-items: center; justify-content: center; padding: 28px; }
  .win { width: min(640px, 100%); max-height: 100%; display: flex; flex-direction: column; background: var(--panel); border: 1px solid var(--panel-line); border-radius: 18px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6); }
  header { display: flex; justify-content: space-between; gap: 16px; padding: 22px 24px 14px; border-bottom: 1px solid var(--line-soft); }
  h1 { font-size: 28px; line-height: 1.05; margin: 0; }
  header p { margin: 6px 0 0; color: var(--muted); }
  .body { overflow: auto; padding: 16px 24px; display: flex; flex-direction: column; gap: 14px; }
  .what { display: flex; flex-direction: column; gap: 6px; }
  .what span { font-weight: 600; }
  textarea { width: 100%; box-sizing: border-box; resize: vertical; padding: 10px 12px; border-radius: 10px; background: var(--bg); border: 1px solid var(--control-line); color: var(--fg); font: inherit; font-size: 14px; }
  fieldset { border: 1px solid var(--line-soft); border-radius: 12px; padding: 10px 14px 12px; margin: 0; display: flex; flex-direction: column; gap: 8px; }
  legend { padding: 0 6px; color: var(--muted); font-size: 13px; }
  .check { display: flex; gap: 8px; align-items: flex-start; font-size: 14px; color: var(--fg-soft); }
  .check input { accent-color: var(--accent); margin-top: 3px; }
  .soft { margin: 0; color: var(--faint); font-size: 13px; }
  .done { margin: 0; display: flex; gap: 8px; align-items: center; color: var(--good); font-weight: 600; word-break: break-all; }
  footer { display: flex; gap: 12px; justify-content: flex-end; padding: 14px 24px 18px; border-top: 1px solid var(--line-soft); }
  .save { background: var(--accent-bg); border-color: var(--accent); }
</style>
