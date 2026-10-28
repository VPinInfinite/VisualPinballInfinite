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
import App from './App.svelte';
import Identify from './lib/Identify.svelte';
import Arrange from './lib/Arrange.svelte';
import CabinetScreen from './lib/CabinetScreen.svelte';

// the Screens page's helpers open the app's page in windows of their own, told by the address
// which they are: a screen's name shown on it, or a stand-in for one of the player's windows; and
// cabinet mode's other screens, the backglass's, the score display's and a topper's
const hash = location.hash.slice(1);
const helper = hash.startsWith('identify') ? Identify : hash.startsWith('arrange') ? Arrange : hash.startsWith('cabinet') ? CabinetScreen : App;
mount(helper, { target: document.getElementById('app') });
