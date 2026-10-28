<script>
  // The link to VPIDB, on the player hub's achievements and in Settings › VPIDB: connected with the
  // player's Discord account the way a TV signs in — a code shown here, entered on VPIDB's site
  // (`vpidb_connect`, then `vpidb_poll` until it's connected, refused or expired) — this PC named as
  // the player chooses; what's shared chosen before connecting, both ticked — what's earned
  // (`vpidb_sharing`), and the library's ratings and which tables it holds (`vpidb_share_library`)
  // — and either switched off at any time; disconnected on a word, on VPIDB and here
  // (`vpidb_disconnect`; where VPIDB can't be reached, here alone, its own page then the way).
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { notice, openLink } from './store.svelte.js';

  // its heading: none where the page around it is VPIDB's own
  let { heading = true } = $props();

  let link = $state(null);
  let device = $state('');
  // what's shared once connected, as chosen before
  let shareEarned = $state(true);
  let shareLibrary = $state(true);
  // a connection asked for: { userCode, link, until }
  let pending = $state(null);
  let working = $state(false);
  let problem = $state('');

  invoke('vpidb_link').then((l) => {
    link = l;
    device = l.device;
  });

  // how sharing last went (`vpidb_status`, then each `vpidb-sync`): shared, or why not
  let sync = $state(null);
  let syncing = $state(false);
  $effect(() => {
    invoke('vpidb_status').then((s) => (sync = s)).catch(() => {});
    const off = listen('vpidb-sync', (e) => (sync = e.payload));
    return () => off.then((f) => f());
  });
  async function syncNow() {
    syncing = true;
    try {
      sync = await invoke('vpidb_sync');
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      syncing = false;
    }
  }
  const at = (secs) => (secs ? new Date(secs * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : '');

  let timer = null;
  function stop() {
    clearInterval(timer);
    timer = null;
  }
  $effect(() => stop);

  async function connect() {
    working = true;
    problem = '';
    try {
      const s = await invoke('vpidb_connect', { device });
      pending = { userCode: s.userCode, link: s.link, until: Date.now() + s.expiresIn * 1000 };
      openLink(s.link);
      stop();
      timer = setInterval(poll, 1000);
    } catch (e) {
      problem = String(e);
    } finally {
      working = false;
    }
  }

  async function poll() {
    try {
      const state = await invoke('vpidb_poll');
      if (state === 'connected') {
        stop();
        pending = null;
        await invoke('vpidb_sharing', { on: shareEarned });
        link = await invoke('vpidb_share_library', { on: shareLibrary });
        const shared = [shareEarned && 'what you earn', shareLibrary && 'your ratings and which tables you have'].filter(Boolean);
        notice(`Connected to VPIDB as ${link.player?.name ?? 'you'}. ${shared.length ? `Sharing ${shared.join(' and ')}.` : 'Nothing is shared.'} Change it here whenever you like.`, { kind: 'done' });
      } else if (state === 'expired' || state === 'refused' || state === 'none') {
        stop();
        pending = null;
        if (state !== 'none') problem = state === 'expired' ? 'The code ran out before it was entered. Connect again for a new one.' : 'VPIDB refused the connection.';
      }
    } catch (e) {
      stop();
      pending = null;
      problem = String(e);
    }
  }

  function cancel() {
    stop();
    pending = null;
    invoke('vpidb_cancel');
  }

  async function share(on) {
    try {
      link = await invoke('vpidb_sharing', { on });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  async function shareLib(on) {
    try {
      link = await invoke('vpidb_share_library', { on });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
      link = await invoke('vpidb_link');
    }
  }

  let disconnecting = $state(false);
  async function forget() {
    disconnecting = true;
    try {
      const done = await invoke('vpidb_disconnect');
      link = done.link;
      notice(done.there ? 'Disconnected, here and on VPIDB. What you earned stays on VPIDB; your tables and ratings are taken off.' : 'Disconnected here, but VPIDB couldn’t be reached: to disconnect this PC there too, use your page on VPIDB.', { kind: 'done' });
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    } finally {
      disconnecting = false;
    }
  }

  const when = (stamp) => {
    const m = /^(\d{4})-(\d\d)-(\d\d)/.exec(stamp ?? '');
    return m ? new Date(+m[1], +m[2] - 1, +m[3]).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' }) : '';
  };
</script>

<section class="part" class:bare={!heading} aria-label="VPIDB">
  {#if heading}<h2 class="partname">VPIDB</h2>{/if}
  {#if !link}
    <p class="note">…</p>
  {:else if link.connected}
    <div class="card">
      <span>Connected as <b>{link.player?.name}</b>{link.since ? ` since ${when(link.since)}` : ''}, this PC named “{link.device}”.</span>
      {#if sync?.ok === false}
        <p class="note bad">Not sharing right now: {sync.problem || 'VPIDB didn’t answer'}. It tries again every two minutes. <button type="button" class="linkish" disabled={syncing} onclick={syncNow}>{syncing ? 'Trying…' : 'Try now'}</button></p>
      {:else if sync?.ok}
        <p class="note">Shared with VPIDB at {at(sync.at)}.</p>
      {/if}
      <label class="check"><input type="checkbox" checked={link.sharing} onchange={(e) => share(e.currentTarget.checked)} />Share what I earn on VPIDB</label>
      <p class="note">{link.sharing ? 'What you earn goes to VPIDB as you earn it: hardcore counts towards masteries there, softcore shows only on your own page.' : 'Off: nothing you earn leaves this PC.'}</p>
      <label class="check"><input type="checkbox" checked={link.shareLibrary} onchange={(e) => shareLib(e.currentTarget.checked)} />Share my ratings and which tables I have</label>
      <p class="note">{link.shareLibrary ? 'VPIDB counts your tables and stars with everyone’s — how many people have each table, and its average rating — and shows only the totals. Only which table and version it is goes, and its rating: no file names, folders or play times.' : 'Off: your library and ratings stay on this PC, and VPIDB takes off what it had.'}</p>
      <div class="row">
        <button type="button" class="btn small ghost" onclick={() => openLink(link.site)}>Open VPIDB</button>
        <button type="button" class="btn small ghost" disabled={disconnecting} title="Disconnects this PC on VPIDB too: what you earned stays there, your tables and ratings are taken off" onclick={forget}>{disconnecting ? 'Disconnecting…' : 'Disconnect'}</button>
      </div>
    </div>
  {:else if pending}
    <div class="card">
      <span>On VPIDB, sign in with Discord and enter this code:</span>
      <span class="code">{pending.userCode}</span>
      <p class="note">VPIDB’s page opened in your browser with the code filled in. Waiting for you to connect… (or on another device: {pending.link})</p>
      <div class="row">
        <button type="button" class="btn small" onclick={() => openLink(pending.link)}>Open the page again</button>
        <button type="button" class="btn small ghost" onclick={cancel}>Cancel</button>
      </div>
    </div>
  {:else}
    <p class="note">VPIDB is a free, non-commercial home for virtual pinball. Connect with your Discord account to share the achievements you earn, see who’s mastered which tables, and help VPIDB show how many people have each table and how they rate it. Choose what’s shared below; you can change it whenever you like.</p>
    <div class="card">
      <label class="field"><span>This PC’s name on VPIDB</span><input class="select" type="text" bind:value={device} maxlength="60" /></label>
      <label class="check"><input type="checkbox" bind:checked={shareEarned} />Share what I earn</label>
      <label class="check"><input type="checkbox" bind:checked={shareLibrary} />Share my ratings and which tables I have</label>
      <p class="note">Only which table and version, and its stars, go to VPIDB, which shows only totals: no file names, folders or play times.</p>
      <div class="row">
        <button type="button" class="btn small go" disabled={working} onclick={connect}>{working ? 'Connecting…' : 'Connect with Discord…'}</button>
      </div>
      {#if problem}<p class="note warnline">{problem}</p>{/if}
    </div>
  {/if}
</section>

<style>
  .note.bad { color: var(--warn); }
  .linkish { padding: 0; border: 0; background: none; color: var(--accent-soft, var(--accent)); font: inherit; text-decoration: underline; text-underline-offset: 2px; cursor: pointer; }
  .part { display: flex; flex-direction: column; gap: 12px; padding-top: 22px; border-top: 1px solid var(--line); }
  .part.bare { padding-top: 0; border-top: 0; }
  .partname { margin: 0; font-size: 20px; font-weight: 700; color: var(--fg); }
  .note { margin: 0; color: var(--faint); font-size: 13px; max-width: 860px; line-height: 1.5; }
  .warnline { color: var(--warn); }
  .card { display: flex; flex-direction: column; gap: 10px; max-width: 860px; padding: 14px 16px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); color: var(--fg-soft); font-size: 14px; }
  .code { font: 700 30px ui-monospace, Consolas, monospace; letter-spacing: 0.12em; color: var(--fg); }
  .row { display: flex; flex-wrap: wrap; gap: 8px; }
  .btn.go { background: var(--accent-bg); border-color: var(--accent); color: #fff; }
  .check { display: inline-flex; gap: 8px; align-items: center; }
  .check input { accent-color: var(--accent); }
  .field { display: flex; flex-direction: column; gap: 6px; max-width: 360px; font-size: 13px; color: var(--faint); }
  .field input { height: 34px; }
</style>
