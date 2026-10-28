<script>
  // A table's script edited in the app: its option lines as controls (Its options) and the whole of
  // it in an editor (Its whole script), one script behind both; saved beside the table as its own
  // script file, which plays in its own's place, the table file untouched. Check it plays the script
  // with no window first and lists its errors by line; Open in Notepad, and Put back the table's own.
  import { invoke } from '@tauri-apps/api/core';
  import { tick, untrack } from 'svelte';
  import { EditorState } from '@codemirror/state';
  import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection } from '@codemirror/view';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import { searchKeymap, highlightSelectionMatches, openSearchPanel, gotoLine } from '@codemirror/search';
  import { StreamLanguage, syntaxHighlighting, HighlightStyle, bracketMatching } from '@codemirror/language';
  import { vbScript } from '@codemirror/legacy-modes/mode/vbscript';
  import { tags } from '@lezer/highlight';
  import Icon from './Icon.svelte';
  import { notice } from './store.svelte.js';

  let { t, onclose, onsettings, onrom } = $props();

  let doc = $state(null);
  let problem = $state('');
  let text = $state('');
  let saved = $state('');
  let tab = $state('options');
  let opts = $state([]);
  let checking = $state(false);
  let checked = $state(null);
  let busy = $state(false);
  // a question asked before something that can't be taken back: its words, and what yes does
  let ask = $state(null);
  const dirty = $derived(!!doc && text !== saved);

  // the editor, made once the script's read, kept while the window's open
  let host = $state(null);
  let view = null;

  const style = HighlightStyle.define([
    { tag: tags.keyword, color: '#7cb7ff' },
    { tag: tags.string, color: '#e6a87c' },
    { tag: tags.comment, color: '#76906f', fontStyle: 'italic' },
    { tag: [tags.number, tags.atom, tags.bool], color: '#b9d99c' },
    { tag: tags.operator, color: '#c3cad6' },
    { tag: tags.standard(tags.variableName), color: '#c792ea' },
  ]);
  const theme = EditorView.theme(
    {
      '&': { height: '100%', backgroundColor: '#0d1118', color: '#e9edf4' },
      '.cm-scroller': { fontFamily: 'Consolas, "Cascadia Mono", ui-monospace, monospace', fontSize: '13px', lineHeight: '1.55' },
      '.cm-gutters': { backgroundColor: '#0d1118', color: '#5c6577', border: 'none' },
      '.cm-activeLine': { backgroundColor: 'rgba(255, 255, 255, 0.04)' },
      '.cm-activeLineGutter': { backgroundColor: 'rgba(255, 255, 255, 0.06)', color: '#c3cad6' },
      '&.cm-focused .cm-cursor': { borderLeftColor: '#4cc2ff' },
      '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': { backgroundColor: 'rgba(76, 194, 255, 0.28) !important' },
      '.cm-panels': { backgroundColor: '#131822', color: '#e9edf4' },
      '.cm-panels.cm-panels-top': { borderBottom: '1px solid #2c3445' },
      '.cm-panels input, .cm-panels button': { color: '#e9edf4', backgroundColor: '#1a2130', border: '1px solid #2c3445', borderRadius: '6px' },
      '.cm-searchMatch': { backgroundColor: 'rgba(255, 200, 80, 0.22)' },
      '.cm-searchMatch-selected': { backgroundColor: 'rgba(255, 200, 80, 0.5)' },
      '.cm-matchingBracket': { backgroundColor: 'rgba(76, 194, 255, 0.25)' },
    },
    { dark: true },
  );

  // made when the window has its script (made again when the script's put back); the text it
  // starts from read untracked, so typing doesn't make it again
  $effect(() => {
    if (!host || view || !doc) return;
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: untrack(() => text),
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightActiveLine(),
          drawSelection(),
          history(),
          bracketMatching(),
          highlightSelectionMatches(),
          StreamLanguage.define(vbScript),
          syntaxHighlighting(style),
          theme,
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) text = u.state.doc.toString();
          }),
        ],
      }),
    });
    return () => {
      view?.destroy();
      view = null;
    };
  });

  // the script read as the page opens it
  $effect(() => {
    invoke('script_open', { id: t.id })
      .then(async (d) => {
        doc = d;
        text = d.text;
        saved = d.text;
        await refreshOptions();
      })
      .catch((e) => (problem = String(e)));
  });

  async function refreshOptions() {
    try {
      opts = await invoke('script_options', { text });
    } catch (e) {
      opts = [];
    }
  }

  // the text put in, the editor in step
  function setText(next) {
    text = next;
    if (view && view.state.doc.toString() !== next) view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
  }

  async function show(which) {
    tab = which;
    if (which === 'options') await refreshOptions();
    else {
      await tick();
      view?.requestMeasure();
    }
  }

  // an option line's new value, written into the script
  async function setOption(o, value) {
    try {
      setText(await invoke('script_with_options', { text, changes: [[o.name, String(value)]] }));
      await refreshOptions();
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
  }

  // a value as the controls show it: a string's words without their quotes
  const words = (v) => (v.startsWith('"') && v.endsWith('"') ? v.slice(1, -1).replaceAll('""', '"') : v);
  // a name as words: `TargetBouncerEnabled` as "Target bouncer enabled"
  const label = (name) => {
    const w = name.replace(/^[bins](?=[A-Z])/, '').replace(/_/g, ' ').replace(/([a-z0-9])([A-Z])/g, '$1 $2').trim();
    return w.charAt(0).toUpperCase() + w.slice(1).replace(/ ([A-Z])(?=[a-z])/g, (_, c) => ` ${c.toLowerCase()}`);
  };
  const groups = $derived.by(() => {
    const out = [];
    for (const o of opts) {
      const g = out.at(-1);
      if (g && g.name === o.group) g.items.push(o);
      else out.push({ name: o.group, items: [o] });
    }
    return out;
  });
  const changed = (o) => doc?.ownValues && o.name in doc.ownValues && doc.ownValues[o.name] !== o.value;

  async function save() {
    busy = true;
    try {
      await invoke('script_save', { id: t.id, text, crlf: doc.crlf });
      saved = text;
      doc.beside = true;
      notice(`${t.short || t.name}’s script is kept beside it, and plays in its own’s place.`);
    } catch (e) {
      notice(String(e), { kind: 'problem' });
    }
    busy = false;
  }

  async function check() {
    checking = true;
    checked = null;
    try {
      checked = await invoke('script_check', { id: t.id, text, crlf: doc.crlf });
    } catch (e) {
      checked = { problems: [], missing: [], failed: String(e) };
    }
    checking = false;
  }

  // to an error's line in the whole script
  async function goTo(line) {
    await show('whole');
    if (!view) return;
    const l = view.state.doc.line(Math.max(1, Math.min(line, view.state.doc.lines)));
    view.dispatch({ selection: { anchor: l.from, head: l.to }, scrollIntoView: true });
    view.focus();
  }

  async function notepad() {
    const open = () =>
      invoke('script_notepad', { id: t.id })
        .then(() => (doc.beside = true))
        .catch((e) => notice(String(e), { kind: 'problem' }));
    if (!dirty) return open();
    ask = {
      text: 'Save your changes first, then open the script in Notepad?',
      yes: async () => {
        await save();
        open();
      },
    };
  }

  function putBack() {
    ask = {
      text: `Put back ${t.short || t.name}’s own script? The script beside it goes to the Recycle Bin, with every change in it.`,
      yes: async () => {
        try {
          await invoke('script_put_back', { id: t.id });
          const d = await invoke('script_open', { id: t.id });
          doc = d;
          saved = d.text;
          setText(d.text);
          await refreshOptions();
        } catch (e) {
          notice(String(e), { kind: 'problem' });
        }
      },
    };
  }

  function close() {
    if (!dirty) return onclose();
    ask = { text: 'Leave without saving your changes?', yes: onclose };
  }

  function answer(yes) {
    const a = ask;
    ask = null;
    if (yes) a.yes();
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !e.defaultPrevented && !document.querySelector('.cm-panels input:focus') && close()} />

<div class="backdrop" role="presentation">
  <div class="win" role="dialog" aria-modal="true" aria-labelledby="se-title" data-loading={doc || problem ? undefined : ''}>
    <header>
      <h2 id="se-title">{t.short || t.name}: its script</h2>
      <div class="tabs" role="tablist">
        <button type="button" role="tab" aria-selected={tab === 'options'} onclick={() => show('options')}>Its options</button>
        <button type="button" role="tab" aria-selected={tab === 'whole'} onclick={() => show('whole')}>Its whole script</button>
      </div>
      <button type="button" class="btn icon ghost" aria-label="Close" onclick={close}><Icon name="x" /></button>
    </header>

    {#if problem}
      <p class="problem">{problem}</p>
    {:else if !doc}
      <p class="soft">Reading its script…</p>
    {:else}
      <div class="body">
        <div class="options" class:hidden={tab !== 'options'}>
          {#if doc.rom}
            <div class="line top">
              <Icon name="chip" size={16} />
              <span>Its ROM: <b>{doc.rom}</b></span>
              <button type="button" class="btn small ghost" onclick={() => (dirty ? (ask = { text: 'Leave without saving your changes, to change its ROM?', yes: onrom }) : onrom())}>Change…</button>
            </div>
          {/if}
          {#if doc.ownMenu}
            <div class="line top">
              <Icon name="sliders" size={16} />
              <span>It also has an options menu of its own, in its settings.</span>
              <button type="button" class="btn small ghost" onclick={() => (dirty ? (ask = { text: 'Leave without saving your changes, to open its settings?', yes: onsettings }) : onsettings())}>Open its settings</button>
            </div>
          {/if}
          {#if !opts.length}
            <p class="soft">Its script has no option lines near its top to change here: the lines a table’s authors leave for players, a setting given a value and explained in a comment. Its whole script is in the next tab.</p>
          {:else}
            <p class="soft">The settings its authors left near the top of its script, each with what its comment says. A change here changes that line in its whole script.</p>
            {#each groups as g, gi (gi)}
              <section>
                {#if g.name}<h3>{g.name}</h3>{/if}
                {#each g.items as o (o.name)}
                  <div class="opt">
                    <div class="name">
                      <span>{label(o.name)}</span>
                      <code>{o.name}</code>
                      {#if changed(o)}<span class="was" title="Its own script has {doc.ownValues[o.name]}">changed</span>{/if}
                    </div>
                    <div class="control">
                      {#if o.choices.length}
                        <select class="select" value={o.value} onchange={(e) => setOption(o, e.currentTarget.value)}>
                          {#if !o.choices.some(([v]) => v === o.value)}<option value={o.value}>{o.value}</option>{/if}
                          {#each o.choices as [v, w] (v)}<option value={v}>{v}: {w.length > 70 ? `${w.slice(0, 68)}…` : w}</option>{/each}
                        </select>
                      {:else if o.kind === 'bool'}
                        <label class="check"><input type="checkbox" checked={o.value.toLowerCase() === 'true'} onchange={(e) => setOption(o, e.currentTarget.checked ? 'True' : 'False')} />{o.value.toLowerCase() === 'true' ? 'On (True)' : 'Off (False)'}</label>
                      {:else if o.kind === 'number'}
                        <input class="select num" type="number" step="any" value={o.value} onchange={(e) => setOption(o, e.currentTarget.value)} />
                      {:else}
                        <input class="select txt" type="text" value={words(o.value)} onchange={(e) => setOption(o, e.currentTarget.value)} />
                      {/if}
                    </div>
                    {#if o.note}<p class="note">{o.note}</p>{/if}
                  </div>
                {/each}
              </section>
            {/each}
          {/if}
        </div>
        <div class="whole" class:hidden={tab !== 'whole'}>
          <div class="tools">
            <button type="button" class="btn small ghost" onclick={() => view && openSearchPanel(view)}><Icon name="search" size={14} />Find</button>
            <button type="button" class="btn small ghost" onclick={() => view && gotoLine(view)}>Go to line</button>
            <span class="soft">Ctrl+F finds, Ctrl+Z takes back, Ctrl+Shift+Z puts back.</span>
          </div>
          <div class="host" bind:this={host}></div>
        </div>
      </div>

      {#if checking || checked}
        <div class="checked" role="status">
          {#if checking}
            <span class="soft">Playing its script for a few seconds, no window…</span>
          {:else if checked.failed}
            <span class="bad"><Icon name="alert" size={15} />It couldn’t be played: {checked.failed}</span>
          {:else if !checked.problems.length}
            <span class="good"><Icon name="check" size={15} />Played with no errors.</span>
            {#if checked.missing.length}<span class="soft">What our engine doesn’t answer yet, which plays on without it: {checked.missing.join(', ')}</span>{/if}
          {:else}
            <span class="bad"><Icon name="alert" size={15} />{checked.problems.length} {checked.problems.length === 1 ? 'error' : 'errors'}:</span>
            {#each checked.problems as p (p.line + ':' + p.column + p.text)}
              <button type="button" class="err" onclick={() => goTo(p.line)}>Line {p.line}{p.column ? `, column ${p.column}` : ''}: {p.text}</button>
            {/each}
          {/if}
        </div>
      {/if}

      <footer>
        {#if ask}
          <span class="ask">{ask.text}</span>
          <button type="button" class="btn ghost" onclick={() => answer(false)}>No</button>
          <button type="button" class="btn go" onclick={() => answer(true)}>Yes</button>
        {:else}
          <span class="state">{dirty ? 'Not saved yet' : doc.beside ? `Kept beside the table as ${doc.file}, played in its own’s place` : 'Its own script, as its file holds it'}</span>
          {#if doc.beside}<button type="button" class="btn ghost" onclick={putBack}>Put back the table’s own script</button>{/if}
          <button type="button" class="btn ghost" onclick={notepad}><Icon name="file" size={15} />Open in Notepad</button>
          <button type="button" class="btn ghost" disabled={checking} onclick={check}><Icon name="play" size={15} />Check it</button>
          <button type="button" class="btn go" disabled={!dirty || busy} onclick={save}>Save</button>
        {/if}
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 50; background: rgba(5, 7, 10, 0.6); display: flex; align-items: center; justify-content: center; padding: 16px; }
  .win { width: min(1280px, 100%); height: min(920px, calc(100vh - 32px)); padding: 16px 18px; border-radius: 16px; background: #131822; border: 1px solid var(--control-line); box-shadow: 0 30px 70px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column; gap: 10px; min-height: 0; }
  header { display: flex; align-items: center; gap: 16px; }
  h2 { margin: 0; font-size: 19px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tabs { display: inline-flex; margin-left: auto; border: 1px solid var(--control-line); border-radius: 10px; overflow: hidden; }
  .tabs button { padding: 7px 14px; border: 0; background: transparent; color: var(--fg-soft); cursor: pointer; }
  .tabs button[aria-selected='true'] { background: var(--accent-bg); color: var(--fg); }
  .body { flex: 1; min-height: 0; display: flex; }
  .hidden { display: none !important; }
  .options { flex: 1; min-width: 0; overflow: auto; padding-right: 6px; display: flex; flex-direction: column; gap: 10px; }
  .whole { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: 6px; }
  .host { flex: 1; min-width: 0; min-height: 0; border: 1px solid var(--card-line); border-radius: 10px; overflow: hidden; }
  .tools { display: flex; align-items: center; gap: 8px; }
  .soft { margin: 0; color: var(--muted); font-size: 13px; }
  .problem { margin: 0; padding: 12px; border-radius: 10px; background: #2a1820; border: 1px solid #5a2a3a; }
  .line.top { display: flex; align-items: center; gap: 10px; padding: 8px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); color: var(--fg-soft); }
  .line.top span { flex: 1; }
  section { display: flex; flex-direction: column; gap: 6px; }
  h3 { margin: 6px 0 0; font-size: 13px; letter-spacing: 0.04em; text-transform: uppercase; color: var(--muted); }
  .opt { display: grid; grid-template-columns: minmax(220px, 300px) minmax(0, 1fr); gap: 4px 16px; align-items: center; padding: 8px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); }
  .name { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 8px; }
  .name code { font-size: 12px; color: var(--muted); }
  .was { padding: 0 6px; border-radius: 5px; background: var(--accent-bg); color: #fff; font-size: 11px; font-weight: 600; }
  .control { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .select { max-width: 100%; }
  .num { width: 130px; }
  .txt { width: min(360px, 100%); }
  .check { display: flex; align-items: center; gap: 8px; color: var(--fg-soft); }
  .note { grid-column: 1 / -1; margin: 0; color: var(--muted); font-size: 13px; line-height: 1.45; }
  .checked { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 12px; padding: 8px 12px; border-radius: 10px; background: var(--card); border: 1px solid var(--card-line); font-size: 13px; }
  .good { display: inline-flex; align-items: center; gap: 6px; color: var(--good); }
  .bad { display: inline-flex; align-items: center; gap: 6px; color: var(--warn); }
  .err { padding: 3px 8px; border-radius: 6px; border: 1px solid var(--card-line); background: transparent; color: var(--fg); font-family: Consolas, ui-monospace, monospace; font-size: 12px; cursor: pointer; text-align: left; }
  .err:hover { border-color: var(--accent); }
  footer { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .state, .ask { margin-right: auto; color: var(--fg-soft); font-size: 13px; }
  .ask { color: var(--fg); }
  .go { background: var(--accent-bg); border-color: var(--accent); }
</style>
