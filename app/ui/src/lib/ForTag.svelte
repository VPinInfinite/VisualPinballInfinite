<script>
  // Which tables a setting is for, where Future Pinball is found beside VPX: VPX's, Future
  // Pinball's, or both. Where only VPX is, nothing shows — every setting is VPX's.
  import { app } from './store.svelte.js';

  /** which: 'vpx' | 'fp' | 'both' */
  let { which } = $props();
  const TEXT = { vpx: 'VPX', fp: 'FP', both: 'VPX + FP' };
  const TITLE = {
    vpx: 'For VPX’s tables',
    fp: 'For Future Pinball’s tables',
    both: 'For VPX’s and Future Pinball’s tables',
  };
</script>

{#if app.fp?.folder && TEXT[which]}
  <span class="fortag {which}" title={TITLE[which]}>{TEXT[which]}</span>
{/if}

<style>
  .fortag {
    display: inline-flex; align-items: center; flex: none; height: 18px; padding: 0 7px; border-radius: 9px;
    font-size: 11px; font-weight: 600; letter-spacing: 0.02em; line-height: 1; white-space: nowrap; vertical-align: middle;
    text-transform: none; margin-left: 8px; border: 1px solid transparent;
  }
  .vpx { background: var(--tag-bg); color: var(--tag-fg); border-color: #333b6a; }
  .fp { background: #3a2616; color: #ffb779; border-color: #6a4120; }
  .both { background: #1f2f28; color: #9fd8b0; border-color: #2f5040; }
</style>
