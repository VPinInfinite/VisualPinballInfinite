<script>
  // The table editor's tab: straight into the table inspector — and inside it, the achievements
  // editor — on the table last opened there (`prefs.lastEditorTable`), else the library's first by
  // name, or the one chosen from a table's page or the library's right-click; another found by
  // name in its header.
  import Inspector from './Inspector.svelte';
  import { app, savePrefs } from './store.svelte.js';

  $effect(() => {
    if (app.inspectId && app.tables.some((t) => t.id === app.inspectId)) return;
    const last = app.prefs.lastEditorTable;
    const first = [...app.tables].sort((a, b) => a.name.localeCompare(b.name))[0];
    const open = last && app.tables.some((t) => t.id === last) ? last : first?.id;
    if (open) app.inspectId = open;
  });
  // the table opened, remembered for next time
  $effect(() => {
    const id = app.inspectId;
    if (id && app.prefs.lastEditorTable !== id) {
      app.prefs.lastEditorTable = id;
      savePrefs();
    }
  });
</script>

<Inspector id={app.inspectId} />
