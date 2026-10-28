<script>
  // Settings: Controls. Every game action on any button — press a key or a
  // controller's button to give it one — and a live test: each action lights as it's pressed, the
  // nudge and the plunger move as they're moved. The controllers, their presses and the actions
  // they make come from the player's own input test, so they're numbered and named as the player
  // reads them; the keyboard is the page's own. Saved under VPX 5436's names, only what changed.
  // The keys and the controllers' buttons in columns of their own: a
  // mapping's alternatives go under Keyboard when they're all keys or mouse buttons, else under
  // Controller; one is changed by clicking it, another added with its column's +.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';
  import { CODES } from './keycodes.js';

  // the actions, in groups, in plain words
  const GROUPS = [
    ['Flippers', [['LeftFlipper', 'Left flipper'], ['RightFlipper', 'Right flipper'], ['LeftStagedFlipper', 'Left upper flipper'], ['RightStagedFlipper', 'Right upper flipper'], ['LeftMagna', 'Left magna-save'], ['RightMagna', 'Right magna-save']]],
    ['Ball and cabinet', [['LaunchBall', 'Launch the ball'], ['Lockbar', 'Lockbar button'], ['LeftNudge', 'Nudge from the left'], ['RightNudge', 'Nudge from the right'], ['CenterNudge', 'Nudge forward'], ['Tilt', 'Tilt bob'], ['SlamTilt', 'Slam tilt']]],
    ['Coins and start', [['Credit1', 'Coin 1'], ['Credit2', 'Coin 2'], ['Credit3', 'Coin 3'], ['Credit4', 'Coin 4'], ['Start', 'Start'], ['ExtraBall', 'Buy-in or extra ball']]],
    ['Coin door', [['CoinDoor', 'Open or close the coin door'], ['Reset', 'Reset'], ['Service1', 'Service button 1'], ['Service2', 'Service button 2'], ['Service3', 'Service button 3'], ['Service4', 'Service button 4'], ['Service5', 'Service button 5'], ['Service6', 'Service button 6'], ['Service7', 'Service button 7'], ['Service8', 'Service button 8']]],
    ['The player', [['VolumeDown', 'Volume down'], ['VolumeUp', 'Volume up'], ['Screenshot', 'Screenshot']]],
    ['The pause menu', [['Pause', 'Pause and open the menu'], ['InGameUI', 'Open the menu (a gamepad’s Back)'], ['UIUp', 'Menu: up'], ['UIDown', 'Menu: down'], ['UILeft', 'Menu: left, or less'], ['UIRight', 'Menu: right, or more']]],
  ];


  let keys = $state({});
  let devices = $state({});
  let defaults = $state({});
  let draft = $state(null);
  let saved = $state('');
  let touched = $state({});
  let pressed = $state({});
  let keyDown = $state({});
  let sensors = $state({ nudge: null, plunger: null });
  let capturing = $state(null);
  let busy = $state(false);
  let problem = $state('');
  // each axis's last position, for telling a press from where it rests
  const axes = {};
  let rests = {};

  const changed = $derived(!!draft && JSON.stringify(draft) !== saved);

  function onEvent(m) {
    if (m.t === 'keys') keys = m.names;
    else if (m.t === 'actions') {
      const d = draft ? { ...draft } : {};
      const s = saved ? JSON.parse(saved) : {};
      for (const a of m.list) {
        defaults[a.id] = a.default;
        // a controller connecting brings its own layout: not the user's change
        if (!touched[a.id]) {
          d[a.id] = a.mapping;
          s[a.id] = a.mapping;
        }
      }
      draft = d;
      saved = JSON.stringify(Object.fromEntries(Object.keys(d).map((k) => [k, s[k] ?? d[k]])));
    } else if (m.t === 'device') {
      if (m.on) devices = { ...devices, [m.id]: m.name || m.id };
      else {
        const { [m.id]: _, ...rest } = devices;
        devices = rest;
      }
    } else if (m.t === 'action') pressed = { ...pressed, [m.id]: m.on };
    else if (m.t === 'sensors') sensors = { nudge: m.nudge, plunger: m.plunger };
    else if (m.t === 'element') {
      const id = `${m.device};${m.element}`;
      if (capturing?.col === 'pad') captureElement(m, id);
      axes[id] = m.value;
    }
  }

  onMount(() => {
    const off = listen('input-test', (e) => onEvent(e.payload));
    invoke('controls_start').catch((e) => (problem = String(e)));
    return () => {
      off.then((f) => f());
      invoke('controls_stop');
    };
  });

  // a mapping's alternatives, each the buttons held together
  const groups = (text) => (text || '').split('|').map((g) => g.split('&').map((b) => b.trim()).filter(Boolean)).filter((g) => g.length);
  const join = (gs) => gs.map((g) => g.join(' & ')).join(' | ');

  // the column an alternative goes in: the keyboard's when it's keys and mouse buttons alone
  const onKeyboard = (bind) => bind.startsWith('Key;') || bind.startsWith('Mouse;');
  const colOf = (g) => (g.every(onKeyboard) ? 'keys' : 'pad');

  /** A button of a mapping, in words: a controller's named only when more than one is connected. */
  function said(bind) {
    const [device, element, dir, threshold] = bind.split(';');
    const n = Number(element);
    if (device === 'Key') return keys[n] || `Key ${n}`;
    if (device === 'Mouse') return `Mouse button ${n}`;
    const who = Object.keys(devices).length > 1 && devices[device] ? `${devices[device]} · ` : '';
    if (n < 0x100) return `${who}Button ${n + 1}`;
    if (n < 0x200) {
      const hat = Math.floor((n - 0x100) / 4);
      return `${who}${hat ? `Hat ${hat + 1} ` : 'Pad '}${['left', 'right', 'up', 'down'][(n - 0x100) % 4]}`;
    }
    const t = Number(threshold);
    return `${who}Axis ${n - 0x200 + 1} ${dir === 'x' ? 'below' : 'past'} ${Number.isFinite(t) ? t.toFixed(2) : ''}`.trim();
  }

  // an alternative on a controller that isn't connected says so when pointed at
  const away = (g) => g.some((b) => !onKeyboard(b) && !devices[b.split(';')[0]]);

  // VPX's layout for any gamepad, which the player gives one as it connects (input_map's
  // `default_layout`): shown while none is connected, so the Controller column says what each does
  const PAD = {
    LeftFlipper: 'Left trigger',
    RightFlipper: 'Right trigger',
    LeftStagedFlipper: 'Left trigger, pressed further',
    RightStagedFlipper: 'Right trigger, pressed further',
    LeftMagna: 'Left shoulder',
    RightMagna: 'Right shoulder',
    Credit1: 'Y (top button)',
    Start: 'B (right button)',
    LaunchBall: 'A (bottom button)',
    InGameUI: 'Back',
    Service1: 'D-pad left',
    Service2: 'D-pad down',
    Service3: 'D-pad up',
    Service4: 'D-pad right',
    UIUp: 'D-pad up',
    UIDown: 'D-pad down',
    UILeft: 'D-pad left',
    UIRight: 'D-pad right',
    // the sticks are read as the cabinet's sensors: the left nudges, the right pulls the plunger
    LeftNudge: 'Left stick',
    RightNudge: 'Left stick',
    CenterNudge: 'Left stick',
  };

  /** Wait for a press for `id`'s `col` column: in place of its alternative `k`, or another. */
  function startCapture(id, col, k = null) {
    capturing = { id, col, k };
    rests = { ...axes };
  }

  function give(bind) {
    const { id, k } = capturing;
    const gs = groups(draft[id]);
    const there = gs.findIndex((g) => g.length === 1 && g[0] === bind);
    if (k !== null && k < gs.length) {
      gs[k] = [bind];
      // pressed again where it already was
      if (there >= 0 && there !== k) gs.splice(there, 1);
    } else if (there < 0) gs.push([bind]);
    draft[id] = join(gs);
    touched[id] = true;
    capturing = null;
  }

  function captureElement(m, id) {
    if (m.element < 0x200) {
      if (m.value === 1) give(id);
      return;
    }
    const rest = rests[id] ?? 0;
    const moved = m.value - rest;
    if (Math.abs(moved) >= 0.7) {
      const threshold = rest + moved * 0.6;
      give(`${id};${moved > 0 ? 'o' : 'x'};${threshold.toFixed(6)}`);
    }
  }

  function remove(id, k) {
    const gs = groups(draft[id]);
    gs.splice(k, 1);
    draft[id] = join(gs);
    touched[id] = true;
  }

  function toDefault(id) {
    draft[id] = defaults[id] || '';
    touched[id] = true;
  }

  function key(e, down) {
    const code = CODES[e.code];
    if (capturing && down) {
      e.preventDefault();
      if (e.code === 'Escape') capturing = null;
      else if (code && capturing.col === 'keys') give(`Key;${code}`);
      return;
    }
    if (!code || !draft) return;
    // the actions on this key alone light while it's held
    for (const [id, text] of Object.entries(draft)) {
      if (groups(text).some((g) => g.length === 1 && g[0] === `Key;${code}`)) keyDown = { ...keyDown, [id]: down };
    }
  }

  async function save() {
    busy = true;
    try {
      await invoke('save_controls', { mappings: $state.snapshot(draft), defaults: $state.snapshot(defaults) });
      saved = JSON.stringify(draft);
      touched = {};
      notice('Saved. Tables started from now on use these.', { kind: 'done' });
      // the input test again, reading what was saved
      await invoke('controls_start');
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  function undo() {
    draft = JSON.parse(saved);
    touched = {};
  }

  const lit = (id) => !!(pressed[id] || keyDown[id]);
  const pct = (v) => `${Math.round(((v ?? 0) + 1) * 50)}%`;
</script>

<svelte:window onkeydown={(e) => key(e, true)} onkeyup={(e) => key(e, false)} />

<div class="page" data-loading={draft || problem ? undefined : ''}>
  <header>
    <div class="title">
      <h1 class="display">Controls</h1>
      <p>Every game action and the keys and buttons that press it. Press any of them to see it light up here; click a key or button to change it, or + to add another.</p>
    </div>
    <div class="actions">
      <button type="button" class="btn ghost" onclick={undo} disabled={!changed || busy}>Undo changes</button>
      <button type="button" class="btn save" onclick={save} disabled={!changed || busy}><Icon name="check" />Save</button>
    </div>
  </header>

  {#if problem}
    <div class="problem">{problem}</div>
  {:else if !draft}
    <p class="soft">Asking the player about your controls…</p>
  {:else}
    <section class="live">
      <div class="who">
        <h2 class="section-title">Controllers</h2>
        {#if Object.keys(devices).length}
          {#each Object.entries(devices) as [id, name] (id)}<p class="dev"><Icon name="disc" size={15} />{name}</p>{/each}
        {:else}
          <p class="soft">None connected — the keyboard works as always. Connect one and it shows here; until then the Controller column shows what a gamepad does once connected.</p>
        {/if}
      </div>
      <div class="sense">
        <div class="nudge" title="Nudge">
          <span class="cap">Nudge</span>
          <div class="pad">
            {#if sensors.nudge}<span class="dot" style="left: {pct(sensors.nudge[0])}; top: {pct(sensors.nudge[1])}"></span>{:else}<span class="none">none</span>{/if}
          </div>
        </div>
        <div class="plunger" title="Plunger">
          <span class="cap">Plunger</span>
          <div class="bar">{#if sensors.plunger !== null && sensors.plunger !== undefined}<span style="height: {pct(sensors.plunger)}"></span>{:else}<span class="none">none</span>{/if}</div>
        </div>
      </div>
    </section>

    {#if capturing}
      <div class="capturing" role="status">
        <Icon name="disc" />
        <span>
          {#if capturing.col === 'keys'}Press the key for{:else}Press the controller’s button for{/if}
          <b>{GROUPS.flatMap((g) => g[1]).find((a) => a[0] === capturing.id)?.[1]}</b>{#if capturing.col === 'pad'} — or move a stick or trigger{/if}. Esc to stop.
        </span>
        <button type="button" class="btn small" onclick={() => (capturing = null)}>Stop</button>
      </div>
    {/if}

    {#each GROUPS as [title, list] (title)}
      <section class="block">
        <div class="grid head">
          <h2 class="section-title">{title}</h2>
          <span class="col">Keyboard</span>
          <span class="col">Controller</span>
        </div>
        <div class="rows">
          {#each list as [id, label] (id)}
            {@const gs = groups(draft[id])}
            <div class="grid arow" class:lit={lit(id)} class:waiting={capturing?.id === id}>
              <span class="label"><span class="led" aria-hidden="true"></span>{label}</span>
              {#each ['keys', 'pad'] as col (col)}
                <span class="binds">
                  {#each gs as g, k (k)}
                    {#if colOf(g) === col}
                      {@const words = g.map(said).join(' + ')}
                      <span class="chip" class:waiting={capturing?.id === id && capturing.k === k} title={away(g) ? 'On a controller that isn’t connected now' : undefined}>
                        <button type="button" class="change" aria-label="Change {words}" onclick={() => startCapture(id, col, k)}>{words}</button>
                        <button type="button" class="x" aria-label="Remove {words}" onclick={() => remove(id, k)}><Icon name="x" size={12} /></button>
                      </span>
                    {/if}
                  {/each}
                  {#if col === 'pad' && PAD[id] && !Object.keys(devices).length && !gs.some((g) => colOf(g) === 'pad')}
                    <span class="chip ghost" title="What a gamepad does once it’s connected (VPX’s layout for gamepads)">Gamepad: {PAD[id]}</span>
                  {/if}
                  <button
                    type="button"
                    class="add"
                    class:waiting={capturing?.id === id && capturing.col === col && capturing.k === null}
                    title={col === 'keys' ? 'Add a key' : 'Add a controller button'}
                    aria-label="{col === 'keys' ? 'Add a key' : 'Add a controller button'} for {label}"
                    onclick={() => startCapture(id, col)}><Icon name="plus" size={13} /></button>
                </span>
              {/each}
              <span class="tools">
                {#if (defaults[id] || '') !== (draft[id] || '')}<button type="button" class="btn small ghost" title="VPX’s own: {groups(defaults[id]).map((g) => g.map(said).join(' + ')).join(', ') || 'nothing'}" onclick={() => toDefault(id)}>VPX’s</button>{/if}
              </span>
            </div>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</div>

<style>
  .page { padding: 28px 32px 48px; display: flex; flex-direction: column; gap: 22px; max-width: 1240px; }
  header { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-end; justify-content: space-between; }
  .title h1 { font-size: 38px; line-height: 1; }
  .title p { margin: 8px 0 0; color: var(--muted); max-width: 680px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .save { background: var(--accent-bg); border-color: var(--accent); }
  .soft { color: var(--faint); }
  .problem { padding: 14px 16px; border-radius: 12px; background: #2a1820; border: 1px solid #5a2a3a; }
  .live { display: flex; gap: 28px; align-items: flex-start; flex-wrap: wrap; padding: 14px 16px; border-radius: 12px; background: var(--card); border: 1px solid var(--card-line); }
  .who { flex: 1; min-width: 260px; display: flex; flex-direction: column; gap: 6px; }
  .who p { margin: 0; }
  .dev { display: flex; align-items: center; gap: 8px; }
  .sense { display: flex; gap: 22px; }
  .cap { display: block; font-size: 12px; color: var(--faint); margin-bottom: 6px; text-align: center; }
  .pad { position: relative; width: 96px; height: 96px; border-radius: 10px; background: #0d1119; border: 1px solid var(--control-line); }
  .pad::before, .pad::after { content: ''; position: absolute; background: #222b3a; }
  .pad::before { left: 50%; top: 8px; bottom: 8px; width: 1px; }
  .pad::after { top: 50%; left: 8px; right: 8px; height: 1px; }
  .dot { position: absolute; width: 12px; height: 12px; margin: -6px 0 0 -6px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 8px var(--accent); z-index: 1; }
  .bar { position: relative; width: 26px; height: 96px; border-radius: 8px; background: #0d1119; border: 1px solid var(--control-line); overflow: hidden; display: flex; align-items: flex-end; justify-content: center; }
  .bar span:not(.none) { width: 100%; background: var(--accent); }
  .none { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: 11px; color: var(--faint); }
  .capturing { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: 12px; background: #10263a; border: 1px solid #1f4a6e; position: sticky; top: 0; z-index: 2; }
  .capturing span { flex: 1; }
  .block { display: flex; flex-direction: column; gap: 6px; }
  .grid { display: grid; grid-template-columns: 240px minmax(0, 1fr) minmax(0, 1fr) 64px; gap: 14px; align-items: center; }
  .head { padding: 0 11px; }
  .head h2 { margin: 0; }
  .col { font-size: 13px; color: var(--muted); }
  .rows { display: flex; flex-direction: column; gap: 4px; }
  .arow { padding: 6px 10px; border-radius: 10px; border: 1px solid transparent; }
  .arow:hover { background: var(--hover); }
  .arow.lit { background: color-mix(in srgb, var(--good) 14%, transparent); border-color: color-mix(in srgb, var(--good) 45%, transparent); }
  .arow.waiting { border-color: color-mix(in srgb, var(--accent) 50%, transparent); }
  .led { width: 10px; height: 10px; border-radius: 50%; background: #263042; flex: 0 0 auto; }
  .arow.lit .led { background: var(--good); box-shadow: 0 0 8px var(--good); }
  .label { display: flex; align-items: center; gap: 12px; font-size: 14px; }
  .binds { min-width: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .chip { display: inline-flex; align-items: center; border-radius: 7px; background: var(--control); border: 1px solid var(--control-line); font-size: 13px; }
  .chip.waiting, .add.waiting { border-color: var(--accent); background: var(--accent-bg); }
  .chip.ghost { padding: 4px 9px; background: transparent; border-style: dashed; color: var(--muted); }
  .chip.ghost:hover { border-color: var(--control-line); }
  .change { padding: 3px 4px 3px 10px; border: 0; border-radius: 7px 0 0 7px; background: transparent; color: var(--fg); font-size: 13px; text-align: left; }
  .change:hover { color: #fff; }
  .chip:hover { border-color: var(--accent); }
  .x { display: inline-flex; margin-right: 3px; padding: 2px; border: 0; border-radius: 5px; background: transparent; color: var(--faint); }
  .x:hover { background: var(--hover); color: var(--fg); }
  .add { display: inline-flex; align-items: center; justify-content: center; width: 26px; height: 26px; border-radius: 7px; border: 1px dashed var(--control-line); background: transparent; color: var(--faint); }
  .add:hover { border-color: var(--accent); color: var(--fg); }
  .tools { display: flex; gap: 6px; justify-content: flex-end; }
</style>
