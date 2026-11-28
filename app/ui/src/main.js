// The app's screens: the fonts bundled (Inter and Barlow Condensed, SIL OFL 1.1), nothing
// loaded from the internet but VPS's database and pictures, which the app fetches itself.
import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import '@fontsource/inter/700.css';
import '@fontsource/inter/800.css';
import '@fontsource/barlow-condensed/600.css';
import '@fontsource/barlow-condensed/700.css';
import './app.css';
import { mount } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import App from './App.svelte';
import Identify from './lib/Identify.svelte';
import Arrange from './lib/Arrange.svelte';
import CabinetScreen from './lib/CabinetScreen.svelte';
import PauseMenu from './lib/PauseMenu.svelte';

// the Screens page's helpers open the app's page in windows of their own, told by the address
// which they are: a screen's name shown on it, or a stand-in for one of the player's windows; and
// cabinet mode's other screens, the backglass's, the score display's and a topper's; and the pause
// menu over VPX while a game is paused
const hash = location.hash.slice(1);

// the page's own errors written to the app's log (each once, the first fifty), where a problem's
// cause is looked for
const told = new Set();
function logProblem(what) {
  const text = String(what || '').slice(0, 4000);
  if (!text || told.has(text) || told.size >= 50) return;
  told.add(text);
  invoke('page_problem', { text: `${hash || 'main'}: ${text}` }).catch(() => {});
}
window.addEventListener('error', (e) => logProblem(e.error?.stack || e.message));
window.addEventListener('unhandledrejection', (e) => logProblem(e.reason?.stack || e.reason));
const helper = hash.startsWith('identify') ? Identify : hash.startsWith('arrange') ? Arrange : hash.startsWith('cabinet') ? CabinetScreen : hash.startsWith('pause') ? PauseMenu : App;
mount(helper, { target: document.getElementById('app') });
