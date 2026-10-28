<script>
  // Identify: a screen's number and name, shown on it for a few seconds by the Screens page, with
  // what the player shows there. A click closes it.
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { params } from './windows.js';

  const p = params('identify');
  const close = () => getCurrentWindow().close();
</script>

<button type="button" class="card" onclick={close} aria-label="Close">
  <span class="n display">{p.get('n')}</span>
  <span class="name">{p.get('name')}</span>
  <span class="size">{p.get('size')?.replace('x', ' × ')} pixels</span>
  <span class="job">{p.get('job')}</span>
</button>

<style>
  .card {
    position: fixed; inset: 0; width: 100%; border: 3px solid var(--accent); border-radius: 0;
    background: var(--panel); color: var(--fg); cursor: pointer;
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; text-align: center;
  }
  .n { font-size: min(34vh, 160px); line-height: 0.9; color: var(--accent); }
  .name { font-size: min(9vh, 36px); font-weight: 700; }
  .size { font-size: min(6vh, 18px); color: var(--muted); }
  .job { font-size: min(7vh, 22px); color: var(--fg-soft); margin-top: 6px; }
</style>
