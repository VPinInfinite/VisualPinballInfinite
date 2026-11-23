// The app's screens, built into ui/dist for the app to carry. WebView2 is Microsoft Edge's
// engine, so the build targets a recent Chromium.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: {
    target: 'chrome120',
    outDir: 'dist',
    emptyOutDir: true,
    // the fonts stay files of their own
    assetsInlineLimit: 0,
  },
});
