<script>
  // Report a problem: what happened in the user's words, with the app's and the
  // player's logs, the settings (a PIN2DMD key never), this PC's facts and, from a table's page, the
  // table's — gathered into one zip the user looks through and attaches to a post themselves, or, in
  // a build that sends reports, sends to the team with Send it to us. Nothing is sent without that
  // press; their own folder is taken out of every path. Your reports: the reports sent from here,
  // each opening to the team's replies and the user's own, with a box to reply in.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { app, notice, checkReports } from './store.svelte.js';

  let { table = null, open = null, list = false, onclose } = $props();

  // whether this build sends reports at all: a build made from the source alone only makes them
  let canSend = $state(false);
  invoke('report_can_send').then((yes) => (canSend = yes)).catch(() => (canSend = false));

  // which part shows: a new report, the reports sent, or one of them (its key) — as the window was
  // opened for, then as the user goes
  const first = () => (open && app.reports.some((r) => r.key === open) ? open : null);
  let shown = $state(first());
  const firstView = () => (shown ? 'one' : list && app.reports.length ? 'list' : 'new');
  let view = $state(firstView());
  const current = $derived(app.reports.find((r) => r.key === shown) ?? null);

  // the team's newest replies taken in as the window opens
  checkReports({ quiet: true });

  let note = $state('');
  let logs = $state(true);
  let settings = $state(true);
  let pc = $state(true);
  let busy = $state(false);
  let made = $state(null);
  let sending = $state(false);
  let sent = $state(null);

  async function make() {
    busy = true;
    try {
      made = await invoke('make_report', { asked: { note, table: table?.id ?? null, logs, settings, pc } });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  const show = (path) => invoke('show_report', { path }).catch((e) => notice(String(e), { kind: 'problem' }));

  async function send() {
    sending = true;
    try {
      app.reports = await invoke('send_report', { path: made, note, table: table?.name ?? null });
      sent = app.reports.find((r) => r.file === made) ?? { answerable: false };
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    sending = false;
  }

  // one report opened: its replies count as read
  function openOne(key) {
    shown = key;
    view = 'one';
    draft = '';
  }
  $effect(() => {
    if (view === 'one' && current?.unread) {
      invoke('report_read', { key: current.key })
        .then((all) => (app.reports = all))
        .catch(() => {});
    }
  });

  let draft = $state('');
  let replying = $state(false);
  const MOST = 1500;

  async function reply() {
    if (!current || !draft.trim()) return;
    replying = true;
    const key = current.key;
    try {
      const now = await invoke('report_reply', { key, text: draft });
      app.reports = app.reports.map((r) => (r.key === key ? now : r));
      draft = '';
    } catch (e) {
      notice(String(e), { kind: 'problem' });
      // closed by the team meanwhile: shown so
      app.reports = await invoke('reports_list').catch(() => app.reports);
    }
    replying = false;
  }

  const day = (at) => new Date(at).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
  const moment = (at) => new Date(at).toLocaleString(undefined, { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' });
  const firstLine = (text) => (text || '').trim().split('\n')[0] || '(nothing written)';
  const teamCount = (r) => r.messages.filter((m) => m.from === 'team').length;

  // where a report stands, in a word or two
  function standing(r) {
    if (r.unread) return { kind: 'new', text: r.unread === 1 ? 'New reply' : `${r.unread} new replies` };
    if (r.closed) return { kind: 'closed', text: 'Closed' };
    if (teamCount(r)) return { kind: 'replied', text: 'Replied' };
    if (!r.answerable) return { kind: 'sent', text: 'Sent' };
    return { kind: 'waiting', text: 'No reply yet' };
  }

  // a message's words, its web links as buttons that open them in the browser
  const LINK = /https:\/\/[^\s<>"')\]]+/g;
  function pieces(text) {
    const out = [];
    let at = 0;
    for (const m of text.matchAll(LINK)) {
      if (m.index > at) out.push({ text: text.slice(at, m.index) });
      out.push({ link: m[0] });
      at = m.index + m[0].length;
    }
    if (at < text.length) out.push({ text: text.slice(at) });
    return out;
  }
  const openLink = (url) => invoke('open_link', { url }).catch((e) => notice(String(e), { kind: 'problem' }));

  const unread = $derived(app.reports.reduce((n, r) => n + (r.unread || 0), 0));

  function key(e) {
    if (e.key === 'Escape' && !busy && !sending && !replying) onclose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim">
  <div class="win" role="dialog" aria-modal="true" aria-labelledby="rp-title">
    <header>
      <div class="top">
        <div>
          <h1 id="rp-title" class="display">Report a problem{table && view === 'new' ? ` with ${table.short}` : ''}</h1>
          {#if view === 'new'}
            <p>{canSend ? 'Makes one file to send to us, or to attach to your post in the Discord server\'s #bug-reports or a GitHub issue. Nothing is sent until you say: you see it first.' : 'Makes one file to attach to your post in the Discord server\'s #bug-reports or a GitHub issue. You see it first.'}</p>
          {:else}
            <p>The reports you sent from this PC, with the team's replies. Only the team sees them.</p>
          {/if}
        </div>
        <button type="button" class="btn icon ghost" aria-label="Close" onclick={onclose} disabled={busy || sending || replying}><Icon name="x" /></button>
      </div>
      {#if app.reports.length}
        <div class="tabs" role="tablist">
          <button type="button" role="tab" aria-selected={view === 'new'} class:on={view === 'new'} onclick={() => (view = 'new')}>New report</button>
          <button type="button" role="tab" aria-selected={view !== 'new'} class:on={view !== 'new'} onclick={() => (view = 'list')}>
            Your reports<span class="n">{app.reports.length}</span>{#if unread}<span class="new">{unread} new</span>{/if}
          </button>
        </div>
      {/if}
    </header>

    {#if view === 'list'}
      <div class="body">
        <ul class="sent">
          {#each app.reports as r (r.key)}
            {@const s = standing(r)}
            <li>
              <button type="button" class="item" class:fresh={r.unread} onclick={() => openOne(r.key)}>
                <span class="when">{day(r.at)}</span>
                <span class="what"><b>{r.table || 'The app'}</b><span class="first">{firstLine(r.note)}</span></span>
                <span class="state {s.kind}">{#if r.unread}<span class="dot" aria-hidden="true"></span>{/if}{s.text}</span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
      <footer>
        <button type="button" class="btn ghost" onclick={onclose}>Close</button>
        <button type="button" class="btn save" onclick={() => (view = 'new')}><Icon name="plus" size={16} />New report</button>
      </footer>
    {:else if view === 'one' && current}
      <div class="body convo">
        <div class="head">
          <button type="button" class="btn small ghost back" onclick={() => (view = 'list')}><Icon name="left" size={16} />Your reports</button>
          <span class="meta">{current.table || 'The app'} · sent {moment(current.at)} · version {current.version}</span>
        </div>
        <div class="msg you">
          <span class="who">You wrote</span>
          <p class="words">{current.note.trim() || '(nothing written)'}</p>
        </div>
        {#each current.messages as m, i (i)}
          <div class="msg {m.from}">
            <span class="who">{m.from === 'team' ? 'The Visual Pinball Infinite team' : 'You'} · {moment(m.at)}</span>
            <p class="words">{#each pieces(m.text) as p, j (j)}{#if p.link}<button type="button" class="link" onclick={() => openLink(p.link)}>{p.link}</button>{:else}{p.text}{/if}{/each}</p>
          </div>
        {/each}
        {#if !current.answerable}
          <p class="soft">This report went before the app could take the team's replies, so they can't answer it here.</p>
        {:else if current.closed}
          <p class="soft">The team has closed this report. If it's still happening, send a new one.</p>
        {:else if !teamCount(current)}
          <p class="soft">No reply yet. The team's replies show here, and the bug button at the top says when one comes.</p>
        {/if}
      </div>
      {#if current.answerable && !current.closed && canSend}
        <footer class="replying">
          <label class="what">
            <span>Reply to the team</span>
            <textarea bind:value={draft} rows="3" maxlength={MOST} placeholder="Anything to add, or how it went…" disabled={replying}></textarea>
          </label>
          <div class="row">
            {#if current.file}<button type="button" class="btn ghost" onclick={() => show(current.file)}><Icon name="folder" size={16} />Show the report</button>{/if}
            <button type="button" class="btn save" onclick={reply} disabled={replying || !draft.trim()}><Icon name="upload" size={16} />{replying ? 'Sending…' : 'Send reply'}</button>
          </div>
        </footer>
      {:else}
        <footer>
          {#if current.file}<button type="button" class="btn ghost" onclick={() => show(current.file)}><Icon name="folder" size={16} />Show the report</button>{/if}
          <button type="button" class="btn ghost" onclick={() => (view = 'list')}>Back</button>
        </footer>
      {/if}
    {:else if made}
      <div class="body">
        <p class="done"><Icon name="check" size={18} width={2.4} />Made: {made.split('\\').pop()}</p>
        {#if canSend}
          <p class="soft">In your Visual Pinball Infinite data folder, under Logs\Reports. Show it to look through what's in it, then attach it to your post with what you were doing when it happened — or send it to us: its text goes, with your note, to the Visual Pinball Infinite team's private channel on Discord, where only the team sees it.</p>
        {:else}
          <p class="soft">In your Visual Pinball Infinite data folder, under Logs\Reports. Show it to look through what's in it, then attach it to your post with what you were doing when it happened.</p>
        {/if}
        {#if sent}
          <p class="done"><Icon name="check" size={18} width={2.4} />Sent. Thank you!</p>
          {#if sent.answerable}<p class="soft">The team's replies come back here, under Your reports, and the bug button at the top says when one comes. You can reply to them there.</p>{/if}
        {/if}
      </div>
      <footer>
        <button type="button" class="btn ghost" onclick={onclose} disabled={sending}>Close</button>
        <button type="button" class="btn ghost" onclick={() => show(made)}><Icon name="folder" size={16} />Show it</button>
        {#if sent?.answerable}
          <button type="button" class="btn ghost" onclick={() => (view = 'list')}>Your reports</button>
        {/if}
        {#if canSend}
          <button type="button" class="btn save" onclick={send} disabled={sending || !!sent}><Icon name="upload" size={16} />{sending ? 'Sending…' : sent ? 'Sent' : 'Send it to us'}</button>
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
          <label class="check"><input type="checkbox" bind:checked={logs} />The app's and VPX's logs, from this time and the time before</label>
          <label class="check"><input type="checkbox" bind:checked={settings} />Your settings, with VPX's and VPinMAME's folders (a PIN2DMD key never)</label>
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
  header { display: flex; flex-direction: column; gap: 12px; padding: 22px 24px 0; border-bottom: 1px solid var(--line-soft); }
  header .top { display: flex; justify-content: space-between; gap: 16px; padding-bottom: 2px; }
  header:not(:has(.tabs)) { padding-bottom: 14px; }
  h1 { font-size: 28px; line-height: 1.05; margin: 0; }
  header p { margin: 6px 0 0; color: var(--muted); }
  .tabs { display: flex; gap: 4px; margin-bottom: -1px; }
  .tabs button {
    display: inline-flex; align-items: center; gap: 8px; padding: 9px 14px; border: 0; border-bottom: 2px solid transparent;
    background: none; color: var(--muted); font: inherit; font-size: 14px; font-weight: 500; cursor: pointer;
  }
  .tabs button:hover { color: var(--fg); }
  .tabs button.on { color: var(--fg); font-weight: 600; border-bottom-color: var(--accent); }
  .tabs .n { color: var(--faint); font-weight: 500; }
  .tabs .new { padding: 1px 7px; border-radius: 9px; background: var(--accent); color: #061521; font-size: 11px; font-weight: 700; }
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

  /* the reports sent */
  .sent { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .item {
    width: 100%; display: grid; grid-template-columns: 96px minmax(0, 1fr) auto; align-items: center; gap: 14px;
    padding: 12px 14px; border-radius: 12px; border: 1px solid var(--card-line); background: var(--card);
    color: var(--fg); font: inherit; text-align: left; cursor: pointer;
  }
  .item:hover { background: var(--hover); }
  .item.fresh { border-color: color-mix(in srgb, var(--accent) 55%, var(--card-line)); }
  .item .when { color: var(--muted); font-size: 13px; }
  .item .what { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .item .what b { font-size: 14px; }
  .item .first { color: var(--fg-soft); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .state { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; color: var(--faint); white-space: nowrap; }
  .state.new { color: var(--accent); font-weight: 600; }
  .state.replied { color: var(--good); }
  .state .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }

  /* one report and its replies */
  .convo { gap: 12px; }
  .convo .head { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .back { padding-left: 6px; }
  .meta { color: var(--muted); font-size: 13px; }
  .msg { display: flex; flex-direction: column; gap: 4px; padding: 10px 14px; border-radius: 12px; border: 1px solid var(--line-soft); background: var(--bg); max-width: 88%; }
  .msg.you { align-self: flex-end; background: var(--control); }
  .msg.team { align-self: flex-start; border-color: color-mix(in srgb, var(--accent) 45%, var(--line-soft)); background: color-mix(in srgb, var(--accent-bg) 55%, var(--bg)); }
  .msg .who { font-size: 12px; color: var(--muted); }
  .msg.team .who { color: var(--accent-soft); font-weight: 600; }
  .words { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 14px; color: var(--fg); }
  .link { display: inline; padding: 0; border: 0; background: none; color: var(--accent-soft); font: inherit; text-decoration: underline; cursor: pointer; overflow-wrap: anywhere; }
  .replying { flex-direction: column; align-items: stretch; gap: 10px; }
  .replying .row { display: flex; gap: 12px; justify-content: flex-end; }
</style>
