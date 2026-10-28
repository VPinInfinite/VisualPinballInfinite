<script>
  // One of cabinet mode's other screens — the backglass's, the score display's or a topper's, as its
  // address says — showing the table chosen, as the main window tells it (`cabinet-show`); its
  // video silent unless the main window gives it the one sound (a topper's video), at its volume.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { volumeAt } from './store.svelte.js';

  const screen = new URLSearchParams(location.hash.slice(1)).get('screen') || 'backglass';
  let what = $state(null);

  onMount(() => {
    invoke('cabinet_showing').then((w) => (what = w));
    const off = listen('cabinet-show', (e) => (what = e.payload));
    return () => off.then((f) => f());
  });

  const m = $derived(what?.[screen]);
</script>

<div class="s">
  {#if m?.video}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video src={m.url} autoplay loop playsinline muted={what?.sound !== screen} use:volumeAt={what?.volume ?? 0}></video>
  {:else if m}
    <img src={m.url} alt="" />
  {:else if what?.name && screen !== 'topper'}
    <span class="name display">{what.name}</span>
  {/if}
</div>

<style>
  .s { position: fixed; inset: 0; background: #000; display: flex; align-items: center; justify-content: center; overflow: hidden; }
  video, img { width: 100%; height: 100%; object-fit: contain; }
  /* drawn with the page, not handed to the graphics card as an overlay of its own: a large video
     (Dracula's 1920 × 1080, Futurama's 3840 × 2160) made one showed clipped to a small box in the
     corner on a second screen, for seconds or for good. A video not quite opaque is never made one. */
  video { opacity: 0.999; }
  .name { font-size: 12vh; color: #e9edf4; text-transform: uppercase; text-align: center; padding: 4vh; }
</style>
